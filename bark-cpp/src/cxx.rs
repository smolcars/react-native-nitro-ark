use crate::cxx::ffi::{
    ArkoorPaymentResult, BarkFeeEstimate, BarkFeeRates, BarkMovement, BarkVtxo,
    OnchainPaymentResult, OnchainTransactionInfo,
};
pub use crate::subscriptions::NotificationSubscription;
use crate::{TOKIO_RUNTIME, utils};
use anyhow::{Context, Ok, bail};
use bark::ark::bitcoin::hex::DisplayHex;
use bark::ark::bitcoin::{Address, address};
use bark::ark::lightning::{self, PaymentHash};
use bdk_wallet::bitcoin::{self, FeeRate, network};
use bip39::Mnemonic;
use bitcoin_ext::FeeRateExt;
use logger::log::{self, info};

use std::path::Path;
use std::str::FromStr;

fn fee_rate_to_sat_per_vbyte(fee_rate: FeeRate) -> f64 {
    fee_rate.to_sat_per_kwu() as f64 / 250.0
}

fn full_ffi_error(context: &str, err: anyhow::Error) -> anyhow::Error {
    let message = utils::format_error_chain(&err);
    log::error!("{} failed:\n{}", context, message);
    anyhow::anyhow!(message)
}

fn ffi_boundary<T>(
    context: &'static str,
    f: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    f().map_err(|err| full_ffi_error(context, err))
}

#[cxx::bridge(namespace = "bark_cxx")]
pub(crate) mod ffi {

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BarkVtxo {
        id: String,
        amount: u64,
        expiry_height: u32,
        server_pubkey: String,
        exit_delta: u16,
        anchor_point: String,
        point: String,
        state: String,
    }

    pub struct BoardResult {
        vtxos: Vec<String>,
        funding_txid: String,
    }

    pub struct NewAddressResult {
        user_pubkey: String,
        ark_id: String,
        address: String,
    }

    pub struct Bolt11Invoice {
        bolt11_invoice: String,
        payment_secret: String,
        payment_hash: String,
    }

    pub struct LightningPaymentResult {
        pub state: String,
        pub invoice: String,
        pub payment_hash: String,
        pub has_amount: bool,
        pub amount: u64,
        pub htlc_vtxos: Vec<BarkVtxo>,
        pub has_movement_id: bool,
        pub movement_id: u32,
        pub preimage: String,
    }

    pub struct ArkoorPaymentResult {
        amount_sat: u64,
        destination_pubkey: String,
        vtxos: Vec<BarkVtxo>,
    }

    pub struct BarkFeeEstimate {
        gross_amount_sat: u64,
        fee_sat: u64,
        net_amount_sat: u64,
        vtxos_spent: Vec<String>,
    }

    pub struct BarkFeeRates {
        fast: f64,
        regular: f64,
        slow: f64,
    }

    pub struct OnchainPaymentResult {
        txid: String,
        amount_sat: u64,
        destination_address: String,
    }

    pub struct OnchainTransactionInfo {
        txid: String,
        tx_hex: String,
        has_onchain_fee: bool,
        onchain_fee_sat: u64,
        balance_change_sat: i64,
        has_confirmation: bool,
        confirmation_height: u32,
        confirmation_hash: String,
    }

    pub struct ExitBlockRefResult {
        height: u32,
        hash: String,
    }

    pub struct ExitTxOriginResult {
        kind: String,
        has_confirmed_in: bool,
        confirmed_in: ExitBlockRefResult,
    }

    pub struct ExitTxStatusResult {
        kind: String,
        txids: Vec<String>,
        child_txid: String,
        has_origin: bool,
        origin: ExitTxOriginResult,
        has_block: bool,
        block: ExitBlockRefResult,
    }

    pub struct ExitTxResult {
        txid: String,
        status: ExitTxStatusResult,
    }

    pub struct ExitStateDetailsResult {
        kind: String,
        tip_height: u32,
        transactions: Vec<ExitTxResult>,
        has_confirmed_block: bool,
        confirmed_block: ExitBlockRefResult,
        claimable_height: u32,
        has_claimable_since: bool,
        claimable_since: ExitBlockRefResult,
        has_last_scanned_block: bool,
        last_scanned_block: ExitBlockRefResult,
        claim_txid: String,
        txid: String,
        has_block: bool,
        block: ExitBlockRefResult,
    }

    pub struct ExitProgressStatusResult {
        vtxo_id: String,
        state: String,
        state_details: ExitStateDetailsResult,
        error: String,
    }

    pub struct ExitVtxoResult {
        vtxo_id: String,
        amount_sat: u64,
        state: String,
        state_details: ExitStateDetailsResult,
        history: Vec<String>,
        history_details: Vec<ExitStateDetailsResult>,
        txids: Vec<String>,
        is_claimable: bool,
        is_initialized: bool,
    }

    pub struct ExitTransactionPackageResult {
        exit_txid: String,
        exit_tx_hex: String,
        child_txid: String,
        child_tx_hex: String,
        child_origin: String,
        has_child: bool,
    }

    pub struct ExitStatusResult {
        vtxo_id: String,
        state: String,
        state_details: ExitStateDetailsResult,
        history: Vec<String>,
        history_details: Vec<ExitStateDetailsResult>,
        transactions: Vec<ExitTransactionPackageResult>,
    }

    pub struct CxxArkInfo {
        network: String,
        server_pubkey: String,
        mailbox_pubkey: String,
        round_interval: u64,
        nb_round_nonces: u16,
        vtxo_exit_delta: u16,
        vtxo_expiry_delta: u16,
        htlc_send_expiry_delta: u16,
        max_vtxo_amount: u64,
        required_board_confirmations: u8,
        min_board_amount: u64,
        ln_receive_anti_dos_required: bool,
    }

    pub struct ConfigOpts {
        ark: String,
        server_access_token: String,
        esplora: String,
        bitcoind: String,
        bitcoind_cookie: String,
        bitcoind_user: String,
        bitcoind_pass: String,
        vtxo_refresh_expiry_threshold: u32,
        fallback_fee_rate: u64,
        htlc_recv_claim_delta: u16,
        vtxo_exit_margin: u16,
        round_tx_required_confirmations: u32,
    }

    pub struct CreateOpts {
        regtest: bool,
        signet: bool,
        bitcoin: bool,
        mnemonic: String,
        birthday_height: *const u32,
        config: ConfigOpts,
    }

    pub struct SendManyOutput {
        destination: String,
        amount_sat: u64,
    }

    pub enum RefreshModeType {
        DefaultThreshold,
        ThresholdBlocks,
        ThresholdHours,
        Counterparty,
        All,
        Specific,
    }

    pub struct LightningReceive {
        pub payment_hash: String,
        pub payment_preimage: String,
        pub invoice: String,
        pub preimage_revealed_at: *const u64,
        pub finished_at: *const u64,
    }

    pub struct OffchainBalance {
        /// Coins that are spendable in the Ark, either in-round or out-of-round.
        pub spendable: u64,
        /// Coins that are in the process of being sent over Lightning.
        pub pending_lightning_send: u64,
        /// Coins that are in the process of being received over Lightning.
        pub claimable_lightning_receive: u64,
        /// Coins locked in a round.
        pub pending_in_round: u64,
        /// Coins that are in the process of unilaterally exiting the Ark.
        pub pending_exit: u64,
        /// Coins that are pending sufficient confirmations from board transactions.
        pub pending_board: u64,
    }

    pub struct OnChainBalance {
        /// All coinbase outputs not yet matured
        pub immature: u64,
        /// Unconfirmed UTXOs generated by a wallet tx
        pub trusted_pending: u64,
        /// Unconfirmed UTXOs received from an external wallet
        pub untrusted_pending: u64,
        /// Confirmed and immediately spendable balance
        pub confirmed: u64,
    }

    pub struct KeyPairResult {
        pub public_key: String,
        pub secret_key: String,
    }

    pub struct MailboxAuthorizationResult {
        pub mailbox_id: String,
        pub expiry: i64,
        pub encoded: String,
    }

    pub struct NotificationEvent {
        pub kind: String,
        pub has_movement: bool,
        pub movement: BarkMovement,
    }

    pub struct NotificationPollResult {
        pub has_event: bool,
        pub is_active: bool,
        pub event: NotificationEvent,
    }

    pub struct BarkMovementDestination {
        pub destination: String,
        pub payment_method: String,
        pub amount_sat: u64,
    }

    pub struct BarkMovement {
        pub id: u32,
        pub status: String,
        pub subsystem_name: String,
        pub subsystem_kind: String,
        pub metadata_json: String,
        pub intended_balance_sat: i64,
        pub effective_balance_sat: i64,
        pub offchain_fee_sat: u64,
        pub sent_to: Vec<BarkMovementDestination>,
        pub received_on: Vec<BarkMovementDestination>,
        pub input_vtxos: Vec<String>,
        pub output_vtxos: Vec<String>,
        pub exited_vtxos: Vec<String>,
        pub created_at: String,
        pub updated_at: String,
        pub completed_at: String,
    }

