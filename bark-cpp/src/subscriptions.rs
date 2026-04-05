use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use anyhow::{Context, anyhow};
use bark::ark::Address;
use bark::ark::lightning::PaymentHash;
use bark::movement::{Movement, PaymentMethod};
use bark::subsystem::Subsystem;
use bark::{NotificationStream, WalletNotification};
use futures::StreamExt;
use logger::log::{debug, info, warn};
use tokio::task::JoinHandle;

use crate::GLOBAL_WALLET_MANAGER;
use crate::TOKIO_RUNTIME;
use crate::cxx::ffi;
use crate::utils;

#[derive(Clone)]
enum NotificationFilter {
    All,
    ArkoorAddress(Address),
    LightningPayment(PaymentHash),
}

impl NotificationFilter {
    fn describe(&self) -> String {
        match self {
            NotificationFilter::All => "all notifications".to_string(),
            NotificationFilter::ArkoorAddress(address) => {
                format!("ark address {}", address)
            }
            NotificationFilter::LightningPayment(payment_hash) => {
                format!("lightning payment {}", payment_hash)
            }
        }
    }
}

pub struct NotificationSubscription {
    rx: Receiver<ffi::NotificationEvent>,
    task: Option<JoinHandle<()>>,
    active: Arc<AtomicBool>,
}

impl NotificationSubscription {
    fn spawn(stream: NotificationStream, filter: NotificationFilter) -> Self {
        let (tx, rx) = mpsc::channel();
        let active = Arc::new(AtomicBool::new(true));
        let active_flag = Arc::clone(&active);
        let filter_description = filter.describe();

        info!("Starting Bark subscription for {}", filter_description);

        let task = TOKIO_RUNTIME.spawn(async move {
            let mut stream = stream;
            while let Some(notification) = stream.next().await {
                debug!(
                    "Received Bark notification for {}: {}",
                    filter_description,
                    notification_summary(&notification)
                );
                match notification_to_event(notification, &filter) {
                    Ok(Some(event)) => {
                        info!(
                            "Subscription {} emitting event kind={} movement_id={}",
                            filter_description, event.kind, event.movement.id
                        );
                        if tx.send(event).is_err() {
                            warn!(
                                "Subscription receiver dropped for {}; stopping task",
                                filter_description
                            );
                            break;
                        }
                    }
                    Ok(None) => {
                        debug!("Notification filtered out for {}", filter_description);
                    }
                    Err(error) => {
                        warn!("Failed to convert Bark notification: {error:#}");
                    }
                }
            }

            info!("Bark subscription task ended for {}", filter_description);
            active_flag.store(false, Ordering::SeqCst);
        });

        Self {
            rx,
            task: Some(task),
            active,
        }
    }

    pub fn stop(mut self: Pin<&mut Self>) -> anyhow::Result<()> {
        info!("Stopping Bark subscription handle");
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub fn wait_next(
        self: Pin<&mut Self>,
        timeout_ms: u32,
    ) -> anyhow::Result<ffi::NotificationPollResult> {
        if !self.is_active() {
            debug!("wait_next called on inactive Bark subscription");
            return Ok(empty_poll_result(false));
        }

        match self
            .rx
            .recv_timeout(Duration::from_millis(timeout_ms as u64))
        {
            Ok(event) => Ok(ffi::NotificationPollResult {
                has_event: true,
                is_active: self.is_active(),
                event,
            }),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(empty_poll_result(self.is_active())),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                info!("Bark subscription channel disconnected");
                self.active.store(false, Ordering::SeqCst);
                Ok(empty_poll_result(false))
            }
        }
    }
}

impl Drop for NotificationSubscription {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.active.store(false, Ordering::SeqCst);
    }
}

