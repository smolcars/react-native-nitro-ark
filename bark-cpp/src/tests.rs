#![allow(unused_imports)]
use crate::cxx::{
    self,
    ffi::{self, RefreshModeType},
};
use anyhow::Context;
use bark::ark::bitcoin::{Amount, FeeRate};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use tempfile::tempdir;

// --- Test Setup ---

const INVALID_MNEMONIC_SENTINEL: &str =
    "h3-secret-sentinel invalid mnemonic words must stay private";

fn assert_invalid_mnemonic_is_redacted<T>(result: anyhow::Result<T>) {
    let error = match result {
        Ok(_) => panic!("invalid mnemonic should be rejected"),
        Err(error) => error,
    };
    let error_chain = crate::utils::format_error_chain(&error);

    assert!(
        error_chain.contains("Invalid mnemonic format"),
        "error should explain that the mnemonic format is invalid: {error_chain}"
    );
    assert!(
        !error_chain.contains(INVALID_MNEMONIC_SENTINEL),
        "error chain must not contain the supplied mnemonic: {error_chain}"
    );
}

#[test]
fn bark_version_matches_resolved_build_metadata() {
    assert_eq!(crate::cxx::bark_version(), env!("BARK_WALLET_VERSION"));
}

#[test]
fn lightning_send_options_preserve_defaults_zero_and_wait() {
    for wait in [false, true] {
        for seconds in [None, Some(0), Some(30), Some(u32::MAX)] {
            let options = cxx::lightning_send_options(ffi::LightningSendOptions {
                wait,
                has_retry_for: seconds.is_some(),
                retry_for_seconds: f64::from(seconds.unwrap_or(0)),
            })
            .unwrap();
            assert_eq!(options.wait, wait);
            assert_eq!(
                options.retry_for,
                seconds.map(|s| std::time::Duration::from_secs(s.into()))
            );
        }
    }
}

#[test]
fn lightning_sends_reject_invalid_retry_before_parsing_or_wallet_access() {
    for seconds in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.0,
        0.5,
        f64::from(u32::MAX) + 1.0,
    ] {
        let options = ffi::LightningSendOptions {
            wait: false,
            has_retry_for: true,
            retry_for_seconds: seconds,
        };
        for result in [
            cxx::pay_lightning_invoice("invalid invoice", std::ptr::null(), options),
            cxx::pay_lightning_offer("invalid offer", std::ptr::null(), options),
            cxx::pay_lightning_address("invalid address", 1000, "", options),
            cxx::pay_lightning_invoice_with_origin(
                "invalid invoice",
                "invalid origin",
                "",
                options,
            ),
        ] {
            let error = result
                .err()
                .expect("invalid retry must reject before starting a payment");
            assert!(
                error
                    .to_string()
                    .contains("retryForSeconds must be a finite unsigned 32-bit integer")
            );
        }
    }
}

#[test]
fn unlock_vtxos_rejects_invalid_ids_before_wallet_access() {
    let result = cxx::unlock_vtxos(vec!["not-a-vtxo-id".to_string()]);
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Invalid VTXO ID"),
        "error should identify the invalid VTXO ID"
    );
}

#[test]
fn cancel_exit_rejects_invalid_id_before_wallet_access() {
    let result = cxx::cancel_exit("not-a-vtxo-id");
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Invalid VTXO ID"),
        "error should identify the invalid VTXO ID"
    );
}

#[test]
fn swept_exit_details_preserve_outpoints_and_tip_height() {
    let tip = bitcoin_ext::BlockHeight::new(321);
    let expected = [
        format!("{}:0", "11".repeat(32)),
        format!("{}:{}", "22".repeat(32), u32::MAX),
    ];
    let inputs = expected
        .iter()
        .map(|input| bark::ark::bitcoin::OutPoint::from_str(input).unwrap())
        .collect();
    let state = bark::exit::ExitState::new_vtxo_swept(tip, inputs);
    let result = cxx::exit_state_details_to_ffi(&state);
    assert_eq!(result.kind, "vtxo-swept");
    assert_eq!(result.tip_height, 321);
    assert_eq!(result.spent_inputs, expected);

    for state in [
        bark::exit::ExitState::new_vtxo_swept(tip, Vec::new()),
        bark::exit::ExitState::new_start(tip),
        bark::exit::ExitState::new_vtxo_already_spent(tip),
    ] {
        assert!(
            cxx::exit_state_details_to_ffi(&state)
                .spent_inputs
                .is_empty()
        );
    }
}