    pub struct PendingRoundStatus {
        pub round_id: u32,
        pub status: String,
        pub funding_txid: String,
        pub unsigned_funding_txids: Vec<String>,
        pub error: String,
        pub is_final: bool,
        pub is_success: bool,
    }

    extern "Rust" {
        type NotificationSubscription;

        fn init_logger();
        fn create_mnemonic() -> Result<String>;
        fn is_wallet_loaded() -> bool;
        fn close_wallet() -> Result<()>;
        fn get_ark_info() -> Result<CxxArkInfo>;
        fn offchain_balance() -> Result<OffchainBalance>;
        fn derive_store_next_keypair() -> Result<KeyPairResult>;
        fn peek_keypair(index: u32) -> Result<KeyPairResult>;
        fn new_address() -> Result<NewAddressResult>;
        fn peek_address(index: u32) -> Result<NewAddressResult>;
        fn sign_message(message: &str, index: u32) -> Result<String>;
        fn sign_messsage_with_mnemonic(
            message: &str,
            mnemonic: &str,
            network: &str,
            index: u32,
        ) -> Result<String>;
        fn derive_keypair_from_mnemonic(
            mnemonic: &str,
            network: &str,
            index: u32,
        ) -> Result<KeyPairResult>;
        fn verify_message(message: &str, signature: &str, public_key: &str) -> Result<bool>;
        fn history() -> Result<Vec<BarkMovement>>;
        fn vtxos() -> Result<Vec<BarkVtxo>>;
        fn dangerous_drop_vtxo(vtxo_id: &str) -> Result<()>;
        fn get_expiring_vtxos(threshold: u32) -> Result<Vec<BarkVtxo>>;
        fn get_first_expiring_vtxo_blockheight() -> Result<*const u32>;
        fn get_next_required_refresh_blockheight() -> Result<*const u32>;
        unsafe fn bolt11_invoice(
            amount_msat: u64,
            description: *const String,
        ) -> Result<Bolt11Invoice>;
        fn lightning_receive_status(payment_hash: String) -> Result<*const LightningReceive>;
        fn check_lightning_payment(
            payment_hash: String,
            wait: bool,
        ) -> Result<LightningPaymentResult>;
        fn sync_pending_boards() -> Result<()>;
        fn maintenance() -> Result<()>;
        fn maintenance_delegated() -> Result<()>;
        fn maintenance_with_onchain() -> Result<()>;
        fn maintenance_with_onchain_delegated() -> Result<()>;
        fn maintenance_refresh() -> Result<()>;
        fn refresh_server() -> Result<()>;
        fn sync() -> Result<()>;
        fn create_wallet(datadir: &str, opts: CreateOpts) -> Result<()>;
        fn load_wallet(datadir: &str, config: CreateOpts) -> Result<()>;
        fn board_amount(amount_sat: u64) -> Result<BoardResult>;
        fn board_all() -> Result<BoardResult>;
        fn validate_arkoor_address(address: &str) -> Result<()>;
        fn send_arkoor_payment(destination: &str, amount_sat: u64) -> Result<ArkoorPaymentResult>;
        fn estimate_arkoor_payment_fee(amount_sat: u64) -> Result<BarkFeeEstimate>;
        fn estimate_board_offchain_fee(amount_sat: u64) -> Result<BarkFeeEstimate>;
        fn estimate_lightning_send_fee(amount_sat: u64) -> Result<BarkFeeEstimate>;
        unsafe fn pay_lightning_invoice(
            destination: &str,
            amount_sat: *const u64,
            wait: bool,
        ) -> Result<LightningPaymentResult>;
        unsafe fn pay_lightning_offer(
            offer: &str,
            amount_sat: *const u64,
            wait: bool,
        ) -> Result<LightningPaymentResult>;
        fn pay_lightning_address(
            addr: &str,
            amount_sat: u64,
            comment: &str,
            wait: bool,
        ) -> Result<LightningPaymentResult>;
        unsafe fn progress_exits(
            fee_rate_sat_per_kvb: *const u64,
        ) -> Result<Vec<ExitProgressStatusResult>>;
        fn get_exit_vtxos() -> Result<Vec<ExitVtxoResult>>;
        fn list_claimable() -> Result<Vec<ExitVtxoResult>>;
        fn get_exit_status(
            vtxo_id: &str,
            include_history: bool,
            include_transactions: bool,
        ) -> Result<*const ExitStatusResult>;
        fn has_pending_exits() -> Result<bool>;
        fn pending_exit_total() -> Result<u64>;
        fn all_claimable_at_height() -> Result<*const u32>;
        unsafe fn drain_exits(
            vtxo_ids: Vec<String>,
            destination_address: &str,
            fee_rate_sat_per_kvb: *const u64,
        ) -> Result<String>;
        fn extract_transaction(psbt: &str) -> Result<String>;
        fn broadcast_transaction(tx_hex: &str) -> Result<String>;
        fn send_onchain(destination: &str, amount_sat: u64) -> Result<String>;
        fn estimate_send_onchain(destination: &str, amount_sat: u64) -> Result<BarkFeeEstimate>;
        fn offboard_specific(vtxo_ids: Vec<String>, destination_address: &str) -> Result<String>;
        fn offboard_all(destination_address: &str) -> Result<String>;
        fn estimate_offboard_all(destination_address: &str) -> Result<BarkFeeEstimate>;
        unsafe fn try_claim_lightning_receive(
            payment_hash: String,
            wait: bool,
            token: *const String,
        ) -> Result<LightningReceive>;
        fn try_claim_all_lightning_receives(wait: bool) -> Result<()>;
        fn start_exit_for_entire_wallet() -> Result<()>;
        fn start_exit_for_vtxos(vtxo_ids: Vec<String>) -> Result<()>;
        fn sync_exit() -> Result<()>;
        fn sync_pending_rounds() -> Result<Vec<PendingRoundStatus>>;
        fn mailbox_keypair() -> Result<KeyPairResult>;
        fn mailbox_authorization(authorization_expiry: i64) -> Result<MailboxAuthorizationResult>;
        fn subscribe_notifications() -> Result<Box<NotificationSubscription>>;
        fn subscribe_arkoor_address_movements(
            address: &str,
        ) -> Result<Box<NotificationSubscription>>;
        fn subscribe_lightning_payment_movements(
            payment_hash: &str,
        ) -> Result<Box<NotificationSubscription>>;
        fn stop(self: Pin<&mut NotificationSubscription>) -> Result<()>;
        fn is_active(self: &NotificationSubscription) -> bool;
        fn wait_next(
            self: Pin<&mut NotificationSubscription>,
            timeout_ms: u32,
        ) -> Result<NotificationPollResult>;

        // Onchain methods
        fn onchain_balance() -> Result<OnChainBalance>;
        fn onchain_sync() -> Result<()>;
        fn onchain_list_unspent() -> Result<String>;
        fn onchain_utxos() -> Result<String>;
        fn onchain_fee_rates() -> Result<BarkFeeRates>;
        fn onchain_transactions() -> Result<Vec<OnchainTransactionInfo>>;
        fn onchain_address() -> Result<String>;
        unsafe fn onchain_send(
            destination: &str,
            amount_sat: u64,
            fee_rate: *const u64,
        ) -> Result<OnchainPaymentResult>;
        unsafe fn onchain_drain(destination: &str, fee_rate: *const u64) -> Result<String>;
        unsafe fn onchain_send_many(
            outputs: Vec<SendManyOutput>,
            fee_rate: *const u64,
        ) -> Result<String>;
    }
}

pub(crate) fn init_logger() {
    crate::init_logger()
}

pub(crate) fn create_mnemonic() -> anyhow::Result<String> {
    ffi_boundary("create_mnemonic", crate::create_mnemonic)
}

pub(crate) fn is_wallet_loaded() -> bool {
    crate::TOKIO_RUNTIME.block_on(crate::is_wallet_loaded())
}

pub(crate) fn close_wallet() -> anyhow::Result<()> {
    ffi_boundary("close_wallet", || {
        crate::TOKIO_RUNTIME.block_on(crate::close_wallet())
    })
}

pub(crate) fn subscribe_notifications() -> anyhow::Result<Box<NotificationSubscription>> {
    ffi_boundary("subscribe_notifications", || {
        crate::TOKIO_RUNTIME.block_on(crate::subscribe_notifications())
    })
}

pub(crate) fn subscribe_arkoor_address_movements(
    address: &str,
) -> anyhow::Result<Box<NotificationSubscription>> {
    ffi_boundary("subscribe_arkoor_address_movements", || {
        crate::TOKIO_RUNTIME.block_on(crate::subscribe_arkoor_address_movements(address))
    })
}