fn empty_bark_movement() -> ffi::BarkMovement {
    ffi::BarkMovement {
        id: 0,
        status: String::new(),
        subsystem_name: String::new(),
        subsystem_kind: String::new(),
        metadata_json: String::new(),
        intended_balance_sat: 0,
        effective_balance_sat: 0,
        offchain_fee_sat: 0,
        sent_to: Vec::new(),
        received_on: Vec::new(),
        input_vtxos: Vec::new(),
        output_vtxos: Vec::new(),
        exited_vtxos: Vec::new(),
        created_at: String::new(),
        updated_at: String::new(),
        completed_at: String::new(),
    }
}

fn empty_notification_event() -> ffi::NotificationEvent {
    ffi::NotificationEvent {
        kind: String::new(),
        has_movement: false,
        movement: empty_bark_movement(),
    }
}

fn empty_poll_result(is_active: bool) -> ffi::NotificationPollResult {
    ffi::NotificationPollResult {
        has_event: false,
        is_active,
        event: empty_notification_event(),
    }
}

fn movement_matches_filter(movement: &Movement, filter: &NotificationFilter) -> bool {
    match filter {
        NotificationFilter::All => true,
        NotificationFilter::ArkoorAddress(address) => {
            debug!(
                "Evaluating arkoor filter target={} movement_id={} subsystem={} received_on={}",
                address,
                movement.id.0,
                movement.subsystem.kind,
                format_destinations(movement)
            );
            if !movement.subsystem.is_subsystem(Subsystem::ARKOOR) {
                debug!(
                    "Arkoor filter rejected movement_id={} because subsystem was {}",
                    movement.id.0, movement.subsystem.kind
                );
                return false;
            }

            let matched =
                movement
                    .received_on
                    .iter()
                    .any(|destination| matches!(destination.destination, PaymentMethod::Ark(ref candidate) if candidate == address));

            debug!(
                "Arkoor filter result target={} movement_id={} matched={}",
                address, movement.id.0, matched
            );
            matched
        }
        NotificationFilter::LightningPayment(payment_hash) => {
            debug!(
                "Evaluating lightning filter target={} movement_id={} subsystem={} metadata={:?}",
                payment_hash, movement.id.0, movement.subsystem.kind, movement.metadata
            );
            if !movement
                .subsystem
                .is_subsystem(Subsystem::LIGHTNING_RECEIVE)
                && !movement.subsystem.is_subsystem(Subsystem::LIGHTNING_SEND)
            {
                debug!(
                    "Lightning filter rejected movement_id={} because subsystem was {}",
                    movement.id.0, movement.subsystem.kind
                );
                return false;
            }

            if movement
                .metadata
                .get("payment_hash")
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse().ok())
                == Some(*payment_hash)
            {
                debug!(
                    "Lightning filter matched movement_id={} using metadata payment_hash",
                    movement.id.0
                );
                return true;
            }

            for destination in &movement.received_on {
                match destination.destination {
                    PaymentMethod::Invoice(ref invoice)
                        if invoice.payment_hash() == *payment_hash =>
                    {
                        debug!(
                            "Lightning filter matched movement_id={} using invoice destination",
                            movement.id.0
                        );
                        return true;
                    }
                    _ => {}
                }
            }

            false
        }
    }
}

fn notification_to_event(
    notification: WalletNotification,
    filter: &NotificationFilter,
) -> anyhow::Result<Option<ffi::NotificationEvent>> {
    match notification {
        WalletNotification::MovementCreated { movement } => {
            if !movement_matches_filter(&movement, filter) {
                return Ok(None);
            }

            info!(
                "Matched MovementCreated for filter={} movement_id={} status={}",
                filter.describe(),
                movement.id.0,
                movement.status
            );
            Ok(Some(ffi::NotificationEvent {
                kind: "movementCreated".to_string(),
                has_movement: true,
                movement: utils::movement_to_bark_movement(&movement)?,
            }))
        }
        WalletNotification::MovementUpdated { movement } => {
            if !movement_matches_filter(&movement, filter) {
                return Ok(None);
            }

            info!(
                "Matched MovementUpdated for filter={} movement_id={} status={}",
                filter.describe(),
                movement.id.0,
                movement.status
            );
            Ok(Some(ffi::NotificationEvent {
                kind: "movementUpdated".to_string(),
                has_movement: true,
                movement: utils::movement_to_bark_movement(&movement)?,
            }))
        }
        WalletNotification::ChannelLagging => {
            info!(
                "Received ChannelLagging notification for {}",
                filter.describe()
            );
            Ok(Some(ffi::NotificationEvent {
                kind: "channelLagging".to_string(),
                has_movement: false,
                movement: empty_bark_movement(),
            }))
        }
    }
}

