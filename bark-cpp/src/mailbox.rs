use std::sync::Arc;
use std::time::Duration;

use bark::Wallet;
use logger::log::{info, warn};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::TOKIO_RUNTIME;

const MAILBOX_RESTART_DELAY: Duration = Duration::from_secs(1);

pub fn spawn_mailbox_sync_task(wallet: Arc<Wallet>, shutdown: CancellationToken) -> JoinHandle<()> {
    TOKIO_RUNTIME.spawn(async move {
        info!("Starting background Bark mailbox processor");

        loop {
            match wallet
                .subscribe_process_mailbox_messages(None, shutdown.clone())
                .await
            {
                Ok(()) if shutdown.is_cancelled() => {
                    info!("Background Bark mailbox processor shutdown complete");
                    break;
                }
                Ok(()) => {
                    warn!("Bark mailbox stream dropped; restarting soon");
                }
                Err(error) => {
                    warn!("Background Bark mailbox processor exited with error: {error:#}");
                }
            }

            tokio::select! {
                _ = tokio::time::sleep(MAILBOX_RESTART_DELAY) => {}
                _ = shutdown.cancelled() => {
                    info!("Stopping background Bark mailbox processor");
                    break;
                }
            }
        }
    })
}