pub(crate) fn subscribe_lightning_payment_movements(
    payment_hash: &str,
) -> anyhow::Result<Box<NotificationSubscription>> {
    ffi_boundary("subscribe_lightning_payment_movements", || {
        crate::TOKIO_RUNTIME.block_on(crate::subscribe_lightning_payment_movements(payment_hash))
    })
}

pub(crate) fn get_ark_info() -> anyhow::Result<ffi::CxxArkInfo> {
    ffi_boundary("get_ark_info", || {
        let info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;
        Ok(ffi::CxxArkInfo {
            network: info.network.to_string(),
            server_pubkey: info.server_pubkey.to_string(),
            mailbox_pubkey: info.mailbox_pubkey.to_string(),
            round_interval: info.round_interval.as_secs(),
            nb_round_nonces: info.nb_round_nonces as u16,
            vtxo_exit_delta: info.vtxo_exit_delta,
            vtxo_expiry_delta: info.vtxo_expiry_delta,
            htlc_send_expiry_delta: info.htlc_send_expiry_delta,
            max_vtxo_amount: info.max_vtxo_amount.map_or(0, |a| a.to_sat()),
            required_board_confirmations: info.required_board_confirmations as u8,
            min_board_amount: info.min_board_amount.to_sat(),
            ln_receive_anti_dos_required: info.ln_receive_anti_dos_required,
        })
    })
}

pub(crate) fn offchain_balance() -> anyhow::Result<ffi::OffchainBalance> {
    ffi_boundary("offchain_balance", || {
        let balance = crate::TOKIO_RUNTIME.block_on(crate::balance())?;
        Ok(ffi::OffchainBalance {
            spendable: balance.spendable.to_sat(),
            claimable_lightning_receive: balance.claimable_lightning_receive.to_sat(),
            pending_lightning_send: balance.pending_lightning_send.to_sat(),

            pending_in_round: balance.pending_in_round.to_sat(),
            pending_exit: balance.pending_exit.map_or(0, |a| a.to_sat()),
            pending_board: balance.pending_board.to_sat(),
        })
    })
}

pub(crate) fn derive_store_next_keypair() -> anyhow::Result<ffi::KeyPairResult> {
    ffi_boundary("derive_store_next_keypair", || {
        let keypair = crate::TOKIO_RUNTIME.block_on(crate::derive_store_next_keypair())?;
        Ok(ffi::KeyPairResult {
            public_key: keypair.public_key().to_string(),
            secret_key: keypair.secret_key().display_secret().to_string(),
        })
    })
}

pub(crate) fn peek_keypair(index: u32) -> anyhow::Result<ffi::KeyPairResult> {
    ffi_boundary("peek_keypair", || {
        let keypair = crate::TOKIO_RUNTIME.block_on(crate::peek_keypair(index))?;
        Ok(ffi::KeyPairResult {
            public_key: keypair.public_key().to_string(),
            secret_key: keypair.secret_key().display_secret().to_string(),
        })
    })
}

pub(crate) fn new_address() -> anyhow::Result<ffi::NewAddressResult> {
    ffi_boundary("new_address", || {
        let address = crate::TOKIO_RUNTIME.block_on(crate::new_address())?;
        Ok(ffi::NewAddressResult {
            user_pubkey: address.policy().user_pubkey().to_string(),
            ark_id: address.ark_id().to_string(),
            address: address.to_string(),
        })
    })
}

pub(crate) fn peek_address(index: u32) -> anyhow::Result<ffi::NewAddressResult> {
    ffi_boundary("peek_address", || {
        let address = crate::TOKIO_RUNTIME.block_on(crate::peek_address(index))?;
        Ok(ffi::NewAddressResult {
            user_pubkey: address.policy().user_pubkey().to_string(),
            ark_id: address.ark_id().to_string(),
            address: address.to_string(),
        })
    })
}

pub(crate) fn sign_message(message: &str, index: u32) -> anyhow::Result<String> {
    ffi_boundary("sign_message", || {
        let message = crate::TOKIO_RUNTIME
            .block_on(crate::sign_message(message, index))?
            .to_string();
        Ok(message)
    })
}

pub(crate) fn sign_messsage_with_mnemonic(
    message: &str,
    mnemonic: &str,
    network: &str,
    index: u32,
) -> anyhow::Result<String> {
    ffi_boundary("sign_messsage_with_mnemonic", || {
        let mnemonic = Mnemonic::from_str(mnemonic)
            .with_context(|| format!("Invalid mnemonic format: '{}'", mnemonic))?;

        let network = match network {
            "mainnet" => network::Network::Bitcoin,
            "regtest" => network::Network::Regtest,
            "signet" => network::Network::Signet,
            _ => bail!("Invalid network format: '{}'", network),
        };

        let message = crate::TOKIO_RUNTIME
            .block_on(crate::sign_messsage_with_mnemonic(
                message, mnemonic, network, index,
            ))?
            .to_string();
        Ok(message)
    })
}

pub(crate) fn derive_keypair_from_mnemonic(
    mnemonic: &str,
    network: &str,
    index: u32,
) -> anyhow::Result<ffi::KeyPairResult> {
    ffi_boundary("derive_keypair_from_mnemonic", || {
        let mnemonic = bip39::Mnemonic::from_str(mnemonic)
            .with_context(|| format!("Invalid mnemonic format: '{}'", mnemonic))?;
        let network = match network {
            "mainnet" => network::Network::Bitcoin,
            "regtest" => network::Network::Regtest,
            "signet" => network::Network::Signet,
            _ => bail!("Invalid network format: '{}'", network),
        };

        let keypair = crate::TOKIO_RUNTIME.block_on(crate::derive_keypair_from_mnemonic(
            mnemonic, network, index,
        ))?;

        Ok(ffi::KeyPairResult {
            public_key: keypair.public_key().to_string(),
            secret_key: keypair.secret_key().display_secret().to_string(),
        })
    })
}

pub(crate) fn verify_message(
    message: &str,
    signature: &str,
    public_key: &str,
) -> anyhow::Result<bool> {
    ffi_boundary("verify_message", || {
        let signature = bark::ark::bitcoin::secp256k1::ecdsa::Signature::from_str(signature)
            .with_context(|| format!("Invalid signature format: '{}'", signature))?;
        let public_key = bark::ark::bitcoin::secp256k1::PublicKey::from_str(public_key)
            .with_context(|| format!("Invalid public key format: '{}'", public_key))?;

        crate::TOKIO_RUNTIME.block_on(crate::verify_message(message, signature, &public_key))
    })
}

pub(crate) fn history() -> anyhow::Result<Vec<BarkMovement>> {
    ffi_boundary("history", || {
        let history = crate::TOKIO_RUNTIME.block_on(crate::history())?;
        fn fun_name(m: &bark::movement::Movement) -> Result<BarkMovement, anyhow::Error> {
            utils::movement_to_bark_movement(m)
        }

        history.iter().map(fun_name).collect()
    })
}

pub(crate) fn vtxos() -> anyhow::Result<Vec<BarkVtxo>> {
    ffi_boundary("vtxos", || {
        let vtxos = crate::TOKIO_RUNTIME.block_on(crate::vtxos())?;
        Ok(vtxos
            .into_iter()
            .map(utils::wallet_vtxo_to_bark_vtxo)
            .collect())
    })
}

pub(crate) fn dangerous_drop_vtxo(vtxo_id: &str) -> anyhow::Result<()> {
    ffi_boundary("dangerous_drop_vtxo", || {
        let vtxo_id = bark::ark::VtxoId::from_str(vtxo_id)
            .with_context(|| format!("Invalid VTXO ID: {vtxo_id}"))?;
        crate::TOKIO_RUNTIME.block_on(crate::dangerous_drop_vtxo(vtxo_id))
    })
}

pub(crate) fn get_expiring_vtxos(threshold: u32) -> anyhow::Result<Vec<BarkVtxo>> {
    ffi_boundary("get_expiring_vtxos", || {
        let expiring_vtxos = crate::TOKIO_RUNTIME.block_on(crate::get_expiring_vtxos(threshold))?;
        Ok(expiring_vtxos
            .into_iter()
            .map(utils::wallet_vtxo_to_bark_vtxo)
            .collect())
    })
}

pub(crate) fn get_first_expiring_vtxo_blockheight() -> anyhow::Result<*const u32> {
    ffi_boundary("get_first_expiring_vtxo_blockheight", || {
        let blockheight =
            crate::TOKIO_RUNTIME.block_on(crate::get_first_expiring_vtxo_blockheight())?;
        match blockheight {
            Some(height) => Ok(Box::into_raw(Box::new(height)) as *const u32),
            None => Ok(std::ptr::null()),
        }
    })
}