#[test]
fn emergency_exit_fee_rejects_invalid_ids_before_wallet_access() {
    let result = cxx::estimate_emergency_exit_fee(
        vec!["not-a-vtxo-id".to_string()],
        std::ptr::null(),
        std::ptr::null(),
        std::ptr::null(),
    );
    let error = match result {
        Ok(_) => panic!("invalid VTXO ID should be rejected"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("Invalid VTXO ID"),
        "error should identify the invalid VTXO ID"
    );
}

#[test]
fn emergency_exit_fee_conversion_preserves_the_breakdown() {
    for (sat_per_kwu, sat_per_vb) in [(500, 2.0), (375, 1.5)] {
        let estimate = bark::exit::ExitFeeEstimate {
            exit_broadcast_fee: Amount::from_sat(1_200),
            claim_fee: Amount::from_sat(300),
            fee_rate: FeeRate::from_sat_per_kwu(sat_per_kwu),
            txs_to_broadcast: 4,
        };

        let result = cxx::exit_fee_estimate_to_ffi(&estimate);

        assert_eq!(result.exit_broadcast_fee_sat, 1_200);
        assert_eq!(result.claim_fee_sat, 300);
        assert_eq!(result.total_fee_sat, 1_500);
        assert_eq!(result.fee_rate_sat_per_vb, sat_per_vb);
        assert_eq!(result.txs_to_broadcast, 4);
    }
}

#[test]
fn emergency_exit_fee_validates_margins_before_wallet_access() {
    for margin in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.5] {
        let error = cxx::estimate_emergency_exit_fee(
            vec!["not-a-vtxo-id".to_string()],
            std::ptr::null(),
            std::ptr::null(),
            &margin,
        )
        .err()
        .expect("invalid margin should be rejected");
        assert!(
            error
                .to_string()
                .contains("feeMargin must be finite and non-negative")
        );
    }

    for margin in [0.0, 1.0, 1.2, 1.5] {
        let error = cxx::estimate_emergency_exit_fee(
            vec!["not-a-vtxo-id".to_string()],
            std::ptr::null(),
            std::ptr::null(),
            &margin,
        )
        .err()
        .expect("invalid VTXO ID should be rejected");
        assert!(error.to_string().contains("Invalid VTXO ID"));
    }
}

#[test]
fn offchain_balance_conversion_preserves_categories_and_totals() {
    for unit in [0, 5_000_000_000] {
        let balance = bark::Balance {
            spendable: Amount::from_sat(1_000 + unit),
            needs_refresh: Amount::from_sat(200 + unit),
            pending_arkoor_send: Amount::from_sat(unit),
            pending_lightning_send: Amount::from_sat(2 * unit),
            claimable_lightning_receive: Amount::from_sat(4 * unit),
            pending_in_round: Amount::from_sat(8 * unit),
            pending_board: Amount::from_sat(16 * unit),
            pending_offboard: Amount::from_sat(32 * unit),
            pending_exit: Amount::from_sat(64 * unit),
        };

        let result = cxx::offchain_balance_to_ffi(&balance);

        assert_eq!(result.spendable, 1_000 + unit);
        assert_eq!(result.needs_refresh, 200 + unit);
        assert_eq!(result.pending_arkoor_send, unit);
        assert_eq!(result.pending_lightning_send, 2 * unit);
        assert_eq!(result.claimable_lightning_receive, 4 * unit);
        assert_eq!(result.pending_in_round, 8 * unit);
        assert_eq!(result.pending_board, 16 * unit);
        assert_eq!(result.pending_offboard, 32 * unit);
        assert_eq!(result.pending_exit, 64 * unit);
        assert_eq!(result.pending, 127 * unit);
        assert_eq!(result.total, 1_200 + 129 * unit);
    }
}

#[test]
fn history_metadata_patch_accepts_json_objects() {
    let patch = crate::parse_history_metadata_patch(
        r#"{"noah":{"lnurl_pay":{"payer_data":{"name":"Alice"}}}}"#,
    )
    .expect("object patches should be accepted");

    assert_eq!(patch["noah"]["lnurl_pay"]["payer_data"]["name"], "Alice");
}

#[test]
fn history_metadata_patch_rejects_non_objects() {
    for patch in ["null", "[]", r#""value""#, "42", "true"] {
        let error = crate::parse_history_metadata_patch(patch)
            .expect_err("non-object patches should be rejected");
        assert!(
            error
                .to_string()
                .contains("History metadata patch must be a JSON object")
        );
    }
}

#[test]
fn history_metadata_patch_rejects_malformed_json() {
    let error =
        crate::parse_history_metadata_patch("{").expect_err("malformed JSON should be rejected");
    assert!(
        error
            .to_string()
            .contains("Invalid history metadata patch JSON")
    );
}

#[test]
fn history_metadata_patch_rejects_oversized_payloads_before_wallet_access() {
    let oversized = format!(
        r#"{{"value":"{}"}}"#,
        "x".repeat(crate::MAX_HISTORY_METADATA_PATCH_BYTES)
    );
    let error = cxx::update_history_metadata(1, &oversized)
        .expect_err("oversized patches should be rejected");
    assert!(
        error
            .to_string()
            .contains("History metadata patch exceeds the 16384 byte limit")
    );
}

