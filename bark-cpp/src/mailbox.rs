use std::sync::Arc;
use std::time::Duration;

use bark::Wallet;
use logger::log::{info, warn};
use tokio::task::JoinHandle;

use crate::TOKIO_RUNTIME;

const MAILBOX_RESTART_DELAY: Duration = Duration::from_secs(1);

pub fn spawn_mailbox_sync_task(wallet: Arc<Wallet>) -> JoinHandle<()> {
    TOKIO_RUNTIME.spawn(async move {
        info!("Starting background Bark mailbox processor");

        loop {
            match wallet.subscribe_process_mailbox_messages(None).await {
                Ok(()) => {
                    warn!("Bark mailbox stream dropped; restarting soon");
                }
                Err(error) => {
                    warn!(
                        "Background Bark mailbox processor exited with error: {error:#}"
                    );
                }
            }

            tokio::time::sleep(MAILBOX_RESTART_DELAY).await;
        }
    })
}