pub(crate) fn get_next_required_refresh_blockheight() -> anyhow::Result<*const u32> {
    ffi_boundary("get_next_required_refresh_blockheight", || {
        let blockheight =
            crate::TOKIO_RUNTIME.block_on(crate::get_next_required_refresh_blockheight())?;
        match blockheight {
            Some(height) => Ok(Box::into_raw(Box::new(height)) as *const u32),
            None => Ok(std::ptr::null()),
        }
    })
}

pub(crate) fn bolt11_invoice(
    amount_msat: u64,
    description: *const String,
) -> anyhow::Result<ffi::Bolt11Invoice> {
    ffi_boundary("bolt11_invoice", || {
        let description_opt = unsafe { description.as_ref().map(|s| s.clone()) };
        let invoice =
            crate::TOKIO_RUNTIME.block_on(crate::bolt11_invoice(amount_msat, description_opt))?;
        Ok(ffi::Bolt11Invoice {
            bolt11_invoice: invoice.to_string(),
            payment_secret: invoice.payment_secret().to_string(),
            payment_hash: invoice.payment_hash().to_string(),
        })
    })
}

pub(crate) fn lightning_receive_status(
    payment_hash: String,
) -> anyhow::Result<*const ffi::LightningReceive> {
    ffi_boundary("lightning_receive_status", || {
        let payment = bark::ark::lightning::PaymentHash::from_str(&payment_hash)
            .with_context(|| format!("Invalid payment hash format: '{}'", payment_hash))?;
        let status = crate::TOKIO_RUNTIME.block_on(crate::lightning_receive_status(payment))?;

        if status.is_none() {
            return Ok(std::ptr::null());
        }

        let status = status.unwrap();
        let status = Box::new(ffi::LightningReceive {
            payment_hash: status.payment_hash.to_string(),
            payment_preimage: status.payment_preimage.to_string(),
            invoice: status.invoice.to_string(),
            preimage_revealed_at: status.preimage_revealed_at.map_or(std::ptr::null(), |v| {
                Box::into_raw(Box::new(v.timestamp() as u64))
            }),
            finished_at: status.finished_at.map_or(std::ptr::null(), |v| {
                Box::into_raw(Box::new(v.timestamp() as u64))
            }),
        });
        Ok(Box::into_raw(status))
    })
}

pub(crate) fn sync_pending_boards() -> anyhow::Result<()> {
    ffi_boundary("sync_pending_boards", || {
        crate::TOKIO_RUNTIME.block_on(crate::sync_pending_boards())
    })
}

pub(crate) fn maintenance() -> anyhow::Result<()> {
    ffi_boundary("maintenance", || {
        crate::TOKIO_RUNTIME.block_on(crate::maintenance())
    })
}

pub(crate) fn maintenance_delegated() -> anyhow::Result<()> {
    ffi_boundary("maintenance_delegated", || {
        crate::TOKIO_RUNTIME.block_on(crate::maintenance_delegated())
    })
}

pub(crate) fn maintenance_with_onchain() -> anyhow::Result<()> {
    ffi_boundary("maintenance_with_onchain", || {
        crate::TOKIO_RUNTIME.block_on(crate::maintenance_with_onchain())
    })
}

pub(crate) fn maintenance_with_onchain_delegated() -> anyhow::Result<()> {
    ffi_boundary("maintenance_with_onchain_delegated", || {
        crate::TOKIO_RUNTIME.block_on(crate::maintenance_with_onchain_delegated())
    })
}

pub(crate) fn maintenance_refresh() -> anyhow::Result<()> {
    ffi_boundary("maintenance_refresh", || {
        crate::TOKIO_RUNTIME.block_on(crate::maintenance_refresh())
    })
}

pub(crate) fn refresh_server() -> anyhow::Result<()> {
    ffi_boundary("refresh_server", || {
        crate::TOKIO_RUNTIME.block_on(crate::refresh_server())
    })
}

pub(crate) fn sync() -> anyhow::Result<()> {
    ffi_boundary("sync", || crate::TOKIO_RUNTIME.block_on(crate::sync()))
}

pub(crate) fn create_wallet(datadir: &str, opts: ffi::CreateOpts) -> anyhow::Result<()> {
    ffi_boundary("create_wallet", || {
        let create_opts = utils::ffi_config_to_config(opts)?;

        log::info!("Creating wallet with options: {:?}", create_opts);

        crate::TOKIO_RUNTIME.block_on(crate::create_wallet(Path::new(datadir), create_opts))
    })
}

pub(crate) fn load_wallet(datadir: &str, config: ffi::CreateOpts) -> anyhow::Result<()> {
    ffi_boundary("load_wallet", || {
        let mnemonic = bip39::Mnemonic::from_str(&config.mnemonic)
            .with_context(|| format!("Invalid mnemonic format: '{}'", config.mnemonic))?;

        log::info!("Loading wallet with datadir: {}", datadir);

        let create_opts = utils::ffi_config_to_config(config)?;

        let (config, _) = utils::merge_config_opts(create_opts)?;

        crate::TOKIO_RUNTIME.block_on(crate::load_wallet(Path::new(datadir), mnemonic, config))
    })
}

pub(crate) fn board_amount(amount_sat: u64) -> anyhow::Result<ffi::BoardResult> {
    ffi_boundary("board_amount", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let board_result = crate::TOKIO_RUNTIME.block_on(crate::board_amount(amount))?;

        Ok(ffi::BoardResult {
            vtxos: board_result
                .vtxos
                .iter()
                .map(|vtxo| vtxo.to_string())
                .collect(),
            funding_txid: board_result.funding_tx.compute_txid().to_string(),
        })
    })
}

pub(crate) fn board_all() -> anyhow::Result<ffi::BoardResult> {
    ffi_boundary("board_all", || {
        let board_result = crate::TOKIO_RUNTIME.block_on(crate::board_all())?;

        Ok(ffi::BoardResult {
            vtxos: board_result
                .vtxos
                .iter()
                .map(|vtxo| vtxo.to_string())
                .collect(),
            funding_txid: board_result.funding_tx.compute_txid().to_string(),
        })
    })
}

pub(crate) fn validate_arkoor_address(address: &str) -> anyhow::Result<()> {
    ffi_boundary("validate_arkoor_address", || {
        let address = bark::ark::Address::from_str(address)
            .with_context(|| format!("Invalid address format: '{}'", address))?;
        crate::TOKIO_RUNTIME.block_on(crate::validate_arkoor_address(address))
    })
}

pub(crate) fn send_arkoor_payment(
    destination: &str,
    amount_sat: u64,
) -> anyhow::Result<ArkoorPaymentResult> {
    ffi_boundary("send_arkoor_payment", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let dest = bark::ark::Address::from_str(destination)
            .with_context(|| format!("Invalid destination address format: '{}'", destination))?;
        let oor_result = crate::TOKIO_RUNTIME.block_on(crate::send_arkoor_payment(dest, amount))?;

        Ok(ArkoorPaymentResult {
            vtxos: oor_result.iter().map(utils::vtxo_to_bark_vtxo).collect(),
            destination_pubkey: destination.to_string(),
            amount_sat,
        })
    })
}

pub(crate) fn estimate_arkoor_payment_fee(amount_sat: u64) -> anyhow::Result<BarkFeeEstimate> {
    ffi_boundary("estimate_arkoor_payment_fee", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let estimate = crate::TOKIO_RUNTIME.block_on(crate::estimate_arkoor_payment_fee(amount))?;

        Ok(BarkFeeEstimate {
            gross_amount_sat: estimate.gross_amount.to_sat(),
            fee_sat: estimate.fee.to_sat(),
            net_amount_sat: estimate.net_amount.to_sat(),
            vtxos_spent: estimate
                .vtxos_spent
                .into_iter()
                .map(|vtxo_id| vtxo_id.to_string())
                .collect(),
        })
    })
}

pub(crate) fn estimate_board_offchain_fee(amount_sat: u64) -> anyhow::Result<BarkFeeEstimate> {
    ffi_boundary("estimate_board_offchain_fee", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let estimate = crate::TOKIO_RUNTIME.block_on(crate::estimate_board_offchain_fee(amount))?;

        Ok(BarkFeeEstimate {
            gross_amount_sat: estimate.gross_amount.to_sat(),
            fee_sat: estimate.fee.to_sat(),
            net_amount_sat: estimate.net_amount.to_sat(),
            vtxos_spent: estimate
                .vtxos_spent
                .into_iter()
                .map(|vtxo_id| vtxo_id.to_string())
                .collect(),
        })
    })
}

pub(crate) fn estimate_lightning_send_fee(amount_sat: u64) -> anyhow::Result<BarkFeeEstimate> {
    ffi_boundary("estimate_lightning_send_fee", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let estimate = crate::TOKIO_RUNTIME.block_on(crate::estimate_lightning_send_fee(amount))?;

        Ok(BarkFeeEstimate {
            gross_amount_sat: estimate.gross_amount.to_sat(),
            fee_sat: estimate.fee.to_sat(),
            net_amount_sat: estimate.net_amount.to_sat(),
            vtxos_spent: estimate
                .vtxos_spent
                .into_iter()
                .map(|vtxo_id| vtxo_id.to_string())
                .collect(),
        })
    })
}