fn format_destinations(movement: &Movement) -> String {
    movement
        .received_on
        .iter()
        .map(|destination| match &destination.destination {
            PaymentMethod::Ark(address) => {
                format!("ark:{}:{}sat", address, destination.amount)
            }
            PaymentMethod::Bitcoin(address) => {
                format!("bitcoin:{:?}:{}sat", address, destination.amount)
            }
            PaymentMethod::Invoice(invoice) => {
                format!(
                    "invoice:{}:{}sat",
                    invoice.payment_hash(),
                    destination.amount
                )
            }
            PaymentMethod::Offer(offer) => {
                format!("offer:{}:{}sat", offer.id(), destination.amount)
            }
            PaymentMethod::LightningAddress(address) => {
                format!("lnaddr:{}:{}sat", address, destination.amount)
            }
            PaymentMethod::OutputScript(script) => {
                format!("script:{}:{}sat", script, destination.amount)
            }
            PaymentMethod::Custom(value) => {
                format!("custom:{}:{}sat", value, destination.amount)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn notification_summary(notification: &WalletNotification) -> String {
    match notification {
        WalletNotification::MovementCreated { movement } => format!(
            "MovementCreated id={} status={} subsystem={} metadata={:?}",
            movement.id.0, movement.status, movement.subsystem.kind, movement.metadata
        ),
        WalletNotification::MovementUpdated { movement } => format!(
            "MovementUpdated id={} status={} subsystem={} metadata={:?}",
            movement.id.0, movement.status, movement.subsystem.kind, movement.metadata
        ),
        WalletNotification::ChannelLagging => "ChannelLagging".to_string(),
    }
}

pub async fn subscribe_notifications() -> anyhow::Result<Box<NotificationSubscription>> {
    info!("Creating subscription handle for all notifications");
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager.with_context(|ctx| {
        Ok(Box::new(NotificationSubscription::spawn(
            ctx.wallet.subscribe_notifications(),
            NotificationFilter::All,
        )))
    })
}

pub async fn subscribe_arkoor_address_movements(
    address: &str,
) -> anyhow::Result<Box<NotificationSubscription>> {
    let address = Address::from_str(address)
        .with_context(|| format!("Invalid Arkoor address format: '{address}'"))?;

    info!(
        "Creating subscription handle for arkoor address {}",
        address
    );

    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager.with_context(|ctx| {
        Ok(Box::new(NotificationSubscription::spawn(
            ctx.wallet.subscribe_notifications(),
            NotificationFilter::ArkoorAddress(address),
        )))
    })
}

pub async fn subscribe_lightning_payment_movements(
    payment_hash: &str,
) -> anyhow::Result<Box<NotificationSubscription>> {
    let payment_hash = PaymentHash::from_str(payment_hash)
        .map_err(|_| anyhow!("Invalid payment hash format: '{payment_hash}'"))?;

    info!(
        "Creating subscription handle for lightning payment {}",
        payment_hash
    );

    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager.with_context(|ctx| {
        Ok(Box::new(NotificationSubscription::spawn(
            ctx.wallet.subscribe_notifications(),
            NotificationFilter::LightningPayment(payment_hash),
        )))
    })
}
