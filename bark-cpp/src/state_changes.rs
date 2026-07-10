use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::Duration;

use logger::log::{info, warn};
use rusqlite::{Connection, OpenFlags};

use crate::GLOBAL_WALLET_MANAGER;
use crate::cxx::ffi;

const STATE_CHANGE_POLL_INTERVAL: Duration = Duration::from_millis(750);

pub struct StateChangeSubscription {
    rx: Receiver<ffi::StateChangeEvent>,
    task: Option<JoinHandle<()>>,
    active: Arc<AtomicBool>,
}

impl StateChangeSubscription {
    fn spawn(db_path: &Path) -> anyhow::Result<Self> {
        let connection = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut data_version = read_data_version(&connection)?;
        let (tx, rx) = mpsc::channel();
        let active = Arc::new(AtomicBool::new(true));
        let active_flag = Arc::clone(&active);

        tx.send(ffi::StateChangeEvent {
            sequence: 0,
            reason: "initial".to_string(),
        })?;

        let task = std::thread::spawn(move || {
            let mut sequence = 0_u64;
            while active_flag.load(Ordering::SeqCst) {
                std::thread::sleep(STATE_CHANGE_POLL_INTERVAL);
                if !active_flag.load(Ordering::SeqCst) {
                    break;
                }

                match read_data_version(&connection) {
                    Ok(next_version) if next_version != data_version => {
                        data_version = next_version;
                        sequence = sequence.saturating_add(1);
                        if tx
                            .send(ffi::StateChangeEvent {
                                sequence,
                                reason: "databaseChanged".to_string(),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        warn!("Wallet state change monitor failed: {error:#}");
                        sequence = sequence.saturating_add(1);
                        let _ = tx.send(ffi::StateChangeEvent {
                            sequence,
                            reason: "resyncRequired".to_string(),
                        });
                        break;
                    }
                }
            }
            active_flag.store(false, Ordering::SeqCst);
            info!("Wallet state change monitor stopped");
        });

        Ok(Self {
            rx,
            task: Some(task),
            active,
        })
    }

    pub fn stop(mut self: Pin<&mut Self>) -> anyhow::Result<()> {
        self.active.store(false, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub fn wait_next(
        self: Pin<&mut Self>,
        timeout_ms: u32,
    ) -> anyhow::Result<ffi::StateChangePollResult> {
        match self
            .rx
            .recv_timeout(Duration::from_millis(timeout_ms as u64))
        {
            Ok(event) => Ok(ffi::StateChangePollResult {
                has_event: true,
                is_active: self.is_active(),
                event,
            }),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(empty_poll_result(self.is_active())),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(empty_poll_result(false)),
        }
    }
}

impl Drop for StateChangeSubscription {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

fn empty_poll_result(is_active: bool) -> ffi::StateChangePollResult {
    ffi::StateChangePollResult {
        has_event: false,
        is_active,
        event: ffi::StateChangeEvent {
            sequence: 0,
            reason: String::new(),
        },
    }
}

fn read_data_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("PRAGMA data_version", [], |row| row.get(0))
}

pub async fn subscribe_wallet_state_changes() -> anyhow::Result<Box<StateChangeSubscription>> {
    let manager = GLOBAL_WALLET_MANAGER.lock().await;
    let db_path = manager.with_context_ref(|ctx| Ok(ctx.db_path.clone()))?;
    drop(manager);
    Ok(Box::new(StateChangeSubscription::spawn(&db_path)?))
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn reports_initial_and_committed_external_changes() {
        let file = NamedTempFile::new().unwrap();
        let connection = Connection::open(file.path()).unwrap();
        connection
            .execute_batch("CREATE TABLE state_test (value INTEGER NOT NULL);")
            .unwrap();

        let mut subscription = Box::pin(StateChangeSubscription::spawn(file.path()).unwrap());
        let initial = subscription.as_mut().wait_next(100).unwrap();
        assert!(initial.has_event);
        assert_eq!(initial.event.sequence, 0);
        assert_eq!(initial.event.reason, "initial");

        let writer = Connection::open(file.path()).unwrap();
        writer
            .execute("INSERT INTO state_test (value) VALUES (1)", [])
            .unwrap();

        let changed = subscription.as_mut().wait_next(2_000).unwrap();
        assert!(changed.has_event);
        assert_eq!(changed.event.sequence, 1);
        assert_eq!(changed.event.reason, "databaseChanged");
        subscription.as_mut().stop().unwrap();
    }

    #[test]
    fn ignores_rolled_back_changes() {
        let file = NamedTempFile::new().unwrap();
        let connection = Connection::open(file.path()).unwrap();
        connection
            .execute_batch("CREATE TABLE state_test (value INTEGER NOT NULL);")
            .unwrap();

        let mut subscription = Box::pin(StateChangeSubscription::spawn(file.path()).unwrap());
        let _ = subscription.as_mut().wait_next(100).unwrap();

        let mut writer = Connection::open(file.path()).unwrap();
        let transaction = writer.transaction().unwrap();
        transaction
            .execute("INSERT INTO state_test (value) VALUES (1)", [])
            .unwrap();
        transaction.rollback().unwrap();

        let result = subscription.as_mut().wait_next(1_000).unwrap();
        assert!(!result.has_event);
        subscription.as_mut().stop().unwrap();
    }
}