#[test]
fn lightning_payment_origin_accepts_supported_methods() {
    let cases = [
        ("lightning-address", "byte@second.tech", "byte@second.tech"),
        (
            "lnurl",
            "LNURL1DP68GURN8GHJ7UM9WFMXJCM99E3K7MF0V9CXJ0M385EKVCENXC6R2C35XVUKXEFCV5MKVV34X5EKZD3EV56NYD3HXQURZEPEXEJXXEPNXSCRVWFNV9NXZCN9XQ6XYEFHVGCXXCMYXYMNSERXFQ5FNS",
            "lnurl1dp68gurn8ghj7um9wfmxjcm99e3k7mf0v9cxj0m385ekvcenxc6r2c35xvukxefcv5mkvv34x5ekzd3ev56nyd3hxqurzepexejxxepnxscrvwfnv9nxzcn9xq6xyefhvgcxxcmyxymnserxfq5fns",
        ),
        (
            "custom",
            "https://example.com/lnurlp/alice",
            "https://example.com/lnurlp/alice",
        ),
    ];

    for (method, value, expected_value) in cases {
        let origin = crate::parse_lightning_payment_origin(method, value)
            .expect("supported Lightning payment origins should be accepted");

        assert_eq!(origin.type_str(), method);
        assert_eq!(origin.value_string(), expected_value);
    }
}

#[test]
fn lightning_payment_origin_rejects_invalid_values() {
    for (method, value) in [
        ("lightning-address", "not-an-address"),
        ("lnurl", "https://example.com/lnurlp/alice"),
        ("custom", ""),
        ("custom", "   "),
    ] {
        crate::parse_lightning_payment_origin(method, value)
            .expect_err("invalid Lightning payment origin values should be rejected");
    }
}

#[test]
fn lightning_payment_origin_rejects_unsupported_methods() {
    for method in [
        "ark",
        "bitcoin",
        "output-script",
        "invoice",
        "offer",
        "unknown",
    ] {
        let error = crate::parse_lightning_payment_origin(method, "value")
            .expect_err("unsupported Lightning payment origin methods should be rejected");
        assert!(
            error
                .to_string()
                .contains("Unsupported Lightning payment origin method")
        );
    }
}

/// Creates a temporary directory and basic wallet creation options for tests.
fn setup_test_wallet_opts() -> (tempfile::TempDir, ffi::CreateOpts) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let mnemonic = cxx::create_mnemonic().expect("Failed to create mnemonic for test");

    let config_opts = ffi::ConfigOpts {
        // Using placeholder values for services not directly hit in most unit tests.
        // For real integration tests, these would point to live regtest services.
        ark: "http://127.0.0.1:50051".to_string(),
        user_agent: "".to_string(),
        esplora: "http://127.0.0.1:3002".to_string(),
        bitcoind: "".to_string(),
        bitcoind_cookie: "".to_string(),
        bitcoind_user: "".to_string(),
        bitcoind_pass: "".to_string(),
        vtxo_refresh_expiry_threshold: 3600,
        vtxo_key_gap_limit: 250,
        fallback_fee_rate: 1,
        htlc_recv_claim_delta: 18,
        vtxo_exit_margin: 12,
        round_tx_required_confirmations: 0,
    };

    let create_opts = ffi::CreateOpts {
        regtest: true,
        signet: false,
        bitcoin: false,
        mnemonic,
        birthday_height: std::ptr::null(),
        config: config_opts,
    };

    (temp_dir, create_opts)
}

#[test]
fn merge_config_opts_preserves_vtxo_key_gap_limits() {
    for gap_limit in [
        0,
        bark::DEFAULT_VTXO_KEY_GAP_LIMIT,
        10_000,
        bark::MAX_VTXO_KEY_GAP_LIMIT,
    ] {
        let (_temp_dir, mut opts) = setup_test_wallet_opts();
        opts.config.vtxo_key_gap_limit = gap_limit;
        let create_opts = crate::utils::ffi_config_to_config(opts).unwrap();
        let (config, _) = crate::utils::merge_config_opts(create_opts).unwrap();
        assert_eq!(config.vtxo_key_gap_limit, gap_limit);
    }
}

#[test]
fn merge_config_opts_rejects_excessive_vtxo_key_gap_limits() {
    for gap_limit in [bark::MAX_VTXO_KEY_GAP_LIMIT + 1, u32::MAX] {
        let (_temp_dir, mut opts) = setup_test_wallet_opts();
        opts.config.vtxo_key_gap_limit = gap_limit;
        let create_opts = crate::utils::ffi_config_to_config(opts).unwrap();
        let error = crate::utils::merge_config_opts(create_opts).unwrap_err();
        assert!(
            crate::utils::format_error_chain(&error)
                .contains("vtxo_key_gap_limit must be at most 100000")
        );
    }
}

