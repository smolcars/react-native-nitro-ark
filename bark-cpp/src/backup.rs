use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use anyhow::{Context, bail};
use bark::WalletProperties;
use bark::persist::BarkPersister;
use bark::persist::sqlite::SqliteClient;
use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use tempfile::{Builder, NamedTempFile};

use crate::GLOBAL_WALLET_MANAGER;

const BACKUP_PAGES_PER_STEP: i32 = 128;
const BACKUP_STEP_PAUSE: Duration = Duration::from_millis(10);
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(10);

static SNAPSHOT_MUTEX: LazyLock<StdMutex<()>> = LazyLock::new(|| StdMutex::new(()));

#[derive(Debug, Clone)]
pub struct WalletSnapshotInfo {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub network: String,
    pub wallet_fingerprint: String,
    pub server_pubkey: Option<String>,
    pub mailbox_pubkey: Option<String>,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Default)]
pub struct WalletSnapshotExpectation {
    pub network: Option<String>,
    pub wallet_fingerprint: Option<String>,
    pub server_pubkey: Option<String>,
}

pub async fn create_wallet_snapshot(destination: &Path) -> anyhow::Result<WalletSnapshotInfo> {
    let (source, _) = loaded_wallet_details().await?;
    let destination = destination.to_path_buf();

    tokio::task::spawn_blocking(move || create_wallet_snapshot_blocking(&source, &destination))
        .await
        .context("wallet snapshot worker failed")?
}

pub async fn validate_wallet_snapshot(
    snapshot: &Path,
    expected: Option<WalletSnapshotExpectation>,
) -> anyhow::Result<WalletSnapshotInfo> {
    let active = loaded_wallet_details_optional().await?;
    let snapshot = snapshot.to_path_buf();

    tokio::task::spawn_blocking(move || {
        validate_wallet_snapshot_blocking(&snapshot, expected, active)
    })
    .await
    .context("wallet snapshot validation worker failed")?
}

async fn loaded_wallet_details() -> anyhow::Result<(PathBuf, WalletProperties)> {
    loaded_wallet_details_optional()
        .await?
        .context("Wallet not loaded")
}

async fn loaded_wallet_details_optional() -> anyhow::Result<Option<(PathBuf, WalletProperties)>> {
    let manager = GLOBAL_WALLET_MANAGER.lock().await;
    if !manager.is_loaded() {
        return Ok(None);
    }
    let (db_path, wallet) =
        manager.with_context_ref(|ctx| Ok((ctx.db_path.clone(), ctx.wallet.clone())))?;
    drop(manager);
    let properties = wallet.properties().await?;
    Ok(Some((db_path, properties)))
}

fn create_wallet_snapshot_blocking(
    source: &Path,
    destination: &Path,
) -> anyhow::Result<WalletSnapshotInfo> {
    let _snapshot_guard = SNAPSHOT_MUTEX
        .lock()
        .map_err(|_| anyhow::anyhow!("wallet snapshot lock is poisoned"))?;

    validate_destination(source, destination)?;
    let partial = temporary_snapshot_file(destination)?;
    sqlite_backup(source, partial.path())?;
    let info = inspect_snapshot(partial.path(), destination)?;
    validate_sqlite_integrity(partial.path())?;
    partial
        .as_file()
        .sync_all()
        .context("failed to flush wallet snapshot")?;
    partial
        .persist_noclobber(destination)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "failed to publish wallet snapshot at {}",
                destination.display()
            )
        })?;

    Ok(info)
}

fn validate_wallet_snapshot_blocking(
    snapshot: &Path,
    expected: Option<WalletSnapshotExpectation>,
    active: Option<(PathBuf, WalletProperties)>,
) -> anyhow::Result<WalletSnapshotInfo> {
    ensure_regular_snapshot_file(snapshot)?;
    if let Some((active_path, _)) = &active {
        if paths_refer_to_same_file(snapshot, active_path)? {
            bail!("Refusing to validate the live wallet database; create a snapshot first");
        }
    }

    validate_sqlite_integrity(snapshot)?;
    let source_schema_version = read_schema_version(snapshot)?;
    let supported_schema_version = supported_schema_version()?;
    if source_schema_version > supported_schema_version {
        bail!(
            "Snapshot schema version {} is newer than supported version {}",
            source_schema_version,
            supported_schema_version
        );
    }

    let parent = snapshot.parent().unwrap_or_else(|| Path::new("."));
    let migrated = Builder::new()
        .prefix(".nitro-ark-validation-")
        .suffix(".sqlite")
        .tempfile_in(parent)
        .context("failed to create temporary validation database")?;
    sqlite_backup(snapshot, migrated.path())?;

    let db = SqliteClient::open(migrated.path())
        .context("snapshot could not be opened or migrated by Bark")?;
    let properties = crate::TOKIO_RUNTIME
        .block_on(async {
            let properties = db
                .read_properties()
                .await?
                .context("snapshot does not contain initialized wallet properties")?;
            db.get_all_vtxos().await?;
            db.get_all_movements().await?;
            db.get_pending_round_state_ids().await?;
            db.get_all_pending_lightning_receives().await?;
            db.get_exit_vtxo_entries().await?;
            let _ = db.initialize_bdk_wallet().await?;
            anyhow::Ok(properties)
        })
        .context("snapshot contains unreadable Bark wallet data")?;

    let effective_expected = expected.or_else(|| {
        active.map(|(_, properties)| WalletSnapshotExpectation {
            network: Some(properties.network.to_string()),
            wallet_fingerprint: Some(properties.fingerprint.to_string()),
            server_pubkey: properties.server_pubkey.map(|key| key.to_string()),
        })
    });
    validate_expected_identity(&properties, effective_expected.as_ref())?;

    snapshot_info(snapshot, snapshot, &properties, source_schema_version)
}