fn lightning_payment_result_to_ffi(
    payment: crate::LightningPaymentResult,
) -> ffi::LightningPaymentResult {
    ffi::LightningPaymentResult {
        state: payment.state,
        invoice: payment
            .invoice
            .as_ref()
            .map_or(String::new(), ToString::to_string),
        payment_hash: payment.payment_hash.to_string(),
        has_amount: payment.amount.is_some(),
        amount: payment.amount.map_or(0, |amount| amount.to_sat()),
        htlc_vtxos: payment
            .htlc_vtxos
            .into_iter()
            .map(utils::wallet_vtxo_to_bark_vtxo)
            .collect(),
        has_movement_id: payment.movement_id.is_some(),
        movement_id: payment.movement_id.unwrap_or(0),
        preimage: payment
            .preimage
            .map_or(String::new(), |p| p.to_lower_hex_string()),
    }
}

pub(crate) fn pay_lightning_invoice(
    destination: &str,
    amount_sat: *const u64,
    wait: bool,
) -> anyhow::Result<ffi::LightningPaymentResult> {
    ffi_boundary("pay_lightning_invoice", || {
        let amount_opt =
            unsafe { amount_sat.as_ref().map(|r| *r) }.map(bark::ark::bitcoin::Amount::from_sat);

        let invoice = lightning::Invoice::from_str(destination)?;

        let send_result = crate::TOKIO_RUNTIME
            .block_on(crate::pay_lightning_invoice(invoice, amount_opt, wait))?;

        Ok(lightning_payment_result_to_ffi(send_result))
    })
}

pub(crate) fn pay_lightning_offer(
    offer: &str,
    amount_sat: *const u64,
    wait: bool,
) -> anyhow::Result<ffi::LightningPaymentResult> {
    ffi_boundary("pay_lightning_offer", || {
        let amount_opt =
            unsafe { amount_sat.as_ref().map(|r| *r) }.map(bark::ark::bitcoin::Amount::from_sat);

        let offer = lightning::Offer::from_str(offer)
            .map_err(|err| anyhow::anyhow!("Failed to parse bolt12 offer: {:?}", err))?;

        let send_result = crate::TOKIO_RUNTIME.block_on(crate::pay_lightning_offer(
            offer.clone(),
            amount_opt,
            wait,
        ))?;

        Ok(lightning_payment_result_to_ffi(send_result))
    })
}

pub(crate) fn pay_lightning_address(
    addr: &str,
    amount_sat: u64,
    comment: &str,
    wait: bool,
) -> anyhow::Result<ffi::LightningPaymentResult> {
    ffi_boundary("pay_lightning_address", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let comment_opt = if comment.is_empty() {
            None
        } else {
            Some(comment)
        };
        let send_result = crate::TOKIO_RUNTIME.block_on(crate::pay_lightning_address(
            addr,
            amount,
            comment_opt,
            wait,
        ))?;

        Ok(lightning_payment_result_to_ffi(send_result))
    })
}

fn empty_exit_block_ref() -> ffi::ExitBlockRefResult {
    ffi::ExitBlockRefResult {
        height: 0,
        hash: String::new(),
    }
}

fn exit_block_ref_to_ffi(block: bitcoin_ext::BlockRef) -> ffi::ExitBlockRefResult {
    ffi::ExitBlockRefResult {
        height: block.height,
        hash: block.hash.to_string(),
    }
}

fn empty_exit_tx_origin() -> ffi::ExitTxOriginResult {
    ffi::ExitTxOriginResult {
        kind: String::new(),
        has_confirmed_in: false,
        confirmed_in: empty_exit_block_ref(),
    }
}

fn exit_tx_origin_to_ffi(origin: &bark::exit::ExitTxOrigin) -> ffi::ExitTxOriginResult {
    match origin {
        bark::exit::ExitTxOrigin::Wallet { confirmed_in } => ffi::ExitTxOriginResult {
            kind: "wallet".to_string(),
            has_confirmed_in: confirmed_in.is_some(),
            confirmed_in: confirmed_in.map_or_else(empty_exit_block_ref, exit_block_ref_to_ffi),
        },
        bark::exit::ExitTxOrigin::Mempool => ffi::ExitTxOriginResult {
            kind: "mempool".to_string(),
            has_confirmed_in: false,
            confirmed_in: empty_exit_block_ref(),
        },
        bark::exit::ExitTxOrigin::Block { confirmed_in } => ffi::ExitTxOriginResult {
            kind: "block".to_string(),
            has_confirmed_in: true,
            confirmed_in: exit_block_ref_to_ffi(*confirmed_in),
        },
    }
}

fn empty_exit_tx_status() -> ffi::ExitTxStatusResult {
    ffi::ExitTxStatusResult {
        kind: String::new(),
        txids: Vec::new(),
        child_txid: String::new(),
        has_origin: false,
        origin: empty_exit_tx_origin(),
        has_block: false,
        block: empty_exit_block_ref(),
    }
}

fn exit_tx_status_to_ffi(status: &bark::exit::ExitTxStatus) -> ffi::ExitTxStatusResult {
    match status {
        bark::exit::ExitTxStatus::VerifyInputs => ffi::ExitTxStatusResult {
            kind: "verify-inputs".to_string(),
            ..empty_exit_tx_status()
        },
        bark::exit::ExitTxStatus::AwaitingInputConfirmation { txids } => {
            let mut txids = txids.iter().map(ToString::to_string).collect::<Vec<_>>();
            txids.sort();

            ffi::ExitTxStatusResult {
                kind: "awaiting-input-confirmation".to_string(),
                txids,
                ..empty_exit_tx_status()
            }
        }
        bark::exit::ExitTxStatus::AwaitingCpfpBroadcast => ffi::ExitTxStatusResult {
            kind: "awaiting-cpfp-broadcast".to_string(),
            ..empty_exit_tx_status()
        },
        bark::exit::ExitTxStatus::AwaitingConfirmation { child_txid, origin } => {
            ffi::ExitTxStatusResult {
                kind: "awaiting-confirmation".to_string(),
                child_txid: child_txid.to_string(),
                has_origin: true,
                origin: exit_tx_origin_to_ffi(origin),
                ..empty_exit_tx_status()
            }
        }
        bark::exit::ExitTxStatus::Confirmed {
            child_txid,
            block,
            origin,
        } => ffi::ExitTxStatusResult {
            kind: "confirmed".to_string(),
            child_txid: child_txid.to_string(),
            has_origin: true,
            origin: exit_tx_origin_to_ffi(origin),
            has_block: true,
            block: exit_block_ref_to_ffi(*block),
            ..empty_exit_tx_status()
        },
    }
}

fn exit_tx_to_ffi(tx: &bark::exit::ExitTx) -> ffi::ExitTxResult {
    let bark::exit::ExitTx { txid, status } = tx;
    ffi::ExitTxResult {
        txid: txid.to_string(),
        status: exit_tx_status_to_ffi(status),
    }
}

fn empty_exit_state_details(kind: &str, tip_height: u32) -> ffi::ExitStateDetailsResult {
    ffi::ExitStateDetailsResult {
        kind: kind.to_string(),
        tip_height,
        transactions: Vec::new(),
        has_confirmed_block: false,
        confirmed_block: empty_exit_block_ref(),
        claimable_height: 0,
        has_claimable_since: false,
        claimable_since: empty_exit_block_ref(),
        has_last_scanned_block: false,
        last_scanned_block: empty_exit_block_ref(),
        claim_txid: String::new(),
        txid: String::new(),
        has_block: false,
        block: empty_exit_block_ref(),
    }
}

