use std::{path::Path, str::FromStr, sync::Arc};

use anyhow::{self, Context, bail};
use bark::{
    Config, Wallet as BarkWallet, WalletVtxo,
    actions::lightning::pay::{LightningSendState, Progress},
    ark::{
        Vtxo, VtxoId,
        bitcoin::{FeeRate, Network, secp256k1::PublicKey},
        lightning::PaymentHash,
    },
    lightning_invoice::Bolt11Invoice,
    lnurllib::lightning_address::LightningAddress,
    lock_manager::memory::MemoryLockManager,
    movement::{Movement, PaymentMethod},
    onchain::OnchainWallet,
    persist::sqlite::SqliteClient,
    round::RoundStatus,
    vtxo::VtxoState,
};

use bitcoin_ext::FeeRateExt;
use logger::log::{debug, error, info};
use tokio::fs;
use tonic::transport::Uri;

use crate::cxx::ffi;
use crate::{LightningPaymentResult, WalletContext};

pub(crate) const DB_FILE: &str = "db.sqlite";

pub(crate) fn format_error_chain(error: &anyhow::Error) -> String {
    error
        .chain()
        .enumerate()
        .map(|(index, cause)| {
            if index == 0 {
                cause.to_string()
            } else {
                format!("caused by: {}", cause)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) async fn lightning_payment_result_from_state(
    ctx: &WalletContext,
    payment_hash: PaymentHash,
    state: LightningSendState,
) -> anyhow::Result<LightningPaymentResult> {
    let (state, invoice, amount, htlc_vtxo_ids, movement_id, preimage) = match state {
        LightningSendState::Unknown => ("unknown", None, None, Vec::new(), None, None),
        LightningSendState::Paid(paid) => {
            ("paid", None, None, Vec::new(), None, Some(paid.preimage))
        }
        LightningSendState::InProgress(send) => match send.progress {
            Progress::Start => (
                "in_progress",
                Some(send.invoice),
                Some(send.payment_amount),
                Vec::new(),
                None,
                None,
            ),
            Progress::HtlcReceived(htlcs) | Progress::PaymentInitiated(htlcs) => (
                "in_progress",
                Some(send.invoice),
                Some(send.payment_amount),
                htlcs.vtxo_ids,
                Some(htlcs.movement_id.0),
                None,
            ),
            Progress::RevocableHtlcs { htlcs, .. } => (
                "in_progress",
                Some(send.invoice),
                Some(send.payment_amount),
                htlcs.vtxo_ids,
                Some(htlcs.movement_id.0),
                None,
            ),
        },
    };

    let mut htlc_vtxos = Vec::with_capacity(htlc_vtxo_ids.len());
    for vtxo_id in htlc_vtxo_ids {
        htlc_vtxos.push(ctx.wallet.get_vtxo_by_id(vtxo_id).await?);
    }

    Ok(LightningPaymentResult {
        state: state.to_string(),
        invoice,
        payment_hash,
        amount,
        htlc_vtxos,
        movement_id,
        preimage,
    })
}

impl ConfigOpts {
    pub fn merge_into(self, cfg: &mut Config) -> anyhow::Result<()> {
        if let Some(url) = self.ark {
            cfg.server_address = https_default_scheme(url).context("invalid ark url")?;
        }
        if let Some(v) = self.server_access_token {
            cfg.server_access_token = if v.is_empty() { None } else { Some(v) };
        }
        if let Some(v) = self.esplora {
            cfg.esplora_address = match v.is_empty() {
                true => None,
                false => Some(https_default_scheme(v).context("invalid esplora url")?),
            };
        }
        if let Some(v) = self.bitcoind {
            cfg.bitcoind_address = if v.is_empty() { None } else { Some(v) };
        }
        if let Some(v) = self.bitcoind_cookie {
            cfg.bitcoind_cookiefile = if v.is_empty() { None } else { Some(v.into()) };
        }
        if let Some(v) = self.bitcoind_user {
            cfg.bitcoind_user = if v.is_empty() { None } else { Some(v) };
        }
        if let Some(v) = self.bitcoind_pass {
            cfg.bitcoind_pass = if v.is_empty() { None } else { Some(v) };
        }
        cfg.htlc_recv_claim_delta = self.htlc_recv_claim_delta;
        cfg.vtxo_exit_margin = self.vtxo_exit_margin;
        cfg.round_tx_required_confirmations = self.round_tx_required_confirmations;
        cfg.vtxo_refresh_expiry_threshold = self.vtxo_refresh_expiry_threshold;
        cfg.fallback_fee_rate = self.fallback_fee_rate.map(FeeRate::from_sat_per_kvb_ceil);

        if cfg.esplora_address.is_none() && cfg.bitcoind_address.is_none() {
            bail!("Provide either an esplora or bitcoind url as chain source.");
        }

        Ok(())
    }
}

/// Parse the URL and add `https` scheme if no scheme is given.
pub fn https_default_scheme(url: String) -> anyhow::Result<String> {
    // default scheme to https if unset
    let mut uri_parts = Uri::from_str(&url).context("invalid url")?.into_parts();
    if uri_parts.authority.is_none() {
        bail!("invalid url '{}': missing authority", url);
    }
    if uri_parts.scheme.is_none() {
        uri_parts.scheme = Some("https".parse().unwrap());
        // because from_parts errors for missing PathAndQuery, set it
        uri_parts.path_and_query = Some(
            uri_parts
                .path_and_query
                .unwrap_or_else(|| "".parse().unwrap()),
        );
        let new = Uri::from_parts(uri_parts).unwrap();
        Ok(new.to_string())
    } else {
        Ok(url)
    }
}

#[derive(Debug, Clone)]
pub struct ConfigOpts {
    pub ark: Option<String>,
    pub server_access_token: Option<String>,

    /// The esplora HTTP API endpoint
    pub esplora: Option<String>,
    /// The bitcoind address
    pub bitcoind: Option<String>,
    pub bitcoind_cookie: Option<String>,
    pub bitcoind_user: Option<String>,
    pub bitcoind_pass: Option<String>,
    pub vtxo_refresh_expiry_threshold: u32,
    pub fallback_fee_rate: Option<u64>,
    pub htlc_recv_claim_delta: u16,
    pub vtxo_exit_margin: u16,
    pub round_tx_required_confirmations: u32,
}

#[derive(Debug, Clone)]
pub struct CreateOpts {
    /// Use regtest network.
    pub regtest: bool,
    /// Use signet network.
    pub signet: bool,
    /// Use bitcoin mainnet
    pub bitcoin: bool,

    /// Recover a wallet with an existing mnemonic.
    /// This currently only works for on-chain funds.
    pub mnemonic: bip39::Mnemonic,

    /// The wallet/mnemonic's birthday blockheight to start syncing when recovering.
    pub birthday_height: Option<u32>,

    pub config: ConfigOpts,
}

pub enum RefreshMode {
    DefaultThreshold,
    ThresholdBlocks(u32),
    ThresholdHours(u32),
    Counterparty,
    All,
    Specific(Vec<VtxoId>),
}

/// In this method we create the wallet and if it fails, the datadir will be wiped again.
pub(crate) async fn try_create_wallet(
    datadir: &Path,
    net: Network,
    config: Config,
    mnemonic: Option<bip39::Mnemonic>,
) -> anyhow::Result<()> {
    info!("Creating new bark Wallet at {}", datadir.display());

    fs::create_dir_all(datadir)
        .await
        .with_context(|| format!("can't create wallet datadir {}", datadir.display()))?;

    debug!("try_create_wallet datadir {:?} ", datadir);
    debug!("try_create_walletnetwork {:?}", net);
    debug!("try_create_wallet config {:?}", config);

    // open db
    // generate seed
    let mnemonic = mnemonic.unwrap_or_else(|| bip39::Mnemonic::generate(12).expect("12 is valid"));
    let seed = mnemonic.to_seed("");

    // open db
    let db_path = datadir.join(DB_FILE);
    let db = Arc::new(
        SqliteClient::open(db_path.clone())
            .with_context(|| format!("failed to open wallet database {}", db_path.display()))?,
    );

    debug!("Loading or creating onchain wallet");
    OnchainWallet::load_or_create(net, seed, db.clone())
        .await
        .context("failed to load or create onchain wallet")?;
    let lock_manager = Box::new(MemoryLockManager::new());
    debug!("Creating bark wallet with exit support");
    match BarkWallet::create_with_exits(&mnemonic, net, config, db, lock_manager, false)
        .await
        .context("error creating wallet")
    {
        Ok(_) => {
            info!("Created bark wallet successfully");
            Ok(())
        }
        Err(err) => {
            error!(
                "Failed to create bark wallet:\n{}",
                format_error_chain(&err)
            );
            Err(err)
        }
    }
}

/// Represents the different destinations for the `send` command
pub enum SendDestination {
    VtxoPubkey(PublicKey),
    Bolt11(Bolt11Invoice),
    LnAddress(LightningAddress),
    // Potentially add LNURL string later if direct LNURL payment is supported
}

/// Parses the destination string into a supported type.
pub fn parse_send_destination(destination: &str) -> anyhow::Result<SendDestination> {
    if let Ok(pk) = PublicKey::from_str(destination) {
        Ok(SendDestination::VtxoPubkey(pk))
    } else if let Ok(invoice) = Bolt11Invoice::from_str(destination) {
        // Further validation might be needed (e.g., expiry) but basic parsing is enough here
        Ok(SendDestination::Bolt11(invoice))
    } else if let Ok(lnaddr) = LightningAddress::from_str(destination) {
        Ok(SendDestination::LnAddress(lnaddr))
    } else {
        // Could check for raw lnurl string here if needed
        bail!(
            "Destination is not a valid VTXO pubkey, bolt11 invoice, or lightning address: {}",
            destination
        )
    }
}

/// Configuration of the Bark wallet.
/// Merge CreateOpts into ConfigOpts
pub fn merge_config_opts(opts: CreateOpts) -> anyhow::Result<(Config, Network)> {
    let net = match (opts.bitcoin, opts.signet, opts.regtest) {
        (true, false, false) => Network::Bitcoin,
        (false, true, false) => Network::Signet,
        (false, false, true) => Network::Regtest,
        _ => bail!("A network must be specified. Use either --signet, --regtest or --bitcoin"),
    };

    let mut config = Config::network_default(net);
    opts.config
        .clone()
        .merge_into(&mut config)
        .context("invalid configuration")?;

    Ok((config, net))
}

pub fn ffi_config_to_config(opts: ffi::CreateOpts) -> anyhow::Result<CreateOpts> {
    let config_opts = ConfigOpts {
        ark: Some(opts.config.ark),
        server_access_token: if opts.config.server_access_token.is_empty() {
            None
        } else {
            Some(opts.config.server_access_token)
        },
        esplora: Some(opts.config.esplora),
        bitcoind: Some(opts.config.bitcoind),
        bitcoind_cookie: Some(opts.config.bitcoind_cookie),
        bitcoind_user: Some(opts.config.bitcoind_user),
        bitcoind_pass: Some(opts.config.bitcoind_pass),
        vtxo_refresh_expiry_threshold: opts.config.vtxo_refresh_expiry_threshold,
        fallback_fee_rate: Some(opts.config.fallback_fee_rate),
        htlc_recv_claim_delta: opts.config.htlc_recv_claim_delta,
        vtxo_exit_margin: opts.config.vtxo_exit_margin,
        round_tx_required_confirmations: opts.config.round_tx_required_confirmations,
    };

    let create_opts = CreateOpts {
        regtest: opts.regtest,
        signet: opts.signet,
        bitcoin: opts.bitcoin,
        mnemonic: bip39::Mnemonic::from_str(&opts.mnemonic)?,
        birthday_height: unsafe { opts.birthday_height.as_ref().map(|r| *r) },
        config: config_opts,
    };

    Ok(create_opts)
}

pub fn wallet_vtxo_to_bark_vtxo(wallet_vtxo: WalletVtxo) -> crate::cxx::ffi::BarkVtxo {
    crate::cxx::ffi::BarkVtxo {
        amount: wallet_vtxo.vtxo.amount().to_sat(),
        expiry_height: wallet_vtxo.vtxo.expiry_height(),
        server_pubkey: wallet_vtxo.vtxo.server_pubkey().to_string(),
        exit_delta: wallet_vtxo.vtxo.exit_delta(),
        anchor_point: format!(
            "{}:{}",
            wallet_vtxo.vtxo.chain_anchor().txid,
            wallet_vtxo.vtxo.chain_anchor().vout
        ),
        point: format!(
            "{}:{}",
            wallet_vtxo.vtxo.point().txid,
            wallet_vtxo.vtxo.point().vout
        ),
        state: vtxo_state_name(&wallet_vtxo.state).to_string(),
    }
}

pub fn vtxo_to_bark_vtxo(vtxo: &Vtxo) -> crate::cxx::ffi::BarkVtxo {
    crate::cxx::ffi::BarkVtxo {
        amount: vtxo.amount().to_sat(),
        expiry_height: vtxo.expiry_height(),
        server_pubkey: vtxo.server_pubkey().to_string(),
        exit_delta: vtxo.exit_delta(),
        anchor_point: format!("{}:{}", vtxo.chain_anchor().txid, vtxo.chain_anchor().vout),
        point: format!("{}:{}", vtxo.point().txid, vtxo.point().vout),
        state: "unknown".to_string(),
    }
}

pub fn vtxo_state_name(state: &VtxoState) -> &'static str {
    match state {
        VtxoState::Spendable => "Spendable",
        VtxoState::Spent => "Spent",
        VtxoState::Locked { holder: _ } => "Locked",
    }
}

pub fn exit_state_name(state: &bark::exit::ExitState) -> &'static str {
    match state {
        bark::exit::ExitState::Start(..) => "Start",
        bark::exit::ExitState::Processing(..) => "Processing",
        bark::exit::ExitState::AwaitingDelta(..) => "AwaitingDelta",
        bark::exit::ExitState::Claimable(..) => "Claimable",
        bark::exit::ExitState::ClaimInProgress(..) => "ClaimInProgress",
        bark::exit::ExitState::Claimed(..) => "Claimed",
    }
}

fn payment_method_to_ffi(pm: &PaymentMethod) -> (String, String) {
    match pm {
        PaymentMethod::Ark(addr) => ("ark".to_string(), addr.to_string()),
        PaymentMethod::Bitcoin(addr) => {
            ("bitcoin".to_string(), addr.assume_checked_ref().to_string())
        }
        PaymentMethod::OutputScript(script) => {
            ("output-script".to_string(), script.to_hex_string())
        }
        PaymentMethod::Invoice(invoice) => ("invoice".to_string(), invoice.to_string()),
        PaymentMethod::Offer(offer) => ("offer".to_string(), offer.to_string()),
        PaymentMethod::LightningAddress(addr) => {
            ("lightning-address".to_string(), addr.to_string())
        }
        PaymentMethod::Custom(s) => ("custom".to_string(), s.clone()),
    }
}

pub fn movement_to_bark_movement(
    movement: &Movement,
) -> anyhow::Result<crate::cxx::ffi::BarkMovement> {
    let sent_to: Vec<crate::cxx::ffi::BarkMovementDestination> = movement
        .sent_to
        .iter()
        .map(|dest| {
            let (payment_method, destination) = payment_method_to_ffi(&dest.destination);
            crate::cxx::ffi::BarkMovementDestination {
                destination,
                payment_method,
                amount_sat: dest.amount.to_sat(),
            }
        })
        .collect();

    let received_on: Vec<crate::cxx::ffi::BarkMovementDestination> = movement
        .received_on
        .iter()
        .map(|dest| {
            let (payment_method, destination) = payment_method_to_ffi(&dest.destination);
            crate::cxx::ffi::BarkMovementDestination {
                destination,
                payment_method,
                amount_sat: dest.amount.to_sat(),
            }
        })
        .collect();

    let metadata_json = serde_json::to_string(&movement.metadata)?;

    let input_vtxos: Vec<String> = movement.input_vtxos.iter().map(|v| v.to_string()).collect();
    let output_vtxos: Vec<String> = movement
        .output_vtxos
        .iter()
        .map(|v| v.to_string())
        .collect();
    let exited_vtxos: Vec<String> = movement
        .exited_vtxos
        .iter()
        .map(|v| v.to_string())
        .collect();

    let created_at = movement.time.created_at.to_rfc3339();
    let updated_at = movement.time.updated_at.to_rfc3339();
    let completed_at = movement
        .time
        .completed_at
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_default();

    Ok(crate::cxx::ffi::BarkMovement {
        id: movement.id.0,
        status: match movement.status {
            bark::movement::MovementStatus::Pending => "pending",
            bark::movement::MovementStatus::Successful => "successful",
            bark::movement::MovementStatus::Failed => "failed",
            bark::movement::MovementStatus::Canceled => "canceled",
        }
        .to_string(),
        subsystem_name: movement.subsystem.name.clone(),
        subsystem_kind: movement.subsystem.kind.clone(),
        metadata_json,
        intended_balance_sat: movement.intended_balance.to_sat(),
        effective_balance_sat: movement.effective_balance.to_sat(),
        offchain_fee_sat: movement.offchain_fee.to_sat(),
        sent_to,
        received_on,
        input_vtxos,
        output_vtxos,
        exited_vtxos,
        created_at,
        updated_at,
        completed_at,
    })
}

pub fn round_status_to_ffi(status: RoundStatus) -> crate::cxx::ffi::RoundStatus {
    let is_final = status.is_final();
    let is_success = status.is_success();

    let (status_str, funding_txid, unsigned_funding_txids, error) = match &status {
        RoundStatus::Confirmed { funding_txid } => (
            "confirmed".to_string(),
            funding_txid.to_string(),
            Vec::new(),
            String::new(),
        ),
        RoundStatus::Unconfirmed { funding_txid } => (
            "unconfirmed".to_string(),
            funding_txid.to_string(),
            Vec::new(),
            String::new(),
        ),
        RoundStatus::Pending => (
            "pending".to_string(),
            String::new(),
            Vec::new(),
            String::new(),
        ),
        RoundStatus::Failed { error } => (
            "failed".to_string(),
            String::new(),
            Vec::new(),
            error.clone(),
        ),
        RoundStatus::Canceled => (
            "canceled".to_string(),
            String::new(),
            Vec::new(),
            String::new(),
        ),
    };

    crate::cxx::ffi::RoundStatus {
        status: status_str,
        funding_txid,
        unsigned_funding_txids,
        error,
        is_final,
        is_success,
    }
}