#[test]
fn merge_config_opts_preserves_valid_refresh_thresholds() {
    for threshold in [0, 144, u16::MAX as u32] {
        let (_temp_dir, mut opts) = setup_test_wallet_opts();
        opts.config.vtxo_refresh_expiry_threshold = threshold;
        let create_opts = crate::utils::ffi_config_to_config(opts).unwrap();
        let (config, _) = crate::utils::merge_config_opts(create_opts).unwrap();

        assert_eq!(u32::from(config.vtxo_refresh_expiry_threshold), threshold);
    }
}

#[test]
fn merge_config_opts_rejects_overflowing_refresh_thresholds() {
    for threshold in [u16::MAX as u32 + 1, u32::MAX] {
        let (_temp_dir, mut opts) = setup_test_wallet_opts();
        opts.config.vtxo_refresh_expiry_threshold = threshold;
        let create_opts = crate::utils::ffi_config_to_config(opts).unwrap();
        let error = crate::utils::merge_config_opts(create_opts).unwrap_err();

        assert!(
            crate::utils::format_error_chain(&error)
                .contains("vtxo_refresh_expiry_threshold must be at most 65535 blocks")
        );
    }
}

#[test]
fn block_delta_inputs_are_checked_at_the_ffi_boundary() {
    for confirmations in [0, u16::MAX as u32] {
        let (_temp_dir, mut opts) = setup_test_wallet_opts();
        opts.config.round_tx_required_confirmations = confirmations;
        let opts = crate::utils::ffi_config_to_config(opts).unwrap();
        let (config, _) = crate::utils::merge_config_opts(opts).unwrap();
        assert_eq!(
            u32::from(config.round_tx_required_confirmations),
            confirmations
        );
    }

    for value in [u16::MAX as u32 + 1, u32::MAX] {
        let (_temp_dir, mut opts) = setup_test_wallet_opts();
        opts.config.round_tx_required_confirmations = value;
        let opts = crate::utils::ffi_config_to_config(opts).unwrap();
        let error = crate::utils::merge_config_opts(opts).unwrap_err();
        assert!(
            crate::utils::format_error_chain(&error)
                .contains("round_tx_required_confirmations must be at most 65535 blocks")
        );

        let error = match cxx::get_expiring_vtxos(value) {
            Ok(_) => panic!("overflowing block delta should be rejected"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("threshold must be at most 65535 blocks")
        );
    }
}

#[test]
fn ffi_config_to_config_maps_empty_user_agent_to_none() {
    let (_temp_dir, opts) = setup_test_wallet_opts();
    let create_opts = crate::utils::ffi_config_to_config(opts).unwrap();

    assert_eq!(create_opts.config.user_agent, None);
}

#[test]
fn ffi_config_to_config_maps_non_empty_user_agent_to_some() {
    let (_temp_dir, mut opts) = setup_test_wallet_opts();
    opts.config.user_agent = "someapp-android/0.0.1".to_string();
    let create_opts = crate::utils::ffi_config_to_config(opts).unwrap();

    assert_eq!(
        create_opts.config.user_agent,
        Some("someapp-android/0.0.1".to_string())
    );
}

#[test]
fn merge_config_opts_sets_bark_user_agent() {
    let (_temp_dir, mut opts) = setup_test_wallet_opts();
    opts.config.user_agent = "someapp-ios/0.0.1".to_string();
    let create_opts = crate::utils::ffi_config_to_config(opts).unwrap();
    let (config, _network) = crate::utils::merge_config_opts(create_opts).unwrap();

    assert_eq!(config.user_agent.as_deref(), Some("someapp-ios/0.0.1"));
}

#[test]
fn format_error_chain_includes_causes() {
    let error = anyhow::anyhow!("root cause")
        .context("middle context")
        .context("outer context");

    assert_eq!(
        crate::utils::format_error_chain(&error),
        "outer context\ncaused by: middle context\ncaused by: root cause"
    );
}

#[test]
fn invalid_mnemonic_errors_do_not_include_the_supplied_value() {
    assert_invalid_mnemonic_is_redacted(cxx::sign_messsage_with_mnemonic(
        "message",
        INVALID_MNEMONIC_SENTINEL,
        "mainnet",
        0,
    ));
    assert_invalid_mnemonic_is_redacted(cxx::derive_keypair_from_mnemonic(
        INVALID_MNEMONIC_SENTINEL,
        "mainnet",
        0,
    ));

    let (create_dir, mut create_opts) = setup_test_wallet_opts();
    create_opts.mnemonic = INVALID_MNEMONIC_SENTINEL.to_string();
    assert_invalid_mnemonic_is_redacted(cxx::create_wallet(
        create_dir.path().to_str().unwrap(),
        create_opts,
    ));

    let (load_dir, mut load_opts) = setup_test_wallet_opts();
    load_opts.mnemonic = INVALID_MNEMONIC_SENTINEL.to_string();
    assert_invalid_mnemonic_is_redacted(cxx::load_wallet(
        load_dir.path().to_str().unwrap(),
        load_opts,
    ));

    let (restore_dir, mut restore_opts) = setup_test_wallet_opts();
    restore_opts.mnemonic = INVALID_MNEMONIC_SENTINEL.to_string();
    assert_invalid_mnemonic_is_redacted(cxx::restore_wallet_from_ark_server(
        restore_dir.path().to_str().unwrap(),
        restore_opts,
    ));
    assert_eq!(fs::read_dir(restore_dir.path()).unwrap().count(), 0);
}

#[test]
fn recovery_report_preserves_groups_unknown_amounts_and_incompleteness() {
    use bark::ark::bitcoin::{OutPoint, secp256k1::PublicKey};
    use bark::ark::{Vtxo, VtxoId, VtxoPolicy};
    let key =
        PublicKey::from_str("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
    let vtxo = |index| {
        let point = OutPoint::from_str(&format!("{}:{index}", "11".repeat(32))).unwrap();
        Vtxo::new(
            point,
            VtxoPolicy::new_pubkey(key),
            Amount::from_sat(1000),
            bitcoin_ext::BlockHeight::new(100),
            key,
            bitcoin_ext::BlockDelta::from(12u16),
            point,
        )
    };
    let unknown = VtxoId::from(vtxo(5).point());
    let mut report = bark::RecoveryReport::default();
    assert!(cxx::recovery_report_to_ffi(&report).is_complete);
    report.push_recovered(&vtxo(0));
    report.push_skipped(&vtxo(1));
    report.push_exited(&vtxo(2));
    report.push_failed(vtxo(4).id(), Some(Amount::from_sat(2000)));
    report.push_failed(unknown, None);
    let mapped = cxx::recovery_report_to_ffi(&report);
    assert!(!mapped.is_complete);
    assert_eq!(
        mapped.failed.vtxo_ids,
        vec![vtxo(4).id().to_string(), unknown.to_string()]
    );
    assert_eq!(mapped.failed.known_amount_sat, 2000);
    for (group, index) in [
        (mapped.recovered, 0),
        (mapped.skipped, 1),
        (mapped.exited, 2),
    ] {
        assert_eq!(group.vtxo_ids, vec![vtxo(index).id().to_string()]);
        assert_eq!(group.known_amount_sat, 1000);
    }
    let mut foreign = bark::RecoveryReport::default();
    foreign.push_foreign(&vtxo(3));
    let mapped = cxx::recovery_report_to_ffi(&foreign);
    assert!(!mapped.is_complete);
    assert_eq!(mapped.foreign.vtxo_ids, vec![vtxo(3).id().to_string()]);
    assert_eq!(mapped.foreign.known_amount_sat, 1000);
}

#[test]
fn restore_initialization_failure_preserves_partial_data() {
    let (dir, mut opts) = setup_test_wallet_opts();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    opts.config.ark = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = cxx::restore_wallet_from_ark_server(dir.path().to_str().unwrap(), opts)
        .err()
        .expect("unavailable server must reject initialization");
    assert!(
        error
            .to_string()
            .contains("Failed to open wallet for Ark server recovery")
    );
    let db = dir.path().join(crate::utils::DB_FILE);
    let before = fs::read(&db).unwrap();
    assert!(!before.is_empty());
    let (_, opts) = setup_test_wallet_opts();
    assert!(cxx::restore_wallet_from_ark_server(dir.path().to_str().unwrap(), opts).is_err());
    assert_eq!(fs::read(db).unwrap(), before);
}

#[test]
#[ignore = "requires an isolated funded captaind and Bitcoin Core regtest container; set NITRO_ARK_RESTORE_CAPTAIND and NITRO_ARK_RESTORE_BITCOIND"]
fn restore_recovers_delegated_round_completed_without_local_data() {
    cxx::init_logger();
    fn bitcoin(container: &str, args: &[&str]) {
        let output = std::process::Command::new("docker")
            .args([
                "exec",
                container,
                "bitcoin-cli",
                "-regtest",
                "-rpcuser=second",
                "-rpcpassword=ark",
                "-rpcwallet=restore-test",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Bitcoin RPC failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let captaind =
        std::env::var("NITRO_ARK_RESTORE_CAPTAIND").expect("isolated captaind binary required");
    let container =
        std::env::var("NITRO_ARK_RESTORE_BITCOIND").expect("isolated regtest container required");
    let (dir, mut opts) = setup_test_wallet_opts();
    opts.config.ark = "http://127.0.0.1:3535".into();
    opts.mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".into();
    opts.config.esplora.clear();
    opts.config.bitcoind = "http://127.0.0.1:18443".into();
    opts.config.bitcoind_user = "second".into();
    opts.config.bitcoind_pass = "ark".into();
    opts.config.round_tx_required_confirmations = 1;
    opts.config.fallback_fee_rate = 10_000;
    let source = dir.path().join("source");
    let restored = dir.path().join("restored");
    cxx::create_wallet(source.to_str().unwrap(), opts.clone()).unwrap();
    cxx::load_wallet(source.to_str().unwrap(), opts.clone()).unwrap();
    // Reject before writing a destination while another wallet is loaded.
    assert!(cxx::restore_wallet_from_ark_server(restored.to_str().unwrap(), opts.clone()).is_err());
    assert!(!restored.exists());
    let address = cxx::onchain_address().unwrap();
    bitcoin(&container, &["sendtoaddress", &address, "0.01"]);
    bitcoin(&container, &["-generate", "6"]);
    cxx::onchain_sync().unwrap();
    cxx::board_amount(100_000).unwrap();
    bitcoin(&container, &["-generate", "6"]);
    let mut board_id = None;
    for _ in 0..50 {
        cxx::sync_pending_boards().unwrap();
        board_id = cxx::vtxos()
            .unwrap()
            .into_iter()
            .find(|v| v.state == "Spendable")
            .map(|v| v.id);
        if board_id.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let board_id = board_id.expect("confirmed board");
    cxx::refresh_vtxos_delegated(vec![board_id.clone()]).unwrap();
    cxx::close_wallet().unwrap();
    fs::remove_dir_all(&source).unwrap();
    let output = std::process::Command::new(captaind)
        .args(["rpc", "trigger-round"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "trigger round: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::thread::sleep(std::time::Duration::from_secs(6));
    bitcoin(&container, &["-generate", "6"]);
    let result =
        cxx::restore_wallet_from_ark_server(restored.to_str().unwrap(), opts.clone()).unwrap();
    assert_eq!(result.status, "completed", "{}", result.error);
    assert!(result.has_report && result.report.is_complete);
    assert!(cxx::is_wallet_loaded());
    let recovered = cxx::vtxos()
        .unwrap()
        .into_iter()
        .filter(|v| v.state == "Spendable")
        .collect::<Vec<_>>();
    assert_eq!(recovered.len(), 1);
    assert_ne!(recovered[0].id, board_id);
    assert!(recovered[0].amount > 0);
    cxx::close_wallet().unwrap();
    assert!(cxx::restore_wallet_from_ark_server(restored.to_str().unwrap(), opts).is_err());
}

/// A test fixture to ensure the wallet is loaded for a test and closed afterward.
struct WalletTestFixture {
    _temp_dir: tempfile::TempDir,
}

impl WalletTestFixture {
    fn new() -> Self {
        cxx::init_logger();
        let (temp_dir, opts) = setup_test_wallet_opts();
        let datadir_str = temp_dir.path().to_str().unwrap();

        if cxx::is_wallet_loaded() {
            cxx::close_wallet().unwrap();
        }

        cxx::create_wallet(datadir_str, opts)
            .with_context(|| "Failed to load wallet in test setup".to_string())
            .unwrap();

        WalletTestFixture {
            _temp_dir: temp_dir,
        }
    }
}

impl Drop for WalletTestFixture {
    fn drop(&mut self) {
        if cxx::is_wallet_loaded() {
            cxx::close_wallet().expect("Failed to close wallet in test teardown");
        }
    }
}

// --- Tests ---

#[test]
fn test_init_logger_ffi() {
    // This just ensures the function can be called without panicking.
    // The logger is initialized globally, so this will be a no-op on subsequent calls.
    cxx::init_logger();
}

#[test]
fn test_create_mnemonic_ffi() {
    cxx::init_logger();
    let result = cxx::create_mnemonic();
    assert!(result.is_ok());
    let mnemonic_str = result.unwrap();
    assert_eq!(mnemonic_str.split_whitespace().count(), 12);
}

#[test]
#[ignore = "requires live regtest backend"]
fn test_wallet_management_ffi() {
    cxx::init_logger();
    let (temp_dir, opts) = setup_test_wallet_opts();
    let datadir_str = temp_dir.path().to_str().unwrap();

    // 1. Wallet should not be loaded initially
    assert!(!cxx::is_wallet_loaded());

    // 2. Load wallet
    let load_result = cxx::create_wallet(datadir_str, opts);
    assert!(
        load_result.is_ok(),
        "Failed to load wallet: {:?}",
        load_result.err()
    );
    assert!(cxx::is_wallet_loaded());

    // 3. Try loading again (should fail)
    let (_temp_dir2, opts2) = setup_test_wallet_opts();
    let datadir_str2 = _temp_dir2.path().to_str().unwrap();
    let load_again_result = cxx::create_wallet(datadir_str2, opts2);
    assert!(
        load_again_result.is_err(),
        "Should not be able to load a second wallet"
    );

    // 4. Close wallet
    let close_result = cxx::close_wallet();
    assert!(close_result.is_ok());
    assert!(!cxx::is_wallet_loaded());

    // 5. Try closing again (should fail)
    let close_again_result = cxx::close_wallet();
    assert!(
        close_again_result.is_err(),
        "Should not be able to close a non-loaded wallet"
    );
}

#[test]
#[ignore = "requires live regtest backend"]
fn test_get_onchain_address_ffi() {
    let _fixture = WalletTestFixture::new();
    let address_result = cxx::onchain_address();
    assert!(address_result.is_ok());
    let address = address_result.unwrap();
    assert!(
        address.starts_with("bcrt1"),
        "Address should be a regtest address"
    );
}

#[test]
#[ignore = "requires live regtest backend"]
fn test_onchain_is_mine_ffi() {
    let _fixture = WalletTestFixture::new();

    // Get the wallet's own address
    let own_address = cxx::onchain_address().unwrap();

    // The wallet's own address should be recognized as "mine"
    let is_own = cxx::onchain_is_mine(&own_address).unwrap();
    assert!(is_own, "Wallet's own address should be recognized as mine");

    // A foreign regtest address should NOT be recognized as "mine"
    let foreign_address = "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    let is_foreign = cxx::onchain_is_mine(foreign_address).unwrap();
    assert!(
        !is_foreign,
        "Foreign address should not be recognized as mine"
    );
}

#[test]
#[ignore = "requires live regtest backend"]
fn test_get_onchain_balance_ffi() {
    let _fixture = WalletTestFixture::new();
    // Use no_sync = true to avoid network calls in a unit test context.
    let balance_result = cxx::onchain_balance();
    assert!(balance_result.is_ok());
    let balance = balance_result.unwrap().confirmed;
    assert_eq!(balance, 0);
}

#[test]
#[ignore = "requires live regtest backend"]
fn test_get_vtxo_pubkey_ffi() {
    let _fixture = WalletTestFixture::new();
    // Request the next available pubkey
    let _fixture = WalletTestFixture::new();
    // On a fresh wallet, these should return empty JSON arrays.
    let onchain_utxos_res = cxx::onchain_utxos();
    assert!(onchain_utxos_res.is_ok());
    assert_eq!(onchain_utxos_res.unwrap(), "[]");

    let vtxos_res = cxx::derive_store_next_keypair();
    assert!(vtxos_res.is_ok());
}

#[test]
#[ignore = "requires live regtest backend"]
fn test_bolt11_invoice_ffi() {
    let _fixture = WalletTestFixture::new();
    // This test requires a running LDK node, which is part of the wallet.
    // It should succeed even without onchain funds.
    let amount_sat = 100_000;
    let invoice_res = cxx::bolt11_invoice(amount_sat, std::ptr::null(), std::ptr::null());
    assert!(
        invoice_res.is_ok(),
        "Failed to create bolt11 invoice: {:?}",
        invoice_res.err()
    );
    let invoice_str = invoice_res.unwrap().bolt11_invoice;
    assert!(
        invoice_str.starts_with("lnbcrt1"),
        "Invoice should be for regtest"
    );
}

#[test]
#[ignore = "requires live regtest backend"]
fn test_onchain_and_boarding_flow_ffi() {
    let _fixture = WalletTestFixture::new();
    // This is an integration test and requires a funded regtest node.
    // 1. Get address
    let _address = cxx::onchain_address().unwrap();

    // (Manual step: fund this address from bitcoind-cli)
    // e.g., `bitcoin-cli -regtest sendtoaddress <address> 1`
    // (Manual step: mine a block)
    // e.g., `bitcoin-cli -regtest -generate 1`

    // 2. Check balance (with sync)
    let balance = cxx::onchain_balance().unwrap().confirmed;
    assert!(
        balance > 0,
        "Wallet should have onchain funds after funding and syncing"
    );

    // 3. Board amount
    let board_amount_sat = 50_000;
    let board_res = cxx::board_amount(board_amount_sat);
    assert!(board_res.is_ok(), "Boarding failed: {:?}", board_res.err());

    // (Manual step: mine the board transaction)

    // 4. Check balance again
    let final_balance = cxx::onchain_balance().unwrap().confirmed;
    assert!(
        final_balance >= board_amount_sat,
        "On chain balance should increase after boarding"
    );
}

#[test]
#[ignore = "requires live regtest backend and a funded wallet"]
fn test_send_onchain_ffi() {
    let _fixture = WalletTestFixture::new();
    let address = cxx::onchain_address().unwrap();

    // This test requires the address to be funded manually.
    let send_res = cxx::onchain_send(&address, 5000, std::ptr::null());
    assert!(
        send_res.is_ok(),
        "send_onchain failed: {:?}",
        send_res.err()
    );
    let txid = send_res.unwrap();
    assert_eq!(txid.txid.len(), 64);
}

#[test]
#[ignore = "requires live regtest backend and a funded wallet"]
fn test_drain_onchain_ffi() {
    let _fixture = WalletTestFixture::new();
    let address = cxx::onchain_address().unwrap();

    // This test requires the address to be funded manually.
    let drain_res = cxx::onchain_drain(&address, std::ptr::null());
    assert!(
        drain_res.is_ok(),
        "drain_onchain failed: {:?}",
        drain_res.err()
    );
    let txid = drain_res.unwrap();
    assert_eq!(txid.len(), 64);
}

#[test]
#[ignore = "requires live regtest backend and a funded wallet"]
fn test_send_many_onchain_ffi() {
    let _fixture = WalletTestFixture::new();
    let address1 = cxx::onchain_address().unwrap();
    let address2 = cxx::onchain_address().unwrap();

    let outputs = vec![
        ffi::SendManyOutput {
            destination: address1,
            amount_sat: 5000,
        },
        ffi::SendManyOutput {
            destination: address2,
            amount_sat: 6000,
        },
    ];

    let send_many_res = cxx::onchain_send_many(outputs, std::ptr::null());
    assert!(
        send_many_res.is_ok(),
        "send_many failed: {:?}",
        send_many_res.err()
    );
    let txid = send_many_res.unwrap();
    assert_eq!(txid.len(), 64);
}

#[test]
#[ignore = "requires live regtest backend and a funded wallet"]
fn test_board_all_ffi() {
    let _fixture = WalletTestFixture::new();
    // Requires wallet to be funded.
    let board_all_res = cxx::board_all();
    assert!(
        board_all_res.is_ok(),
        "board_all failed: {:?}",
        board_all_res.err()
    );
}

#[test]
#[ignore = "requires live regtest backend and a funded wallet with vtxos"]
fn test_send_arkoot_payment_ffi() {
    let _fixture = WalletTestFixture::new();
    // This is a complex test as it can handle different destination types.
    // Here we test sending to a VTXO pubkey (OOR).
    let keypair = cxx::derive_store_next_keypair().unwrap();
    let send_res = cxx::send_arkoor_payment(&keypair.public_key, 5000);
    assert!(
        send_res.is_ok(),
        "send_payment (OOR) failed: {:?}",
        send_res.err()
    );
}

#[test]
#[ignore = "requires live regtest backend and a funded wallet with vtxos"]
fn test_send_bolt11_payment_ffi() {
    let _fixture = WalletTestFixture::new();
    // This is a complex test as it can handle different destination types.
    // Here we test sending to a bolt11 invoice.
    let invoice = cxx::bolt11_invoice(10000, std::ptr::null(), std::ptr::null()).unwrap();
    let amount: u64 = 5000;
    let send_res = cxx::pay_lightning_invoice(
        &invoice.bolt11_invoice,
        &amount as *const u64,
        ffi::LightningSendOptions {
            wait: false,
            has_retry_for: false,
            retry_for_seconds: 0.0,
        },
    );
    assert!(
        send_res.is_ok(),
        "send_payment (bolt11) failed: {:?}",
        send_res.err()
    );
}

#[test]
#[ignore = "requires live regtest backend and a funded wallet with vtxos"]
fn test_offboard_ffi() {
    let _fixture = WalletTestFixture::new();
    // This test would require creating VTXOs first.
    // We test that the call with no VTXOs doesn't panic.
    let offboard_all_res = cxx::offboard_all("");
    assert!(offboard_all_res.is_ok());

    let offboard_specific_res = cxx::offboard_specific(vec![], "");
    assert!(offboard_specific_res.is_ok());
}

#[test]
#[ignore = "requires live regtest backend with a funded lightning node"]
fn test_claim_bolt11_payment_ffi() {
    let _fixture = WalletTestFixture::new();
    // This requires another LN node to pay an invoice generated by our wallet.
    let invoice = cxx::bolt11_invoice(10000, std::ptr::null(), std::ptr::null()).unwrap();
    // In a real test, you would now pay this invoice from another node.
    // For this unit test, we just check that trying to claim an unpaid invoice fails gracefully.
    let claim_res = cxx::try_claim_lightning_receive(invoice.payment_hash, false);
    // Depending on the LDK setup, this might error differently.
    // The key is that it shouldn't panic.
    assert!(claim_res.is_err(), "Claiming an unpaid invoice should fail");
}