fn exit_state_details_to_ffi(state: &bark::exit::ExitState) -> ffi::ExitStateDetailsResult {
    match state {
        bark::exit::ExitState::Start(state) => {
            let bark::exit::ExitStartState { tip_height } = state;
            empty_exit_state_details("start", *tip_height)
        }
        bark::exit::ExitState::Processing(state) => {
            let bark::exit::ExitProcessingState {
                tip_height,
                transactions,
            } = state;
            ffi::ExitStateDetailsResult {
                transactions: transactions.iter().map(exit_tx_to_ffi).collect(),
                ..empty_exit_state_details("processing", *tip_height)
            }
        }
        bark::exit::ExitState::AwaitingDelta(state) => {
            let bark::exit::ExitAwaitingDeltaState {
                tip_height,
                confirmed_block,
                claimable_height,
            } = state;
            ffi::ExitStateDetailsResult {
                has_confirmed_block: true,
                confirmed_block: exit_block_ref_to_ffi(*confirmed_block),
                claimable_height: *claimable_height,
                ..empty_exit_state_details("awaiting-delta", *tip_height)
            }
        }
        bark::exit::ExitState::Claimable(state) => {
            let bark::exit::ExitClaimableState {
                tip_height,
                claimable_since,
                last_scanned_block,
            } = state;
            ffi::ExitStateDetailsResult {
                has_claimable_since: true,
                claimable_since: exit_block_ref_to_ffi(*claimable_since),
                has_last_scanned_block: last_scanned_block.is_some(),
                last_scanned_block: last_scanned_block
                    .map_or_else(empty_exit_block_ref, exit_block_ref_to_ffi),
                ..empty_exit_state_details("claimable", *tip_height)
            }
        }
        bark::exit::ExitState::ClaimInProgress(state) => {
            let bark::exit::ExitClaimInProgressState {
                tip_height,
                claimable_since,
                claim_txid,
            } = state;
            ffi::ExitStateDetailsResult {
                has_claimable_since: true,
                claimable_since: exit_block_ref_to_ffi(*claimable_since),
                claim_txid: claim_txid.to_string(),
                ..empty_exit_state_details("claim-in-progress", *tip_height)
            }
        }
        bark::exit::ExitState::Claimed(state) => {
            let bark::exit::ExitClaimedState {
                tip_height,
                txid,
                block,
            } = state;
            ffi::ExitStateDetailsResult {
                txid: txid.to_string(),
                has_block: true,
                block: exit_block_ref_to_ffi(*block),
                ..empty_exit_state_details("claimed", *tip_height)
            }
        }
    }
}

pub(crate) fn progress_exits(
    fee_rate_sat_per_kvb: *const u64,
) -> anyhow::Result<Vec<ffi::ExitProgressStatusResult>> {
    ffi_boundary("progress_exits", || {
        let fee_rate = unsafe {
            fee_rate_sat_per_kvb
                .as_ref()
                .copied()
                .map(FeeRate::from_sat_per_kvb_ceil)
        };
        let statuses = TOKIO_RUNTIME.block_on(crate::progress_exits(fee_rate))?;

        statuses
            .into_iter()
            .map(|status| {
                Ok(ffi::ExitProgressStatusResult {
                    vtxo_id: status.vtxo_id.to_string(),
                    state: utils::exit_state_name(&status.state).to_string(),
                    state_details: exit_state_details_to_ffi(&status.state),
                    error: status.error.map_or(String::new(), |error| {
                        utils::format_error_chain(&anyhow::Error::new(error))
                    }),
                })
            })
            .collect()
    })
}

pub(crate) fn get_exit_vtxos() -> anyhow::Result<Vec<ffi::ExitVtxoResult>> {
    ffi_boundary("get_exit_vtxos", || {
        let exits = TOKIO_RUNTIME.block_on(crate::get_exit_vtxos())?;

        exits.into_iter().map(exit_vtxo_to_ffi).collect()
    })
}

pub(crate) fn list_claimable() -> anyhow::Result<Vec<ffi::ExitVtxoResult>> {
    ffi_boundary("list_claimable", || {
        let exits = TOKIO_RUNTIME.block_on(crate::list_claimable())?;

        exits.into_iter().map(exit_vtxo_to_ffi).collect()
    })
}

fn exit_vtxo_to_ffi(exit: bark::exit::ExitVtxo) -> anyhow::Result<ffi::ExitVtxoResult> {
    Ok(ffi::ExitVtxoResult {
        vtxo_id: exit.id().to_string(),
        amount_sat: exit.amount().to_sat(),
        state: utils::exit_state_name(exit.state()).to_string(),
        state_details: exit_state_details_to_ffi(exit.state()),
        history: exit
            .history()
            .iter()
            .map(utils::exit_state_name)
            .map(str::to_string)
            .collect(),
        history_details: exit
            .history()
            .iter()
            .map(exit_state_details_to_ffi)
            .collect(),
        txids: exit
            .txids()
            .map(|txids| txids.iter().map(ToString::to_string).collect())
            .unwrap_or_default(),
        is_claimable: exit.is_claimable(),
        is_initialized: exit.is_initialized(),
    })
}

fn exit_transaction_package_to_ffi(
    package: bark::exit::ExitTransactionPackage,
) -> ffi::ExitTransactionPackageResult {
    let (child_txid, child_tx_hex, child_origin, has_child) = match package.child {
        Some(child) => (
            child.info.txid.to_string(),
            bitcoin::consensus::encode::serialize_hex(&child.info.tx),
            child.origin.to_string(),
            true,
        ),
        None => (String::new(), String::new(), String::new(), false),
    };

    ffi::ExitTransactionPackageResult {
        exit_txid: package.exit.txid.to_string(),
        exit_tx_hex: bitcoin::consensus::encode::serialize_hex(&package.exit.tx),
        child_txid,
        child_tx_hex,
        child_origin,
        has_child,
    }
}

pub(crate) fn get_exit_status(
    vtxo_id: &str,
    include_history: bool,
    include_transactions: bool,
) -> anyhow::Result<*const ffi::ExitStatusResult> {
    ffi_boundary("get_exit_status", || {
        let status = TOKIO_RUNTIME.block_on(crate::get_exit_status(
            vtxo_id.to_string(),
            include_history,
            include_transactions,
        ))?;

        match status {
            Some(status) => {
                let history = status.history.unwrap_or_default();
                let result = ffi::ExitStatusResult {
                    vtxo_id: status.vtxo_id.to_string(),
                    state: utils::exit_state_name(&status.state).to_string(),
                    state_details: exit_state_details_to_ffi(&status.state),
                    history: history
                        .iter()
                        .map(utils::exit_state_name)
                        .map(str::to_string)
                        .collect(),
                    history_details: history.iter().map(exit_state_details_to_ffi).collect(),
                    transactions: status
                        .transactions
                        .into_iter()
                        .map(exit_transaction_package_to_ffi)
                        .collect(),
                };
                Ok(Box::into_raw(Box::new(result)) as *const ffi::ExitStatusResult)
            }
            None => Ok(std::ptr::null()),
        }
    })
}

pub(crate) fn has_pending_exits() -> anyhow::Result<bool> {
    ffi_boundary("has_pending_exits", || {
        TOKIO_RUNTIME.block_on(crate::has_pending_exits())
    })
}

pub(crate) fn pending_exit_total() -> anyhow::Result<u64> {
    ffi_boundary("pending_exit_total", || {
        Ok(TOKIO_RUNTIME
            .block_on(crate::pending_exit_total())?
            .to_sat())
    })
}

pub(crate) fn all_claimable_at_height() -> anyhow::Result<*const u32> {
    ffi_boundary("all_claimable_at_height", || {
        let blockheight = TOKIO_RUNTIME.block_on(crate::all_claimable_at_height())?;
        match blockheight {
            Some(height) => Ok(Box::into_raw(Box::new(height)) as *const u32),
            None => Ok(std::ptr::null()),
        }
    })
}

pub(crate) fn drain_exits(
    vtxo_ids: Vec<String>,
    destination_address: &str,
    fee_rate_sat_per_kvb: *const u64,
) -> anyhow::Result<String> {
    ffi_boundary("drain_exits", || {
        let fee_rate = unsafe {
            fee_rate_sat_per_kvb
                .as_ref()
                .copied()
                .map(FeeRate::from_sat_per_kvb_ceil)
        };

        let ark_info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;

        let destination_address_opt = Address::<address::NetworkUnchecked>::from_str(
            destination_address,
        )
        .with_context(|| {
            format!(
                "Invalid destination address format: '{}'",
                destination_address
            )
        })?;
        let addr = destination_address_opt
            .require_network(ark_info.network)
            .with_context(|| {
                format!(
                    "Address '{}' is not valid for configured network {:?}",
                    destination_address, ark_info.network
                )
            })?;

        let psbt = TOKIO_RUNTIME.block_on(crate::drain_exits(vtxo_ids, addr, fee_rate))?;
        Ok(psbt.to_string())
    })
}

pub(crate) fn extract_transaction(psbt: &str) -> anyhow::Result<String> {
    ffi_boundary("extract_transaction", || {
        let psbt = bitcoin::Psbt::from_str(psbt)
            .with_context(|| format!("Invalid PSBT format: '{}'", psbt))?;
        let tx = TOKIO_RUNTIME.block_on(crate::onchain::extract_transaction(psbt))?;
        Ok(bitcoin::consensus::encode::serialize_hex(&tx))
    })
}

pub(crate) fn broadcast_transaction(tx_hex: &str) -> anyhow::Result<String> {
    ffi_boundary("broadcast_transaction", || {
        let tx = bitcoin::consensus::encode::deserialize_hex(tx_hex)
            .with_context(|| format!("Invalid transaction hex format: '{}'", tx_hex))?;
        let txid = TOKIO_RUNTIME.block_on(crate::onchain::broadcast_transaction(tx))?;
        Ok(txid.to_string())
    })
}

