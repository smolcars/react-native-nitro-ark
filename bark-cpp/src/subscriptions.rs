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
use logger::log::warn;
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

        let task = TOKIO_RUNTIME.spawn(async move {
            let mut stream = stream;
            while let Some(notification) = stream.next().await {
                match notification_to_event(notification, &filter) {
                    Ok(Some(event)) => {
                        if tx.send(event).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        warn!("Failed to convert Bark notification: {error:#}");
                    }
                }
            }

            active_flag.store(false, Ordering::SeqCst);
        });

        Self {
            rx,
            task: Some(task),
            active,
        }
    }

    pub fn stop(mut self: Pin<&mut Self>) -> anyhow::Result<()> {
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
            if !movement.subsystem.is_subsystem(Subsystem::ARKOOR) {
                return false;
            }

            movement
                .received_on
                .iter()
                .any(|destination| match destination.destination {
                    PaymentMethod::Ark(ref candidate) if candidate == address => true,
                    _ => false,
                })
        }
        NotificationFilter::LightningPayment(payment_hash) => {
            if !movement
                .subsystem
                .is_subsystem(Subsystem::LIGHTNING_RECEIVE)
                && !movement.subsystem.is_subsystem(Subsystem::LIGHTNING_SEND)
            {
                return false;
            }

            if movement
                .metadata
                .get("payment_hash")
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse().ok())
                == Some(*payment_hash)
            {
                return true;
            }

            for destination in &movement.received_on {
                match destination.destination {
                    PaymentMethod::Invoice(ref invoice)
                        if invoice.payment_hash() == *payment_hash =>
                    {
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

            Ok(Some(ffi::NotificationEvent {
                kind: "movementUpdated".to_string(),
                has_movement: true,
                movement: utils::movement_to_bark_movement(&movement)?,
            }))
        }
        WalletNotification::ChannelLagging => Ok(Some(ffi::NotificationEvent {
            kind: "channelLagging".to_string(),
            has_movement: false,
            movement: empty_bark_movement(),
        })),
    }
}

pub async fn subscribe_notifications() -> anyhow::Result<Box<NotificationSubscription>> {
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

    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager.with_context(|ctx| {
        Ok(Box::new(NotificationSubscription::spawn(
            ctx.wallet.subscribe_notifications(),
            NotificationFilter::LightningPayment(payment_hash),
        )))
    })
}
