use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use bark::lock_manager::memory::MemoryLockManager;
use bark::onchain::OnchainWallet;
use bark::persist::sqlite::SqliteClient;
use bark::{OpenWalletArgs, RecoveryReport, RecoveryStatus, Wallet, WalletSeed};
use tokio::sync::{RwLock, oneshot};

use crate::utils::{DB_FILE, format_error_chain, merge_config_opts};
use crate::{CreateOpts, GLOBAL_WALLET_MANAGER, WalletContext, WalletManager};

pub struct RestoreWalletResult {
    pub report: Option<RecoveryReport>,
    pub error: Option<String>,
}

impl From<RecoveryStatus> for RestoreWalletResult {
    fn from(status: RecoveryStatus) -> Self {
        match status {
            RecoveryStatus::Completed(report) => Self {
                report: Some(report),
                error: None,
            },
            RecoveryStatus::Failed(error) => Self {
                report: None,
                error: Some(format_error_chain(&error)),
            },
            RecoveryStatus::NotRun => Self {
                report: None,
                error: Some("Ark server recovery scan did not run".into()),
            },
        }
    }
}

fn prepare_restore_destination(datadir: &Path) -> anyhow::Result<PathBuf> {
    if datadir.as_os_str().is_empty() {
        bail!("Restore destination must not be empty");
    }
    fs::create_dir_all(datadir).context("Failed to create restore destination")?;
    if fs::read_dir(datadir)?.next().transpose()?.is_some() {
        bail!(
            "Restore requires a new or empty directory; existing wallet data is never overwritten"
        );
    }
    let db_path = datadir.join(DB_FILE);
    // Reserve the database exclusively so a competing restore cannot open it.
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&db_path)
        .context("Restore database already exists or could not be created")?;
    Ok(db_path)
}

impl WalletManager {
    pub(crate) async fn restore_wallet_from_ark_server(
        &mut self,
        datadir: &Path,
        opts: CreateOpts,
    ) -> anyhow::Result<RestoreWalletResult> {
        if self.is_loaded() {
            bail!("Close the loaded wallet before restoring from the Ark server");
        }
        let mnemonic = opts.mnemonic.clone();
        let (config, network) = merge_config_opts(opts)?;
        let db_path = prepare_restore_destination(datadir)?;
        let db = Arc::new(SqliteClient::open(&db_path)?);
        let onchain_wallet = Arc::new(RwLock::new(
            OnchainWallet::load_or_create(network, mnemonic.to_seed(""), db.clone()).await?,
        ));
        let (sender, receiver) = oneshot::channel();
        let wallet = Wallet::open(
            network,
            WalletSeed::new_from_mnemonic(network, &mnemonic),
            config,
            OpenWalletArgs {
                run_daemon: false,
                persister: Some(db),
                lock_manager: Some(Box::new(MemoryLockManager::new())),
                onchain: Some(onchain_wallet.clone()),
                create_if_not_exists: true,
                skip_recovery: false,
                on_recovery_finished: Some(Box::new(move |status| {
                    let _ = sender.send(status);
                })),
                ..Default::default()
            },
        )
        .await
        .context("Failed to open wallet for Ark server recovery; preserve the restore directory")?;
        let status = receiver
            .await
            .context("Bark did not return a recovery result")?;
        let mut result = RestoreWalletResult::from(status);

        // Replay completion messages after the seed scan, before background sync starts.
        let delegated_sync = async {
            wallet
                .sync_mailbox()
                .await
                .context("Failed to sync restored wallet mailbox")?;
            wallet
                .sync_pending_rounds()
                .await
                .context("Failed to sync restored pending rounds")?;
            anyhow::Ok(())
        }
        .await;
        if let Err(error) = delegated_sync {
            let message = format_error_chain(&error);
            result.error = Some(match result.error {
                Some(previous) => format!("{previous}\n{message}"),
                None => message,
            });
        }
        // Even an incomplete scan can import funds. Keep that wallet available.
        self.context = Some(WalletContext::new(wallet, onchain_wallet, db_path));
        Ok(result)
    }
}

pub async fn restore_wallet_from_ark_server(
    datadir: &Path,
    opts: CreateOpts,
) -> anyhow::Result<RestoreWalletResult> {
    GLOBAL_WALLET_MANAGER
        .lock()
        .await
        .restore_wallet_from_ark_server(datadir, opts)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_preserves_existing_files_and_partial_databases() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["backup.txt", DB_FILE] {
            let path = dir.path().join(name);
            fs::write(&path, b"keep wallet data").unwrap();
            assert!(prepare_restore_destination(dir.path()).is_err());
            assert_eq!(fs::read(&path).unwrap(), b"keep wallet data");
            fs::remove_file(path).unwrap();
        }
        let db_path = prepare_restore_destination(dir.path()).unwrap();
        assert!(db_path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&db_path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
        assert!(prepare_restore_destination(dir.path()).is_err());
        assert!(prepare_restore_destination(Path::new("")).is_err());
        let new_dir = dir.path().join("new");
        assert!(prepare_restore_destination(&new_dir).unwrap().exists());
    }

    #[test]
    fn failed_and_unrun_scans_are_explicit() {
        for status in [
            RecoveryStatus::NotRun,
            RecoveryStatus::Failed(
                anyhow::anyhow!("mailbox unavailable").context("Recovery failed"),
            ),
        ] {
            let result = RestoreWalletResult::from(status);
            assert!(result.report.is_none());
            assert!(result.error.is_some());
        }
    }
}