pub(crate) fn send_onchain(destination: &str, amount_sat: u64) -> anyhow::Result<String> {
    ffi_boundary("send_onchain", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let address_unchecked = bitcoin::Address::from_str(destination)
            .with_context(|| format!("Invalid destination address format: '{}'", destination))?;

        let ark_info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;

        let destination_address = address_unchecked
            .require_network(ark_info.network)
            .with_context(|| {
                format!(
                    "address '{}' is not valid for configured network {}",
                    destination, ark_info.network
                )
            })?;

        let result =
            crate::TOKIO_RUNTIME.block_on(crate::send_onchain(destination_address, amount))?;

        Ok(result.to_string())
    })
}

pub(crate) fn estimate_send_onchain(
    destination: &str,
    amount_sat: u64,
) -> anyhow::Result<BarkFeeEstimate> {
    ffi_boundary("estimate_send_onchain", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);
        let address_unchecked = bitcoin::Address::from_str(destination)
            .with_context(|| format!("Invalid destination address format: '{}'", destination))?;

        let ark_info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;

        let destination_address = address_unchecked
            .require_network(ark_info.network)
            .with_context(|| {
                format!(
                    "address '{}' is not valid for configured network {}",
                    destination, ark_info.network
                )
            })?;

        let estimate = crate::TOKIO_RUNTIME
            .block_on(crate::estimate_send_onchain(destination_address, amount))?;

        Ok(BarkFeeEstimate {
            gross_amount_sat: estimate.gross_amount.to_sat(),
            fee_sat: estimate.fee.to_sat(),
            net_amount_sat: estimate.net_amount.to_sat(),
            vtxos_spent: estimate
                .vtxos_spent
                .into_iter()
                .map(|vtxo_id| vtxo_id.to_string())
                .collect(),
        })
    })
}

pub(crate) fn offboard_specific(
    vtxo_ids: Vec<String>,
    destination_address: &str,
) -> anyhow::Result<String> {
    ffi_boundary("offboard_specific", || {
        let ids = vtxo_ids
            .into_iter()
            .map(|s| bark::ark::VtxoId::from_str(&s))
            .collect::<Result<Vec<_>, _>>()?;

        let ark_info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;

        let destination_address_opt = Address::<address::NetworkUnchecked>::from_str(
            destination_address,
        )
        .with_context(|| {
            format!(
                "Invalid destination address format: '{}'",
                destination_address
            )
        })?;
        let addr = destination_address_opt
            .require_network(ark_info.network)
            .with_context(|| {
                format!(
                    "Address '{}' is not valid for configured network {:?}",
                    destination_address, ark_info.network
                )
            })?;

        if ids.is_empty() {
            bail!("At least one VTXO ID must be provided for specific offboarding");
        }

        info!(
            "Attempting to offboard {} specific VTXOs to {:?}",
            ids.len(),
            addr
        );

        let offboard_specific_result =
            crate::TOKIO_RUNTIME.block_on(crate::offboard_specific(ids, addr))?;

        Ok(offboard_specific_result.to_string())
    })
}

pub(crate) fn offboard_all(destination_address: &str) -> anyhow::Result<String> {
    ffi_boundary("offboard_all", || {
        let ark_info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;

        let destination_address_opt = Address::<address::NetworkUnchecked>::from_str(
            destination_address,
        )
        .with_context(|| {
            format!(
                "Invalid destination address format: '{}'",
                destination_address
            )
        })?;
        let addr = destination_address_opt
            .require_network(ark_info.network)
            .with_context(|| {
                format!(
                    "Address '{}' is not valid for configured network {:?}",
                    destination_address, ark_info.network
                )
            })?;

        info!("Attempting to offboard all VTXOs to {:?}", addr);

        let offboard_all_result = crate::TOKIO_RUNTIME.block_on(crate::offboard_all(addr))?;

        Ok(offboard_all_result.to_string())
    })
}

pub(crate) fn estimate_offboard_all(destination_address: &str) -> anyhow::Result<BarkFeeEstimate> {
    ffi_boundary("estimate_offboard_all", || {
        let ark_info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;

        let destination_address_opt = Address::<address::NetworkUnchecked>::from_str(
            destination_address,
        )
        .with_context(|| {
            format!(
                "Invalid destination address format: '{}'",
                destination_address
            )
        })?;
        let addr = destination_address_opt
            .require_network(ark_info.network)
            .with_context(|| {
                format!(
                    "Address '{}' is not valid for configured network {:?}",
                    destination_address, ark_info.network
                )
            })?;

        let estimate = crate::TOKIO_RUNTIME.block_on(crate::estimate_offboard_all(addr))?;

        Ok(BarkFeeEstimate {
            gross_amount_sat: estimate.gross_amount.to_sat(),
            fee_sat: estimate.fee.to_sat(),
            net_amount_sat: estimate.net_amount.to_sat(),
            vtxos_spent: estimate
                .vtxos_spent
                .into_iter()
                .map(|vtxo_id| vtxo_id.to_string())
                .collect(),
        })
    })
}

pub(crate) fn try_claim_lightning_receive(
    payment_hash: String,
    wait: bool,
    token: *const String,
) -> anyhow::Result<ffi::LightningReceive> {
    ffi_boundary("try_claim_lightning_receive", || {
        let payment_hash = PaymentHash::from_str(&payment_hash)?;
        let token_opt = unsafe { token.as_ref().map(|s| s.clone()) };

        let status = TOKIO_RUNTIME.block_on(crate::try_claim_lightning_receive(
            payment_hash,
            wait,
            token_opt,
        ))?;

        Ok(ffi::LightningReceive {
            payment_hash: status.payment_hash.to_string(),
            payment_preimage: status.payment_preimage.to_string(),
            invoice: status.invoice.to_string(),
            preimage_revealed_at: status.preimage_revealed_at.map_or(std::ptr::null(), |v| {
                Box::into_raw(Box::new(v.timestamp() as u64))
            }),
            finished_at: status.finished_at.map_or(std::ptr::null(), |v| {
                Box::into_raw(Box::new(v.timestamp() as u64))
            }),
        })
    })
}

pub(crate) fn try_claim_all_lightning_receives(wait: bool) -> anyhow::Result<()> {
    ffi_boundary("try_claim_all_lightning_receives", || {
        crate::TOKIO_RUNTIME.block_on(crate::try_claim_all_lightning_receives(wait))?;
        Ok(())
    })
}

pub(crate) fn check_lightning_payment(
    payment_hash: String,
    wait: bool,
) -> anyhow::Result<ffi::LightningPaymentResult> {
    ffi_boundary("check_lightning_payment", || {
        let payment_hash = PaymentHash::from_str(&payment_hash)?;
        let result =
            crate::TOKIO_RUNTIME.block_on(crate::check_lightning_payment(payment_hash, wait))?;
        Ok(lightning_payment_result_to_ffi(result))
    })
}

pub(crate) fn start_exit_for_entire_wallet() -> anyhow::Result<()> {
    ffi_boundary("start_exit_for_entire_wallet", || {
        TOKIO_RUNTIME.block_on(crate::start_exit_for_entire_wallet())
    })
}

pub(crate) fn start_exit_for_vtxos(vtxo_ids: Vec<String>) -> anyhow::Result<()> {
    ffi_boundary("start_exit_for_vtxos", || {
        TOKIO_RUNTIME.block_on(crate::start_exit_for_vtxos(vtxo_ids))
    })
}

pub(crate) fn sync_exit() -> anyhow::Result<()> {
    ffi_boundary("sync_exit", || TOKIO_RUNTIME.block_on(crate::sync_exit()))
}

pub(crate) fn sync_pending_rounds() -> anyhow::Result<Vec<ffi::PendingRoundStatus>> {
    ffi_boundary("sync_pending_rounds", || {
        let statuses = TOKIO_RUNTIME.block_on(crate::sync_pending_rounds())?;
        Ok(statuses
            .into_iter()
            .map(|status| {
                let round_status = utils::round_status_to_fields(status.status);
                ffi::PendingRoundStatus {
                    round_id: status.round_id.0,
                    status: round_status.status,
                    funding_txid: round_status.funding_txid,
                    unsigned_funding_txids: round_status.unsigned_funding_txids,
                    error: round_status.error,
                    is_final: round_status.is_final,
                    is_success: round_status.is_success,
                }
            })
            .collect())
    })
}

pub(crate) fn mailbox_keypair() -> anyhow::Result<ffi::KeyPairResult> {
    ffi_boundary("mailbox_keypair", || {
        let keypair = crate::TOKIO_RUNTIME.block_on(crate::mailbox_keypair())?;
        Ok(ffi::KeyPairResult {
            public_key: keypair.public_key().to_string(),
            secret_key: keypair.secret_key().display_secret().to_string(),
        })
    })
}