fn validate_destination(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if destination.exists() {
        bail!(
            "Snapshot destination already exists: {}",
            destination.display()
        );
    }
    if paths_refer_to_same_file(source, destination)? {
        bail!("Snapshot destination cannot be the live wallet database");
    }

    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        bail!(
            "Snapshot destination parent does not exist: {}",
            parent.display()
        );
    }
    Ok(())
}

fn ensure_regular_snapshot_file(path: &Path) -> anyhow::Result<()> {
    let metadata = path
        .metadata()
        .with_context(|| format!("snapshot does not exist: {}", path.display()))?;
    if !metadata.is_file() {
        bail!("Snapshot path is not a regular file: {}", path.display());
    }
    if metadata.len() == 0 {
        bail!("Snapshot file is empty: {}", path.display());
    }
    Ok(())
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> anyhow::Result<bool> {
    if left == right {
        return Ok(true);
    }
    if left.exists() && right.exists() {
        return Ok(left.canonicalize()? == right.canonicalize()?);
    }
    Ok(false)
}

fn temporary_snapshot_file(destination: &Path) -> anyhow::Result<NamedTempFile> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("wallet-snapshot.sqlite");
    Builder::new()
        .prefix(&format!(".{file_name}.partial-"))
        .tempfile_in(parent)
        .context("failed to create partial wallet snapshot")
}

fn sqlite_backup(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let source_connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("failed to open source database {}", source.display()))?;
    source_connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;

    let mut destination_connection = Connection::open(destination).with_context(|| {
        format!(
            "failed to open destination database {}",
            destination.display()
        )
    })?;
    destination_connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;

    let backup = Backup::new(&source_connection, &mut destination_connection)
        .context("failed to initialize SQLite Online Backup")?;
    backup
        .run_to_completion(BACKUP_PAGES_PER_STEP, BACKUP_STEP_PAUSE, None)
        .context("SQLite Online Backup failed")?;
    drop(backup);
    destination_connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .context("failed to finalize snapshot journal")?;
    Ok(())
}

fn validate_sqlite_integrity(path: &Path) -> anyhow::Result<()> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("failed to open snapshot {}", path.display()))?;
    connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;

    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context("failed to run SQLite integrity_check")?;
    if integrity != "ok" {
        bail!("SQLite integrity_check failed: {integrity}");
    }

    let foreign_key_violation: Option<String> = connection
        .query_row(
            "SELECT printf('%s rowid=%s parent=%s', \"table\", rowid, parent) FROM pragma_foreign_key_check LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .context("failed to run SQLite foreign_key_check")?;
    if let Some(violation) = foreign_key_violation {
        bail!("SQLite foreign_key_check failed: {violation}");
    }
    Ok(())
}

fn supported_schema_version() -> anyhow::Result<u32> {
    let scratch = Builder::new()
        .prefix("nitro-ark-schema-")
        .suffix(".sqlite")
        .tempfile()
        .context("failed to create schema probe database")?;
    SqliteClient::open(scratch.path()).context("failed to initialize schema probe database")?;
    read_schema_version(scratch.path())
}

fn read_schema_version(path: &Path) -> anyhow::Result<u32> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("failed to read schema version from {}", path.display()))?;
    let version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(value), 0) FROM migrations",
            [],
            |row| row.get(0),
        )
        .context("snapshot does not contain a readable Bark migrations table")?;
    u32::try_from(version).context("snapshot schema version is invalid")
}

fn inspect_snapshot(
    snapshot_path: &Path,
    published_path: &Path,
) -> anyhow::Result<WalletSnapshotInfo> {
    let db = SqliteClient::open(snapshot_path).context("failed to inspect completed snapshot")?;
    let properties = crate::TOKIO_RUNTIME
        .block_on(db.read_properties())?
        .context("completed snapshot has no wallet properties")?;
    let schema_version = read_schema_version(snapshot_path)?;
    snapshot_info(snapshot_path, published_path, &properties, schema_version)
}