pub(crate) fn mailbox_authorization(
    authorization_expiry: i64,
) -> anyhow::Result<ffi::MailboxAuthorizationResult> {
    ffi_boundary("mailbox_authorization", || {
        let expiry = chrono::DateTime::from_timestamp(authorization_expiry, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?
            .with_timezone(&chrono::Local);
        let auth = crate::TOKIO_RUNTIME.block_on(crate::mailbox_authorization(expiry))?;

        // Encode the full authorization using ProtocolEncoding
        use bark::ark::ProtocolEncoding;
        let mut encoded_bytes = Vec::new();
        auth.encode(&mut encoded_bytes)
            .context("Failed to encode mailbox authorization")?;

        Ok(ffi::MailboxAuthorizationResult {
            mailbox_id: auth.mailbox().to_string(),
            expiry: auth.expiry().timestamp(),
            encoded: hex::encode(&encoded_bytes),
        })
    })
}

// Onchain methods

pub(crate) fn onchain_list_unspent() -> anyhow::Result<String> {
    ffi_boundary("onchain_list_unspent", || {
        let unspent = TOKIO_RUNTIME.block_on(crate::onchain::list_unspent())?;
        serde_json::to_string(&unspent).map_err(Into::into)
    })
}

pub(crate) fn onchain_sync() -> anyhow::Result<()> {
    ffi_boundary("onchain_sync", || {
        crate::TOKIO_RUNTIME.block_on(crate::onchain::sync())?;
        Ok(())
    })
}

pub(crate) fn onchain_address() -> anyhow::Result<String> {
    ffi_boundary("onchain_address", || {
        let address = crate::TOKIO_RUNTIME.block_on(crate::onchain::address())?;
        Ok(address.to_string())
    })
}

pub(crate) fn onchain_balance() -> anyhow::Result<ffi::OnChainBalance> {
    ffi_boundary("onchain_balance", || {
        let balance = crate::TOKIO_RUNTIME.block_on(crate::onchain::onchain_balance())?;
        Ok(ffi::OnChainBalance {
            immature: balance.immature.to_sat(),
            trusted_pending: balance.trusted_pending.to_sat(),
            untrusted_pending: balance.untrusted_pending.to_sat(),
            confirmed: balance.confirmed.to_sat(),
        })
    })
}

pub(crate) fn onchain_utxos() -> anyhow::Result<String> {
    ffi_boundary("onchain_utxos", || {
        let utxos = crate::TOKIO_RUNTIME.block_on(async { crate::onchain::utxos().await })?;

        let res = utxos
            .iter()
            .map(|utxo| match utxo {
                bark::onchain::Utxo::Local(local) => serde_json::json!({
                    "outpoint": format!("{}:{}", local.outpoint.txid, local.outpoint.vout),
                    "amount": local.amount.to_sat(),
                    "confirmation_height": local.confirmation_height.map_or(0, |_h| 0),
                }),
                bark::onchain::Utxo::Exit(exit) => serde_json::json!({
                    "vtxo": utils::vtxo_to_bark_vtxo(&exit.vtxo),
                    "height": exit.height
                }),
            })
            .collect::<Vec<_>>();

        serde_json::to_string(&res).map_err(Into::into)
    })
}

pub(crate) fn onchain_fee_rates() -> anyhow::Result<BarkFeeRates> {
    ffi_boundary("onchain_fee_rates", || {
        let fee_rates =
            crate::TOKIO_RUNTIME.block_on(async { crate::onchain::fee_rates().await })?;

        Ok(BarkFeeRates {
            fast: fee_rate_to_sat_per_vbyte(fee_rates.fast),
            regular: fee_rate_to_sat_per_vbyte(fee_rates.regular),
            slow: fee_rate_to_sat_per_vbyte(fee_rates.slow),
        })
    })
}

pub(crate) fn onchain_transactions() -> anyhow::Result<Vec<OnchainTransactionInfo>> {
    ffi_boundary("onchain_transactions", || {
        let transactions =
            crate::TOKIO_RUNTIME.block_on(async { crate::onchain::transaction_infos().await })?;

        Ok(transactions
            .into_iter()
            .map(|tx_info| {
                let (has_confirmation, confirmation_height, confirmation_hash) =
                    match tx_info.confirmation {
                        Some(block) => (true, block.height, block.hash.to_string()),
                        None => (false, 0, String::new()),
                    };

                OnchainTransactionInfo {
                    txid: tx_info.txid.to_string(),
                    tx_hex: bitcoin::consensus::encode::serialize_hex(tx_info.tx.as_ref()),
                    has_onchain_fee: tx_info.onchain_fees.is_some(),
                    onchain_fee_sat: tx_info.onchain_fees.map(|fee| fee.to_sat()).unwrap_or(0),
                    balance_change_sat: tx_info.balance_change.to_sat(),
                    has_confirmation,
                    confirmation_height,
                    confirmation_hash,
                }
            })
            .collect())
    })
}

pub(crate) fn onchain_send(
    destination: &str,
    amount_sat: u64,
    fee_rate: *const u64,
) -> anyhow::Result<OnchainPaymentResult> {
    ffi_boundary("onchain_send", || {
        let amount = bark::ark::bitcoin::Amount::from_sat(amount_sat);

        let ark_info = crate::TOKIO_RUNTIME.block_on(crate::get_ark_info())?;

        let address_unchecked = Address::<address::NetworkUnchecked>::from_str(destination)
            .with_context(|| format!("invalid destination address format: '{}'", destination))?;

        let destination_address = address_unchecked
            .require_network(ark_info.network)
            .with_context(|| {
                format!(
                    "address '{}' is not valid for configured network {}",
                    destination, ark_info.network
                )
            })?;

        let txid = crate::TOKIO_RUNTIME.block_on(async {
            let fee_rate = if fee_rate.is_null() {
                let mut manager = crate::GLOBAL_WALLET_MANAGER.lock().await;
                manager
                    .with_context_async(|ctx| async {
                        Ok(ctx.wallet.chain().fee_rates().await.regular)
                    })
                    .await?
            } else {
                FeeRate::from_sat_per_vb(unsafe { *fee_rate }).context("Invalid fee rate")?
            };

            crate::onchain::send(destination_address.clone(), amount, fee_rate).await
        })?;

        Ok(OnchainPaymentResult {
            txid: txid.to_string(),
            amount_sat,
            destination_address: destination_address.to_string(),
        })
    })
}

pub(crate) fn onchain_drain(destination: &str, fee_rate: *const u64) -> anyhow::Result<String> {
    ffi_boundary("onchain_drain", || {
        let txid = crate::TOKIO_RUNTIME.block_on(async {
            let mut manager = crate::GLOBAL_WALLET_MANAGER.lock().await;
            let (address, fee_rate) = manager
                .with_context_async(|ctx| async {
                    let net = ctx.wallet.properties().await?.network;
                    let address = Address::from_str(destination)?
                        .require_network(net)
                        .context("Address on wrong network")?;
                    let fee_rate = if fee_rate.is_null() {
                        ctx.wallet.chain().fee_rates().await.regular
                    } else {
                        FeeRate::from_sat_per_vb(unsafe { *fee_rate })
                            .context("Invalid fee rate")?
                    };
                    Ok((address, fee_rate))
                })
                .await?;

            crate::onchain::drain(address, fee_rate).await
        })?;
        Ok(txid.to_string())
    })
}

pub(crate) fn onchain_send_many(
    outputs: Vec<ffi::SendManyOutput>,
    fee_rate: *const u64,
) -> anyhow::Result<String> {
    ffi_boundary("onchain_send_many", || {
        let txid = crate::TOKIO_RUNTIME.block_on(async {
            let mut manager = crate::GLOBAL_WALLET_MANAGER.lock().await;
            let (destinations, fee_rate): (Vec<(Address, bark::ark::bitcoin::Amount)>, FeeRate) =
                manager
                    .with_context_async(|ctx| async {
                        let mut destinations = Vec::new();
                        let net = ctx.wallet.properties().await?.network;
                        for output in outputs {
                            let address = Address::from_str(&output.destination)
                                .context("Invalid address format")?
                                .require_network(net)
                                .context("Address on wrong network")?;
                            let amount = bark::ark::bitcoin::Amount::from_sat(output.amount_sat);
                            destinations.push((address, amount));
                        }

                        let fee_rate = if fee_rate.is_null() {
                            ctx.wallet.chain().fee_rates().await.regular
                        } else {
                            FeeRate::from_sat_per_vb(unsafe { *fee_rate })
                                .context("Invalid fee rate")?
                        };
                        Ok((destinations, fee_rate))
                    })
                    .await?;

            crate::onchain::send_many(&destinations, fee_rate).await
        })?;
        Ok(txid.to_string())
    })
}