fn snapshot_info(
    content_path: &Path,
    reported_path: &Path,
    properties: &WalletProperties,
    schema_version: u32,
) -> anyhow::Result<WalletSnapshotInfo> {
    let metadata = content_path
        .metadata()
        .context("failed to read snapshot metadata")?;
    Ok(WalletSnapshotInfo {
        path: reported_path.to_string_lossy().into_owned(),
        size_bytes: metadata.len(),
        sha256: sha256_file(content_path)?,
        network: properties.network.to_string(),
        wallet_fingerprint: properties.fingerprint.to_string(),
        server_pubkey: properties.server_pubkey.map(|key| key.to_string()),
        mailbox_pubkey: properties.server_mailbox_pubkey.map(|key| key.to_string()),
        schema_version,
    })
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let file = File::open(path)
        .with_context(|| format!("failed to open snapshot for hashing: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_expected_identity(
    properties: &WalletProperties,
    expected: Option<&WalletSnapshotExpectation>,
) -> anyhow::Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };

    if let Some(network) = &expected.network
        && properties.network.to_string() != *network
    {
        bail!(
            "Snapshot network mismatch: expected {}, found {}",
            network,
            properties.network
        );
    }
    if let Some(fingerprint) = &expected.wallet_fingerprint
        && properties.fingerprint.to_string() != *fingerprint
    {
        bail!(
            "Snapshot wallet fingerprint mismatch: expected {}, found {}",
            fingerprint,
            properties.fingerprint
        );
    }
    if let Some(server_pubkey) = &expected.server_pubkey {
        let actual = properties.server_pubkey.map(|key| key.to_string());
        if actual.as_deref() != Some(server_pubkey.as_str()) {
            bail!(
                "Snapshot server public key mismatch: expected {}, found {}",
                server_pubkey,
                actual.as_deref().unwrap_or("none")
            );
        }
    }
    Ok(())
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use bark::ark::bitcoin::Network;
    use bark::ark::bitcoin::bip32::Fingerprint;
    use tempfile::tempdir;

    use super::*;

    fn create_test_wallet_db(path: &Path) -> WalletProperties {
        let db = SqliteClient::open(path).unwrap();
        let properties = WalletProperties {
            network: Network::Signet,
            fingerprint: Fingerprint::from([1, 2, 3, 4]),
            server_pubkey: None,
            server_mailbox_pubkey: None,
        };
        crate::TOKIO_RUNTIME
            .block_on(async {
                db.init_wallet(&properties).await?;
                let _ = db.initialize_bdk_wallet().await?;
                anyhow::Ok(())
            })
            .unwrap();
        properties
    }

    #[test]
    fn creates_and_validates_consistent_snapshot() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("db.sqlite");
        let destination = dir.path().join("snapshot.sqlite");
        let properties = create_test_wallet_db(&source);

        let created = create_wallet_snapshot_blocking(&source, &destination).unwrap();
        assert_eq!(created.path, destination.to_string_lossy());
        assert_eq!(created.network, "signet");
        assert_eq!(
            created.wallet_fingerprint,
            properties.fingerprint.to_string()
        );
        assert_eq!(created.sha256.len(), 64);
        assert_eq!(created.size_bytes, destination.metadata().unwrap().len());

        let validated = validate_wallet_snapshot_blocking(
            &destination,
            Some(WalletSnapshotExpectation {
                network: Some("signet".to_string()),
                wallet_fingerprint: Some(properties.fingerprint.to_string()),
                server_pubkey: None,
            }),
            None,
        )
        .unwrap();
        assert_eq!(validated.sha256, created.sha256);
    }

    #[test]
    fn refuses_to_overwrite_existing_destination() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("db.sqlite");
        let destination = dir.path().join("snapshot.sqlite");
        create_test_wallet_db(&source);
        File::create(&destination).unwrap();

        let error = create_wallet_snapshot_blocking(&source, &destination).unwrap_err();
        assert!(error.to_string().contains("already exists"));
    }

    #[test]
    fn rejects_corrupt_snapshot_and_identity_mismatch() {
        let dir = tempdir().unwrap();
        let corrupt = dir.path().join("corrupt.sqlite");
        std::fs::write(&corrupt, b"not a sqlite database").unwrap();
        assert!(validate_wallet_snapshot_blocking(&corrupt, None, None).is_err());

        let source = dir.path().join("db.sqlite");
        let snapshot = dir.path().join("snapshot.sqlite");
        create_test_wallet_db(&source);
        create_wallet_snapshot_blocking(&source, &snapshot).unwrap();
        let error = validate_wallet_snapshot_blocking(
            &snapshot,
            Some(WalletSnapshotExpectation {
                network: Some("bitcoin".to_string()),
                ..Default::default()
            }),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("network mismatch"));
    }
}
