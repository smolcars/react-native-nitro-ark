#pragma once

#include "BarkNotificationSubscription.hpp"
#include "HybridNitroArkSpec.hpp"
#include "generated/ark_cxx.h"
#include "generated/cxx.h"
#include <memory>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <sys/wait.h>
#include <vector>

namespace margelo::nitro::nitroark {

using namespace margelo::nitro;
// Helper function to convert rust vtxos vector to C++ vector
inline std::vector<BarkVtxo> convertRustVtxosToVector(const rust::Vec<bark_cxx::BarkVtxo>& rust_vtxos) {
  std::vector<BarkVtxo> vtxos;
  vtxos.reserve(rust_vtxos.size());

  for (const auto& vtxo_rs : rust_vtxos) {
    BarkVtxo vtxo;
    vtxo.amount = static_cast<double>(vtxo_rs.amount);
    vtxo.expiry_height = static_cast<double>(vtxo_rs.expiry_height);
    vtxo.server_pubkey = std::string(vtxo_rs.server_pubkey.data(), vtxo_rs.server_pubkey.length());
    vtxo.exit_delta = static_cast<double>(vtxo_rs.exit_delta);
    vtxo.anchor_point = std::string(vtxo_rs.anchor_point.data(), vtxo_rs.anchor_point.length());
    vtxo.point = std::string(vtxo_rs.point.data(), vtxo_rs.point.length());
    vtxo.state = std::string(vtxo_rs.state.data(), vtxo_rs.state.length());
    vtxos.push_back(std::move(vtxo));
  }

  return vtxos;
}

inline PendingRoundStatus convertRustPendingRoundStatus(const bark_cxx::PendingRoundStatus& status_rs) {
  PendingRoundStatus status;
  status.round_id = static_cast<double>(status_rs.round_id);
  status.status = std::string(status_rs.status.data(), status_rs.status.length());
  status.funding_txid = std::string(status_rs.funding_txid.data(), status_rs.funding_txid.length());
  status.unsigned_funding_txids.reserve(status_rs.unsigned_funding_txids.size());
  for (const auto& txid : status_rs.unsigned_funding_txids) {
    status.unsigned_funding_txids.emplace_back(std::string(txid.data(), txid.length()));
  }
  status.error = std::string(status_rs.error.data(), status_rs.error.length());
  status.is_final = status_rs.is_final;
  status.is_success = status_rs.is_success;
  return status;
}

inline LightningPaymentResult convertRustLightningPaymentResult(const bark_cxx::LightningPaymentResult& rust_result) {
  LightningPaymentResult result;
  result.state = std::string(rust_result.state.data(), rust_result.state.length());
  if (rust_result.invoice.length() == 0) {
    result.invoice = std::nullopt;
  } else {
    result.invoice = std::string(rust_result.invoice.data(), rust_result.invoice.length());
  }
  result.payment_hash = std::string(rust_result.payment_hash.data(), rust_result.payment_hash.length());
  result.amount = rust_result.has_amount ? std::make_optional(static_cast<double>(rust_result.amount)) : std::nullopt;
  result.htlc_vtxos = convertRustVtxosToVector(rust_result.htlc_vtxos);
  result.movement_id =
      rust_result.has_movement_id ? std::make_optional(static_cast<double>(rust_result.movement_id)) : std::nullopt;
  if (rust_result.preimage.length() == 0) {
    result.preimage = std::nullopt;
  } else {
    result.preimage = std::string(rust_result.preimage.data(), rust_result.preimage.length());
  }
  return result;
}

inline ExitBlockRefResult convertRustExitBlockRef(const bark_cxx::ExitBlockRefResult& block_rs) {
  ExitBlockRefResult block;
  block.height = static_cast<double>(block_rs.height);
  block.hash = std::string(block_rs.hash.data(), block_rs.hash.length());
  return block;
}

inline ExitTxOriginResult convertRustExitTxOrigin(const bark_cxx::ExitTxOriginResult& origin_rs) {
  ExitTxOriginResult origin;
  origin.kind = std::string(origin_rs.kind.data(), origin_rs.kind.length());
  if (origin_rs.has_confirmed_in) {
    origin.confirmed_in = convertRustExitBlockRef(origin_rs.confirmed_in);
  }
  return origin;
}

inline ExitTxStatusResult convertRustExitTxStatus(const bark_cxx::ExitTxStatusResult& status_rs) {
  ExitTxStatusResult status;
  status.kind = std::string(status_rs.kind.data(), status_rs.kind.length());

  if (!status_rs.txids.empty()) {
    status.txids = std::vector<std::string>();
    status.txids->reserve(status_rs.txids.size());
    for (const auto& txid : status_rs.txids) {
      status.txids->emplace_back(std::string(txid.data(), txid.length()));
    }
  }
  if (status_rs.child_txid.length() != 0) {
    status.child_txid = std::string(status_rs.child_txid.data(), status_rs.child_txid.length());
  }
  if (status_rs.has_origin) {
    status.origin = convertRustExitTxOrigin(status_rs.origin);
  }
  if (status_rs.has_block) {
    status.block = convertRustExitBlockRef(status_rs.block);
  }

  return status;
}

inline ExitTxResult convertRustExitTx(const bark_cxx::ExitTxResult& tx_rs) {
  ExitTxResult tx;
  tx.txid = std::string(tx_rs.txid.data(), tx_rs.txid.length());
  tx.status = convertRustExitTxStatus(tx_rs.status);
  return tx;
}

inline ExitStateDetailsResult convertRustExitStateDetails(const bark_cxx::ExitStateDetailsResult& state_rs) {
  ExitStateDetailsResult state;
  state.kind = std::string(state_rs.kind.data(), state_rs.kind.length());
  state.tip_height = static_cast<double>(state_rs.tip_height);

  if (!state_rs.transactions.empty()) {
    state.transactions = std::vector<ExitTxResult>();
    state.transactions->reserve(state_rs.transactions.size());
    for (const auto& tx_rs : state_rs.transactions) {
      state.transactions->push_back(convertRustExitTx(tx_rs));
    }
  }
  if (state_rs.has_confirmed_block) {
    state.confirmed_block = convertRustExitBlockRef(state_rs.confirmed_block);
  }
  if (state_rs.claimable_height != 0) {
    state.claimable_height = static_cast<double>(state_rs.claimable_height);
  }
  if (state_rs.has_claimable_since) {
    state.claimable_since = convertRustExitBlockRef(state_rs.claimable_since);
  }
  if (state_rs.has_last_scanned_block) {
    state.last_scanned_block = convertRustExitBlockRef(state_rs.last_scanned_block);
  }
  if (state_rs.claim_txid.length() != 0) {
    state.claim_txid = std::string(state_rs.claim_txid.data(), state_rs.claim_txid.length());
  }
  if (state_rs.txid.length() != 0) {
    state.txid = std::string(state_rs.txid.data(), state_rs.txid.length());
  }
  if (state_rs.has_block) {
    state.block = convertRustExitBlockRef(state_rs.block);
  }

  return state;
}

inline std::vector<ExitVtxoResult>
convertRustExitVtxosToVector(const rust::Vec<bark_cxx::ExitVtxoResult>& rust_results) {
  std::vector<ExitVtxoResult> results;
  results.reserve(rust_results.size());

  for (const auto& rust_result : rust_results) {
    ExitVtxoResult result;
    result.vtxo_id = std::string(rust_result.vtxo_id.data(), rust_result.vtxo_id.length());
    result.amount_sat = static_cast<double>(rust_result.amount_sat);
    result.state = std::string(rust_result.state.data(), rust_result.state.length());
    result.state_details = convertRustExitStateDetails(rust_result.state_details);

    result.history.reserve(rust_result.history.size());
    for (const auto& state : rust_result.history) {
      result.history.emplace_back(std::string(state.data(), state.length()));
    }

    result.history_details.reserve(rust_result.history_details.size());
    for (const auto& stateDetails : rust_result.history_details) {
      result.history_details.push_back(convertRustExitStateDetails(stateDetails));
    }

    result.txids.reserve(rust_result.txids.size());
    for (const auto& txid : rust_result.txids) {
      result.txids.emplace_back(std::string(txid.data(), txid.length()));
    }

    result.is_claimable = rust_result.is_claimable;
    result.is_initialized = rust_result.is_initialized;
    results.push_back(std::move(result));
  }

  return results;
}

inline BarkMovementDestination convertRustMovementDestination(const bark_cxx::BarkMovementDestination& destination_rs) {
  BarkMovementDestination destination;
  destination.destination = std::string(destination_rs.destination.data(), destination_rs.destination.length());
  destination.payment_method =
      std::string(destination_rs.payment_method.data(), destination_rs.payment_method.length());
  destination.amount_sat = static_cast<double>(destination_rs.amount_sat);
  return destination;
}

inline BarkMovement convertRustMovement(const bark_cxx::BarkMovement& movement_rs) {
  BarkMovement movement;
  movement.id = static_cast<double>(movement_rs.id);
  movement.status = std::string(movement_rs.status.data(), movement_rs.status.length());
  movement.metadata_json = std::string(movement_rs.metadata_json.data(), movement_rs.metadata_json.length());
  movement.intended_balance_sat = static_cast<double>(movement_rs.intended_balance_sat);
  movement.effective_balance_sat = static_cast<double>(movement_rs.effective_balance_sat);
  movement.offchain_fee_sat = static_cast<double>(movement_rs.offchain_fee_sat);
  movement.created_at = std::string(movement_rs.created_at.data(), movement_rs.created_at.length());
  movement.updated_at = std::string(movement_rs.updated_at.data(), movement_rs.updated_at.length());
  if (movement_rs.completed_at.length() == 0) {
    movement.completed_at = std::nullopt;
  } else {
    movement.completed_at = std::string(movement_rs.completed_at.data(), movement_rs.completed_at.length());
  }

  movement.subsystem.name = std::string(movement_rs.subsystem_name.data(), movement_rs.subsystem_name.length());
  movement.subsystem.kind = std::string(movement_rs.subsystem_kind.data(), movement_rs.subsystem_kind.length());

  movement.sent_to.reserve(movement_rs.sent_to.size());
  for (const auto& destination_rs : movement_rs.sent_to) {
    movement.sent_to.push_back(convertRustMovementDestination(destination_rs));
  }

  movement.received_on.reserve(movement_rs.received_on.size());
  for (const auto& destination_rs : movement_rs.received_on) {
    movement.received_on.push_back(convertRustMovementDestination(destination_rs));
  }

  movement.input_vtxos.reserve(movement_rs.input_vtxos.size());
  for (const auto& vtxo_id : movement_rs.input_vtxos) {
    movement.input_vtxos.emplace_back(std::string(vtxo_id.data(), vtxo_id.length()));
  }

  movement.output_vtxos.reserve(movement_rs.output_vtxos.size());
  for (const auto& vtxo_id : movement_rs.output_vtxos) {
    movement.output_vtxos.emplace_back(std::string(vtxo_id.data(), vtxo_id.length()));
  }

  movement.exited_vtxos.reserve(movement_rs.exited_vtxos.size());
  for (const auto& vtxo_id : movement_rs.exited_vtxos) {
    movement.exited_vtxos.emplace_back(std::string(vtxo_id.data(), vtxo_id.length()));
  }

  return movement;
}

class NitroArk : public HybridNitroArkSpec {

private:
  void trackSubscription(const std::shared_ptr<HybridBarkNotificationSubscriptionSpec>& subscription) {
    std::lock_guard<std::mutex> lock(subscriptions_mutex_);
    subscriptions_.push_back(subscription);
  }

  void stopAllSubscriptions() {
    std::vector<std::shared_ptr<HybridBarkNotificationSubscriptionSpec>> active_subscriptions;
    {
      std::lock_guard<std::mutex> lock(subscriptions_mutex_);
      auto it = subscriptions_.begin();
      while (it != subscriptions_.end()) {
        if (auto subscription = it->lock()) {
          active_subscriptions.push_back(std::move(subscription));
          ++it;
        } else {
          it = subscriptions_.erase(it);
        }
      }
    }

    for (const auto& subscription : active_subscriptions) {
      try {
        subscription->stop();
      } catch (...) {
      }
    }
  }

  // Helper function to create ConfigOpts from BarkConfigOpts
  static bark_cxx::ConfigOpts createConfigOpts(const std::optional<BarkConfigOpts>& config) {
    bark_cxx::ConfigOpts config_opts;
    if (config.has_value()) {
      config_opts.ark = config->ark;
      config_opts.server_access_token = config->server_access_token.value_or("");
      config_opts.esplora = config->esplora.value_or("");
      config_opts.bitcoind = config->bitcoind.value_or("");
      config_opts.bitcoind_cookie = config->bitcoind_cookie.value_or("");
      config_opts.bitcoind_user = config->bitcoind_user.value_or("");
      config_opts.bitcoind_pass = config->bitcoind_pass.value_or("");
      config_opts.vtxo_refresh_expiry_threshold =
          static_cast<uint32_t>(config->vtxo_refresh_expiry_threshold);
      config_opts.fallback_fee_rate = static_cast<uint64_t>(config->fallback_fee_rate);
      config_opts.htlc_recv_claim_delta = static_cast<uint32_t>(config->htlc_recv_claim_delta);
      config_opts.vtxo_exit_margin = static_cast<uint32_t>(config->vtxo_exit_margin);
      config_opts.round_tx_required_confirmations = static_cast<uint32_t>(config->round_tx_required_confirmations);
    }
    return config_opts;
  }

public:
  NitroArk() : HybridObject(TAG) {
    // Initialize the Rust logger once when a NitroArk object is created.
    bark_cxx::init_logger();
  }

  ~NitroArk() override {
    stopAllSubscriptions();
  }

  // --- Management ---

  std::shared_ptr<Promise<std::string>> createMnemonic() override {
    return Promise<std::string>::async([]() {
      try {
        rust::String mnemonic_rs = bark_cxx::create_mnemonic();
        return std::string(mnemonic_rs.data(), mnemonic_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> createWallet(const std::string& datadir, const BarkCreateOpts& opts) override {
    return Promise<void>::async([datadir, opts]() {
      try {
        bark_cxx::CreateOpts create_opts;
        create_opts.regtest = opts.regtest.value_or(false);
        create_opts.signet = opts.signet.value_or(false);
        create_opts.bitcoin = opts.bitcoin.value_or(true);
        create_opts.mnemonic = opts.mnemonic;

        uint32_t birthday_height_val;
        if (opts.birthday_height.has_value()) {
          birthday_height_val = static_cast<uint32_t>(opts.birthday_height.value());
          create_opts.birthday_height = &birthday_height_val;
        } else {
          create_opts.birthday_height = nullptr;
        }

        create_opts.config = createConfigOpts(opts.config);

        bark_cxx::create_wallet(datadir, create_opts);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> loadWallet(const std::string& datadir, const BarkCreateOpts& opts) override {
    return Promise<void>::async([datadir, opts]() {
      try {
        bark_cxx::CreateOpts create_opts;
        create_opts.regtest = opts.regtest.value_or(false);
        create_opts.signet = opts.signet.value_or(false);
        create_opts.bitcoin = opts.bitcoin.value_or(true);
        create_opts.mnemonic = opts.mnemonic;

        uint32_t birthday_height_val;
        if (opts.birthday_height.has_value()) {
          birthday_height_val = static_cast<uint32_t>(opts.birthday_height.value());
          create_opts.birthday_height = &birthday_height_val;
        } else {
          create_opts.birthday_height = nullptr;
        }

        create_opts.config = createConfigOpts(opts.config);

        bark_cxx::load_wallet(datadir, create_opts);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> closeWallet() override {
    stopAllSubscriptions();

    return Promise<void>::async([]() {
      try {
        bark_cxx::close_wallet();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> refreshServer() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::refresh_server();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<bool>> isWalletLoaded() override {
    return Promise<bool>::async([]() { return bark_cxx::is_wallet_loaded(); });
  }

  std::shared_ptr<Promise<void>> syncPendingBoards() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::sync_pending_boards();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> maintenance() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::maintenance();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> maintenanceWithOnchain() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::maintenance_with_onchain();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> maintenanceDelegated() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::maintenance_delegated();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> maintenanceWithOnchainDelegated() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::maintenance_with_onchain_delegated();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> maintenanceRefresh() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::maintenance_refresh();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> sync() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::sync();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> startExitForEntireWallet() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::start_exit_for_entire_wallet();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> startExitForVtxos(const std::vector<std::string>& vtxoIds) override {
    return Promise<void>::async([vtxoIds]() {
      try {
        rust::Vec<rust::String> rust_vtxo_ids;
        rust_vtxo_ids.reserve(vtxoIds.size());
        for (const auto& vtxoId : vtxoIds) {
          rust_vtxo_ids.push_back(vtxoId);
        }
        bark_cxx::start_exit_for_vtxos(std::move(rust_vtxo_ids));
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> syncExit() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::sync_exit();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::vector<ExitProgressStatusResult>>>
  progressExits(std::optional<double> feeRateSatPerKvb) override {
    return Promise<std::vector<ExitProgressStatusResult>>::async([feeRateSatPerKvb]() {
      try {
        uint64_t feeRateVal;
        rust::Vec<bark_cxx::ExitProgressStatusResult> rust_results;
        if (feeRateSatPerKvb.has_value()) {
          feeRateVal = static_cast<uint64_t>(feeRateSatPerKvb.value());
          rust_results = bark_cxx::progress_exits(&feeRateVal);
        } else {
          rust_results = bark_cxx::progress_exits(nullptr);
        }

        std::vector<ExitProgressStatusResult> results;
        results.reserve(rust_results.size());
        for (const auto& rust_result : rust_results) {
          ExitProgressStatusResult result;
          result.vtxo_id = std::string(rust_result.vtxo_id.data(), rust_result.vtxo_id.length());
          result.state = std::string(rust_result.state.data(), rust_result.state.length());
          result.state_details = convertRustExitStateDetails(rust_result.state_details);
          if (rust_result.error.length() == 0) {
            result.error = std::nullopt;
          } else {
            result.error = std::string(rust_result.error.data(), rust_result.error.length());
          }
          results.push_back(std::move(result));
        }
        return results;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::vector<ExitVtxoResult>>> getExitVtxos() override {
    return Promise<std::vector<ExitVtxoResult>>::async([]() {
      try {
        rust::Vec<bark_cxx::ExitVtxoResult> rust_results = bark_cxx::get_exit_vtxos();
        return convertRustExitVtxosToVector(rust_results);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::vector<ExitVtxoResult>>> listClaimable() override {
    return Promise<std::vector<ExitVtxoResult>>::async([]() {
      try {
        rust::Vec<bark_cxx::ExitVtxoResult> rust_results = bark_cxx::list_claimable();
        return convertRustExitVtxosToVector(rust_results);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::optional<ExitStatusResult>>>
  getExitStatus(const std::string& vtxoId, std::optional<bool> includeHistory,
                std::optional<bool> includeTransactions) override {
    return Promise<std::optional<ExitStatusResult>>::async([vtxoId, includeHistory, includeTransactions]() {
      try {
        const bark_cxx::ExitStatusResult* status_ptr =
            bark_cxx::get_exit_status(vtxoId, includeHistory.value_or(false), includeTransactions.value_or(false));
        if (status_ptr == nullptr) {
          return std::optional<ExitStatusResult>();
        }

        std::unique_ptr<const bark_cxx::ExitStatusResult> rust_status(status_ptr);
        ExitStatusResult result;
        result.vtxo_id = std::string(rust_status->vtxo_id.data(), rust_status->vtxo_id.length());
        result.state = std::string(rust_status->state.data(), rust_status->state.length());
        result.state_details = convertRustExitStateDetails(rust_status->state_details);

        result.history.reserve(rust_status->history.size());
        for (const auto& state : rust_status->history) {
          result.history.emplace_back(std::string(state.data(), state.length()));
        }

        result.history_details.reserve(rust_status->history_details.size());
        for (const auto& stateDetails : rust_status->history_details) {
          result.history_details.push_back(convertRustExitStateDetails(stateDetails));
        }

        result.transactions.reserve(rust_status->transactions.size());
        for (const auto& rust_tx : rust_status->transactions) {
          ExitTransactionPackageResult tx;
          tx.exit_txid = std::string(rust_tx.exit_txid.data(), rust_tx.exit_txid.length());
          tx.exit_tx_hex = std::string(rust_tx.exit_tx_hex.data(), rust_tx.exit_tx_hex.length());
          tx.child_txid = std::string(rust_tx.child_txid.data(), rust_tx.child_txid.length());
          tx.child_tx_hex = std::string(rust_tx.child_tx_hex.data(), rust_tx.child_tx_hex.length());
          tx.child_origin = std::string(rust_tx.child_origin.data(), rust_tx.child_origin.length());
          tx.has_child = rust_tx.has_child;
          result.transactions.push_back(std::move(tx));
        }

        return std::optional<ExitStatusResult>(result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<bool>> hasPendingExits() override {
    return Promise<bool>::async([]() {
      try {
        return bark_cxx::has_pending_exits();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<double>> pendingExitTotal() override {
    return Promise<double>::async([]() {
      try {
        return static_cast<double>(bark_cxx::pending_exit_total());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::optional<double>>> allClaimableAtHeight() override {
    return Promise<std::optional<double>>::async([]() {
      try {
        const uint32_t* result_ptr = bark_cxx::all_claimable_at_height();
        if (result_ptr == nullptr) {
          return std::optional<double>(std::nullopt);
        }
        double value = static_cast<double>(*result_ptr);
        delete result_ptr;
        return std::optional<double>(value);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> drainExits(const std::vector<std::string>& vtxoIds,
                                                   const std::string& destinationAddress,
                                                   std::optional<double> feeRateSatPerKvb) override {
    return Promise<std::string>::async([vtxoIds, destinationAddress, feeRateSatPerKvb]() {
      try {
        rust::Vec<rust::String> rust_vtxo_ids;
        rust_vtxo_ids.reserve(vtxoIds.size());
        for (const auto& vtxoId : vtxoIds) {
          rust_vtxo_ids.push_back(vtxoId);
        }

        uint64_t feeRateVal;
        rust::String result;
        if (feeRateSatPerKvb.has_value()) {
          feeRateVal = static_cast<uint64_t>(feeRateSatPerKvb.value());
          result = bark_cxx::drain_exits(std::move(rust_vtxo_ids), destinationAddress, &feeRateVal);
        } else {
          result = bark_cxx::drain_exits(std::move(rust_vtxo_ids), destinationAddress, nullptr);
        }

        return std::string(result.data(), result.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> extractTransaction(const std::string& psbt) override {
    return Promise<std::string>::async([psbt]() {
      try {
        rust::String result = bark_cxx::extract_transaction(psbt);
        return std::string(result.data(), result.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> broadcastTransaction(const std::string& txHex) override {
    return Promise<std::string>::async([txHex]() {
      try {
        rust::String result = bark_cxx::broadcast_transaction(txHex);
        return std::string(result.data(), result.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::vector<PendingRoundStatus>>> syncPendingRounds() override {
    return Promise<std::vector<PendingRoundStatus>>::async([]() {
      try {
        rust::Vec<bark_cxx::PendingRoundStatus> statuses_rs = bark_cxx::sync_pending_rounds();
        std::vector<PendingRoundStatus> statuses;
        statuses.reserve(statuses_rs.size());
        for (const auto& status_rs : statuses_rs) {
          statuses.push_back(convertRustPendingRoundStatus(status_rs));
        }
        return statuses;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  // --- Wallet Info ---

  std::shared_ptr<Promise<BarkArkInfo>> getArkInfo() override {
    return Promise<BarkArkInfo>::async([]() {
      try {
        bark_cxx::CxxArkInfo rust_info = bark_cxx::get_ark_info();
        BarkArkInfo info;
        info.network = std::string(rust_info.network.data(), rust_info.network.length());
        info.server_pubkey = std::string(rust_info.server_pubkey.data(), rust_info.server_pubkey.length());
        info.mailbox_pubkey = std::string(rust_info.mailbox_pubkey.data(), rust_info.mailbox_pubkey.length());
        info.round_interval = static_cast<double>(rust_info.round_interval);
        info.nb_round_nonces = static_cast<double>(rust_info.nb_round_nonces);
        info.vtxo_exit_delta = static_cast<double>(rust_info.vtxo_exit_delta);
        info.vtxo_expiry_delta = static_cast<double>(rust_info.vtxo_expiry_delta);
        info.htlc_send_expiry_delta = static_cast<double>(rust_info.htlc_send_expiry_delta);
        info.max_vtxo_amount = static_cast<double>(rust_info.max_vtxo_amount);
        info.required_board_confirmations = static_cast<double>(rust_info.required_board_confirmations);
        info.min_board_amount = static_cast<double>(rust_info.min_board_amount);
        info.ln_receive_anti_dos_required = rust_info.ln_receive_anti_dos_required;
        return info;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<OffchainBalanceResult>> offchainBalance() override {
    return Promise<OffchainBalanceResult>::async([]() {
      try {
        bark_cxx::OffchainBalance rust_balance = bark_cxx::offchain_balance();
        OffchainBalanceResult balance;
        balance.spendable = static_cast<double>(rust_balance.spendable);
        balance.pending_lightning_send = static_cast<double>(rust_balance.pending_lightning_send);
        balance.claimable_lightning_receive = static_cast<double>(rust_balance.claimable_lightning_receive);
        balance.pending_in_round = static_cast<double>(rust_balance.pending_in_round);
        balance.pending_exit = static_cast<double>(rust_balance.pending_exit);
        balance.pending_board = static_cast<double>(rust_balance.pending_board);

        return balance;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<KeyPairResult>> deriveStoreNextKeypair() override {
    return Promise<KeyPairResult>::async([]() {
      try {
        bark_cxx::KeyPairResult keypair_rs = bark_cxx::derive_store_next_keypair();
        KeyPairResult keypair;
        keypair.public_key = std::string(keypair_rs.public_key.data(), keypair_rs.public_key.length());
        keypair.secret_key = std::string(keypair_rs.secret_key.data(), keypair_rs.secret_key.length());

        return keypair;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<KeyPairResult>> peekKeyPair(double index) override {
    return Promise<KeyPairResult>::async([index]() {
      try {
        uint32_t index_val = static_cast<uint32_t>(index);
        bark_cxx::KeyPairResult keypair_rs = bark_cxx::peek_keypair(index_val);
        KeyPairResult keypair;
        keypair.public_key = std::string(keypair_rs.public_key.data(), keypair_rs.public_key.length());
        keypair.secret_key = std::string(keypair_rs.secret_key.data(), keypair_rs.secret_key.length());
        return keypair;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<NewAddressResult>> newAddress() override {
    return Promise<NewAddressResult>::async([]() {
      try {
        bark_cxx::NewAddressResult address_rs = bark_cxx::new_address();
        NewAddressResult address;
        address.user_pubkey = std::string(address_rs.user_pubkey.data(), address_rs.user_pubkey.length());
        address.ark_id = std::string(address_rs.ark_id.data(), address_rs.ark_id.length());
        address.address = std::string(address_rs.address.data(), address_rs.address.length());
        return address;

      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<NewAddressResult>> peekAddress(double index) override {
    return Promise<NewAddressResult>::async([index]() {
      try {
        bark_cxx::NewAddressResult address_rs = bark_cxx::peek_address(static_cast<uint32_t>(index));
        NewAddressResult address;
        address.user_pubkey = std::string(address_rs.user_pubkey.data(), address_rs.user_pubkey.length());
        address.ark_id = std::string(address_rs.ark_id.data(), address_rs.ark_id.length());
        address.address = std::string(address_rs.address.data(), address_rs.address.length());
        return address;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> signMessage(const std::string& message, double index) override {
    return Promise<std::string>::async([message, index]() {
      try {
        uint32_t index_val = static_cast<uint32_t>(index);
        rust::String signature_rs = bark_cxx::sign_message(message, index_val);
        return std::string(signature_rs.data(), signature_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> signMesssageWithMnemonic(const std::string& message,
                                                                 const std::string& mnemonic,
                                                                 const std::string& network, double index) override {
    return Promise<std::string>::async([message, mnemonic, network, index]() {
      try {
        uint32_t index_val = static_cast<uint32_t>(index);
        rust::String signature_rs = bark_cxx::sign_messsage_with_mnemonic(message, mnemonic, network, index_val);
        return std::string(signature_rs.data(), signature_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<KeyPairResult>> deriveKeypairFromMnemonic(const std::string& mnemonic,
                                                                    const std::string& network, double index) override {
    return Promise<KeyPairResult>::async([mnemonic, network, index]() {
      try {
        uint32_t index_val = static_cast<uint32_t>(index);
        bark_cxx::KeyPairResult keypair_rs = bark_cxx::derive_keypair_from_mnemonic(mnemonic, network, index_val);
        KeyPairResult keypair;
        keypair.public_key = std::string(keypair_rs.public_key.data(), keypair_rs.public_key.length());
        keypair.secret_key = std::string(keypair_rs.secret_key.data(), keypair_rs.secret_key.length());
        return keypair;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<bool>> verifyMessage(const std::string& message, const std::string& signature,
                                               const std::string& publicKey) override {
    return Promise<bool>::async([message, signature, publicKey]() {
      try {
        return bark_cxx::verify_message(message, signature, publicKey);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<KeyPairResult>> mailboxKeypair() override {
    return Promise<KeyPairResult>::async([]() {
      try {
        bark_cxx::KeyPairResult keypair_rs = bark_cxx::mailbox_keypair();
        KeyPairResult keypair;
        keypair.public_key = std::string(keypair_rs.public_key.data(), keypair_rs.public_key.length());
        keypair.secret_key = std::string(keypair_rs.secret_key.data(), keypair_rs.secret_key.length());
        return keypair;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<MailboxAuthorizationResult>> mailboxAuthorization(double authorizationExpiry) override {
    return Promise<MailboxAuthorizationResult>::async([authorizationExpiry]() {
      try {
        int64_t expiry_val = static_cast<int64_t>(authorizationExpiry);
        bark_cxx::MailboxAuthorizationResult auth_rs = bark_cxx::mailbox_authorization(expiry_val);
        MailboxAuthorizationResult result;
        result.mailbox_id = std::string(auth_rs.mailbox_id.data(), auth_rs.mailbox_id.length());
        result.expiry = static_cast<double>(auth_rs.expiry);
        result.encoded = std::string(auth_rs.encoded.data(), auth_rs.encoded.length());
        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<HybridBarkNotificationSubscriptionSpec>
  subscribeNotifications(const std::function<void(const BarkNotificationEvent&)>& onEvent) override {
    try {
      auto subscription = std::make_shared<BarkNotificationSubscription>(
          bark_cxx::subscribe_notifications(), std::function<void(const BarkNotificationEvent&)>(onEvent));
      trackSubscription(subscription);
      return subscription;
    } catch (const rust::Error& e) {
      throw std::runtime_error(e.what());
    }
  }

  std::shared_ptr<HybridBarkNotificationSubscriptionSpec>
  subscribeArkoorAddressMovements(const std::string& address,
                                  const std::function<void(const BarkNotificationEvent&)>& onEvent) override {
    try {
      auto subscription =
          std::make_shared<BarkNotificationSubscription>(bark_cxx::subscribe_arkoor_address_movements(address),
                                                         std::function<void(const BarkNotificationEvent&)>(onEvent));
      trackSubscription(subscription);
      return subscription;
    } catch (const rust::Error& e) {
      throw std::runtime_error(e.what());
    }
  }

  std::shared_ptr<HybridBarkNotificationSubscriptionSpec>
  subscribeLightningPaymentMovements(const std::string& paymentHash,
                                     const std::function<void(const BarkNotificationEvent&)>& onEvent) override {
    try {
      auto subscription =
          std::make_shared<BarkNotificationSubscription>(bark_cxx::subscribe_lightning_payment_movements(paymentHash),
                                                         std::function<void(const BarkNotificationEvent&)>(onEvent));
      trackSubscription(subscription);
      return subscription;
    } catch (const rust::Error& e) {
      throw std::runtime_error(e.what());
    }
  }

  std::shared_ptr<Promise<std::vector<BarkMovement>>> history() override {
    return Promise<std::vector<BarkMovement>>::async([]() {
      try {
        rust::Vec<bark_cxx::BarkMovement> movements_rs = bark_cxx::history();

        std::vector<BarkMovement> movements;
        movements.reserve(movements_rs.size());

        for (const auto& movement_rs : movements_rs) {
          movements.push_back(convertRustMovement(movement_rs));
        }

        return movements;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::vector<BarkVtxo>>> vtxos() override {
    return Promise<std::vector<BarkVtxo>>::async([]() {
      try {
        rust::Vec<bark_cxx::BarkVtxo> rust_vtxos = bark_cxx::vtxos();
        return convertRustVtxosToVector(rust_vtxos);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::vector<BarkVtxo>>> getExpiringVtxos(double threshold) override {
    return Promise<std::vector<BarkVtxo>>::async([threshold]() {
      try {
        rust::Vec<bark_cxx::BarkVtxo> rust_vtxos = bark_cxx::get_expiring_vtxos(static_cast<uint32_t>(threshold));
        return convertRustVtxosToVector(rust_vtxos);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::optional<double>>> getFirstExpiringVtxoBlockheight() override {
    return Promise<std::optional<double>>::async([]() {
      try {
        const uint32_t* result_ptr = bark_cxx::get_first_expiring_vtxo_blockheight();
        if (result_ptr == nullptr) {
          return std::optional<double>(std::nullopt);
        }
        double value = static_cast<double>(*result_ptr);
        delete result_ptr; // Free the heap-allocated memory from Rust
        return std::optional<double>(value);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::optional<double>>> getNextRequiredRefreshBlockheight() override {
    return Promise<std::optional<double>>::async([]() {
      try {
        const uint32_t* result_ptr = bark_cxx::get_next_required_refresh_blockheight();
        if (result_ptr == nullptr) {
          return std::optional<double>(std::nullopt);
        }
        double value = static_cast<double>(*result_ptr);
        delete result_ptr; // Free the heap-allocated memory from Rust
        return std::optional<double>(value);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  // --- Onchain Operations ---

  std::shared_ptr<Promise<OnchainBalanceResult>> onchainBalance() override {
    return Promise<OnchainBalanceResult>::async([]() {
      try {
        bark_cxx::OnChainBalance rust_balance = bark_cxx::onchain_balance();
        OnchainBalanceResult balance;
        balance.immature = static_cast<double>(rust_balance.immature);
        balance.trusted_pending = static_cast<double>(rust_balance.trusted_pending);
        balance.untrusted_pending = static_cast<double>(rust_balance.untrusted_pending);
        balance.confirmed = static_cast<double>(rust_balance.confirmed);
        return balance;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> onchainSync() override {
    return Promise<void>::async([]() {
      try {
        bark_cxx::onchain_sync();
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> onchainListUnspent() override {
    return Promise<std::string>::async([]() {
      try {
        rust::String json_rs = bark_cxx::onchain_list_unspent();
        return std::string(json_rs.data(), json_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> onchainUtxos() override {
    return Promise<std::string>::async([]() {
      try {
        rust::String json_rs = bark_cxx::onchain_utxos();
        return std::string(json_rs.data(), json_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<BarkFeeRates>> onchainFeeRates() override {
    return Promise<BarkFeeRates>::async([]() {
      try {
        bark_cxx::BarkFeeRates rust_result = bark_cxx::onchain_fee_rates();

        BarkFeeRates result;
        result.fast = rust_result.fast;
        result.regular = rust_result.regular;
        result.slow = rust_result.slow;

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::vector<OnchainTransactionInfo>>> onchainTransactions() override {
    return Promise<std::vector<OnchainTransactionInfo>>::async([]() {
      try {
        rust::Vec<bark_cxx::OnchainTransactionInfo> rust_results = bark_cxx::onchain_transactions();

        std::vector<OnchainTransactionInfo> results;
        results.reserve(rust_results.size());

        for (const auto& rust_result : rust_results) {
          OnchainTransactionInfo result;
          result.txid = std::string(rust_result.txid.data(), rust_result.txid.length());
          result.tx_hex = std::string(rust_result.tx_hex.data(), rust_result.tx_hex.length());
          result.has_onchain_fee = rust_result.has_onchain_fee;
          result.onchain_fee_sat = static_cast<double>(rust_result.onchain_fee_sat);
          result.balance_change_sat = static_cast<double>(rust_result.balance_change_sat);
          result.has_confirmation = rust_result.has_confirmation;
          result.confirmation_height = static_cast<double>(rust_result.confirmation_height);
          result.confirmation_hash =
              std::string(rust_result.confirmation_hash.data(), rust_result.confirmation_hash.length());
          results.push_back(std::move(result));
        }

        return results;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> onchainAddress() override {
    return Promise<std::string>::async([]() {
      try {
        rust::String address_rs = bark_cxx::onchain_address();
        return std::string(address_rs.data(), address_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<OnchainPaymentResult>> onchainSend(const std::string& destination, double amountSat,
                                                             std::optional<double> feeRate) override {
    return Promise<OnchainPaymentResult>::async([destination, amountSat, feeRate]() {
      try {
        uint64_t feeRate_val;
        bark_cxx::OnchainPaymentResult rust_result;
        if (feeRate.has_value()) {
          feeRate_val = static_cast<uint64_t>(feeRate.value());
          rust_result = bark_cxx::onchain_send(destination, static_cast<uint64_t>(amountSat), &feeRate_val);
        } else {
          rust_result = bark_cxx::onchain_send(destination, static_cast<uint64_t>(amountSat), nullptr);
        }

        OnchainPaymentResult result;
        result.txid = std::string(rust_result.txid.data(), rust_result.txid.length());
        result.amount_sat = static_cast<double>(rust_result.amount_sat);
        result.destination_address =
            std::string(rust_result.destination_address.data(), rust_result.destination_address.length());

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> onchainDrain(const std::string& destination,
                                                     std::optional<double> feeRate) override {
    return Promise<std::string>::async([destination, feeRate]() {
      try {
        uint64_t feeRate_val;
        rust::String txid_rs;
        if (feeRate.has_value()) {
          feeRate_val = static_cast<uint64_t>(feeRate.value());
          txid_rs = bark_cxx::onchain_drain(destination, &feeRate_val);
        } else {
          txid_rs = bark_cxx::onchain_drain(destination, nullptr);
        }
        return std::string(txid_rs.data(), txid_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> onchainSendMany(const std::vector<BarkSendManyOutput>& outputs,
                                                        std::optional<double> feeRate) override {
    return Promise<std::string>::async([outputs, feeRate]() {
      try {
        rust::Vec<bark_cxx::SendManyOutput> cxx_outputs;
        for (const auto& output : outputs) {
          cxx_outputs.push_back({rust::String(output.destination), static_cast<uint64_t>(output.amountSat)});
        }
        uint64_t feeRate_val;
        rust::String txid_rs;
        if (feeRate.has_value()) {
          feeRate_val = static_cast<uint64_t>(feeRate.value());
          txid_rs = bark_cxx::onchain_send_many(std::move(cxx_outputs), &feeRate_val);
        } else {
          txid_rs = bark_cxx::onchain_send_many(std::move(cxx_outputs), nullptr);
        }
        return std::string(txid_rs.data(), txid_rs.length());
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  // --- Lightning Operations ---

  std::shared_ptr<Promise<LightningPaymentResult>> payLightningInvoice(const std::string& destination, bool wait,
                                                                       std::optional<double> amountSat) override {
    return Promise<LightningPaymentResult>::async([destination, wait, amountSat]() {
      try {
        bark_cxx::LightningPaymentResult rust_result;
        if (amountSat.has_value()) {
          uint64_t amountSat_val = static_cast<uint64_t>(amountSat.value());
          rust_result = bark_cxx::pay_lightning_invoice(destination, &amountSat_val, wait);
        } else {
          rust_result = bark_cxx::pay_lightning_invoice(destination, nullptr, wait);
        }

        return convertRustLightningPaymentResult(rust_result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<LightningPaymentResult>> payLightningOffer(const std::string& offer, bool wait,
                                                                     std::optional<double> amountSat) override {
    return Promise<LightningPaymentResult>::async([offer, wait, amountSat]() {
      try {
        bark_cxx::LightningPaymentResult rust_result;
        if (amountSat.has_value()) {
          uint64_t amountSat_val = static_cast<uint64_t>(amountSat.value());
          rust_result = bark_cxx::pay_lightning_offer(offer, &amountSat_val, wait);
        } else {
          rust_result = bark_cxx::pay_lightning_offer(offer, nullptr, wait);
        }

        return convertRustLightningPaymentResult(rust_result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<LightningPaymentResult>> payLightningAddress(const std::string& addr, double amountSat,
                                                                       const std::string& comment, bool wait) override {
    return Promise<LightningPaymentResult>::async([addr, amountSat, comment, wait]() {
      try {
        bark_cxx::LightningPaymentResult rust_result =
            bark_cxx::pay_lightning_address(addr, static_cast<uint64_t>(amountSat), comment, wait);

        return convertRustLightningPaymentResult(rust_result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<BarkFeeEstimate>> estimateLightningSendFee(double amountSat) override {
    return Promise<BarkFeeEstimate>::async([amountSat]() {
      try {
        bark_cxx::BarkFeeEstimate rust_result = bark_cxx::estimate_lightning_send_fee(static_cast<uint64_t>(amountSat));

        BarkFeeEstimate result;
        result.gross_amount_sat = static_cast<double>(rust_result.gross_amount_sat);
        result.fee_sat = static_cast<double>(rust_result.fee_sat);
        result.net_amount_sat = static_cast<double>(rust_result.net_amount_sat);

        std::vector<std::string> vtxos_spent;
        vtxos_spent.reserve(rust_result.vtxos_spent.size());
        for (const auto& vtxo_id : rust_result.vtxos_spent) {
          vtxos_spent.push_back(std::string(vtxo_id.data(), vtxo_id.length()));
        }
        result.vtxos_spent = vtxos_spent;

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<Bolt11Invoice>> bolt11Invoice(double amountMsat,
                                                        const std::optional<std::string>& description) override {
    return Promise<Bolt11Invoice>::async([amountMsat, description]() {
      try {
        bark_cxx::Bolt11Invoice invoice_rs;
        if (description.has_value()) {
          rust::String description_rs(description.value());
          invoice_rs = bark_cxx::bolt11_invoice(static_cast<uint64_t>(amountMsat), &description_rs);
        } else {
          invoice_rs = bark_cxx::bolt11_invoice(static_cast<uint64_t>(amountMsat), nullptr);
        }
        return Bolt11Invoice(std::string(invoice_rs.bolt11_invoice.data(), invoice_rs.bolt11_invoice.length()),
                             std::string(invoice_rs.payment_secret.data(), invoice_rs.payment_secret.length()),
                             std::string(invoice_rs.payment_hash.data(), invoice_rs.payment_hash.length()));
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<LightningReceive>>
  tryClaimLightningReceive(const std::string& paymentHash, bool wait,
                           const std::optional<std::string>& token) override {
    return Promise<LightningReceive>::async([paymentHash, wait, token]() -> LightningReceive {
      try {
        bark_cxx::LightningReceive result;
        if (token.has_value()) {
          rust::String token_rs(token.value());
          result = bark_cxx::try_claim_lightning_receive(paymentHash, wait, &token_rs);
        } else {
          result = bark_cxx::try_claim_lightning_receive(paymentHash, wait, nullptr);
        }

        LightningReceive lr;
        lr.payment_hash = std::string(result.payment_hash.data(), result.payment_hash.length());
        lr.payment_preimage = std::string(result.payment_preimage.data(), result.payment_preimage.length());
        lr.invoice = std::string(result.invoice.data(), result.invoice.length());

        if (result.preimage_revealed_at != nullptr) {
          lr.preimage_revealed_at = static_cast<double>(*result.preimage_revealed_at);
        } else {
          lr.preimage_revealed_at = std::nullopt;
        }

        if (result.finished_at != nullptr) {
          lr.finished_at = static_cast<double>(*result.finished_at);
        } else {
          lr.finished_at = std::nullopt;
        }

        return lr;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> tryClaimAllLightningReceives(bool wait) override {
    return Promise<void>::async([wait]() {
      try {
        bark_cxx::try_claim_all_lightning_receives(wait);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::optional<LightningReceive>>>
  lightningReceiveStatus(const std::string& paymentHash) override {
    return Promise<std::optional<LightningReceive>>::async([paymentHash]() {
      try {
        const bark_cxx::LightningReceive* status_ptr = bark_cxx::lightning_receive_status(paymentHash);

        if (status_ptr == nullptr) {
          return std::optional<LightningReceive>();
        }

        std::unique_ptr<const bark_cxx::LightningReceive> status(status_ptr);

        LightningReceive result;
        result.payment_hash = std::string(status->payment_hash.data(), status->payment_hash.length());
        result.payment_preimage = std::string(status->payment_preimage.data(), status->payment_preimage.length());
        result.invoice = std::string(status->invoice.data(), status->invoice.length());

        if (status->preimage_revealed_at != nullptr) {
          result.preimage_revealed_at = static_cast<double>(*status->preimage_revealed_at);
        } else {
          result.preimage_revealed_at = std::nullopt;
        }

        if (status->finished_at != nullptr) {
          result.finished_at = static_cast<double>(*status->finished_at);
        } else {
          result.finished_at = std::nullopt;
        }

        return std::optional<LightningReceive>(result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<LightningPaymentResult>> checkLightningPayment(const std::string& paymentHash,
                                                                         bool wait) override {
    return Promise<LightningPaymentResult>::async([paymentHash, wait]() {
      try {
        return convertRustLightningPaymentResult(bark_cxx::check_lightning_payment(paymentHash, wait));
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  // --- Ark Operations ---
  std::shared_ptr<Promise<BoardResult>> boardAmount(double amountSat) override {
    return Promise<BoardResult>::async([amountSat]() {
      try {
        bark_cxx::BoardResult result_rs = bark_cxx::board_amount(static_cast<uint64_t>(amountSat));
        BoardResult result;
        result.funding_txid = std::string(result_rs.funding_txid.data(), result_rs.funding_txid.length());
        std::vector<std::string> vtxos_vec;
        for (const auto& vtxo : result_rs.vtxos) {
          vtxos_vec.push_back(std::string(vtxo.data(), vtxo.length()));
        }
        result.vtxos = vtxos_vec;
        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<BoardResult>> boardAll() override {
    return Promise<BoardResult>::async([]() {
      try {
        bark_cxx::BoardResult result_rs = bark_cxx::board_all();
        BoardResult result;
        result.funding_txid = std::string(result_rs.funding_txid.data(), result_rs.funding_txid.length());
        std::vector<std::string> vtxos_vec;
        for (const auto& vtxo : result_rs.vtxos) {
          vtxos_vec.push_back(std::string(vtxo.data(), vtxo.length()));
        }
        result.vtxos = vtxos_vec;
        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<void>> validateArkoorAddress(const std::string& address) override {
    return Promise<void>::async([address]() {
      try {
        bark_cxx::validate_arkoor_address(address);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<ArkoorPaymentResult>> sendArkoorPayment(const std::string& destination,
                                                                  double amountSat) override {
    return Promise<ArkoorPaymentResult>::async([destination, amountSat]() {
      try {
        bark_cxx::ArkoorPaymentResult rust_result =
            bark_cxx::send_arkoor_payment(destination, static_cast<uint64_t>(amountSat));

        ArkoorPaymentResult result;
        result.amount_sat = static_cast<double>(rust_result.amount_sat);
        result.destination_pubkey =
            std::string(rust_result.destination_pubkey.data(), rust_result.destination_pubkey.length());

        result.vtxos = convertRustVtxosToVector(rust_result.vtxos);

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<BarkFeeEstimate>> estimateArkoorPaymentFee(double amountSat) override {
    return Promise<BarkFeeEstimate>::async([amountSat]() {
      try {
        bark_cxx::BarkFeeEstimate rust_result = bark_cxx::estimate_arkoor_payment_fee(static_cast<uint64_t>(amountSat));

        BarkFeeEstimate result;
        result.gross_amount_sat = static_cast<double>(rust_result.gross_amount_sat);
        result.fee_sat = static_cast<double>(rust_result.fee_sat);
        result.net_amount_sat = static_cast<double>(rust_result.net_amount_sat);

        std::vector<std::string> vtxos_spent;
        vtxos_spent.reserve(rust_result.vtxos_spent.size());
        for (const auto& vtxo_id : rust_result.vtxos_spent) {
          vtxos_spent.push_back(std::string(vtxo_id.data(), vtxo_id.length()));
        }
        result.vtxos_spent = vtxos_spent;

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<BarkFeeEstimate>> estimateBoardOffchainFee(double amountSat) override {
    return Promise<BarkFeeEstimate>::async([amountSat]() {
      try {
        bark_cxx::BarkFeeEstimate rust_result =
            bark_cxx::estimate_board_offchain_fee(static_cast<uint64_t>(amountSat));

        BarkFeeEstimate result;
        result.gross_amount_sat = static_cast<double>(rust_result.gross_amount_sat);
        result.fee_sat = static_cast<double>(rust_result.fee_sat);
        result.net_amount_sat = static_cast<double>(rust_result.net_amount_sat);

        std::vector<std::string> vtxos_spent;
        vtxos_spent.reserve(rust_result.vtxos_spent.size());
        for (const auto& vtxo_id : rust_result.vtxos_spent) {
          vtxos_spent.push_back(std::string(vtxo_id.data(), vtxo_id.length()));
        }
        result.vtxos_spent = vtxos_spent;

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> sendOnchain(const std::string& destination, double amountSat) override {
    return Promise<std::string>::async([destination, amountSat]() {
      try {
        rust::String result = bark_cxx::send_onchain(destination, static_cast<uint64_t>(amountSat));
        return std::string(result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<BarkFeeEstimate>> estimateSendOnchain(const std::string& destination,
                                                                double amountSat) override {
    return Promise<BarkFeeEstimate>::async([destination, amountSat]() {
      try {
        bark_cxx::BarkFeeEstimate rust_result =
            bark_cxx::estimate_send_onchain(destination, static_cast<uint64_t>(amountSat));

        BarkFeeEstimate result;
        result.gross_amount_sat = static_cast<double>(rust_result.gross_amount_sat);
        result.fee_sat = static_cast<double>(rust_result.fee_sat);
        result.net_amount_sat = static_cast<double>(rust_result.net_amount_sat);

        std::vector<std::string> vtxos_spent;
        vtxos_spent.reserve(rust_result.vtxos_spent.size());
        for (const auto& vtxo_id : rust_result.vtxos_spent) {
          vtxos_spent.push_back(std::string(vtxo_id.data(), vtxo_id.length()));
        }
        result.vtxos_spent = vtxos_spent;

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  // --- Offboarding / Exiting ---

  std::shared_ptr<Promise<std::string>> offboardSpecific(const std::vector<std::string>& vtxoIds,
                                                         const std::string& destinationAddress) override {
    return Promise<std::string>::async([vtxoIds, destinationAddress]() {
      try {
        rust::Vec<rust::String> rust_vtxo_ids;
        for (const auto& id : vtxoIds) {
          rust_vtxo_ids.push_back(rust::String(id));
        }
        rust::String result = bark_cxx::offboard_specific(std::move(rust_vtxo_ids), destinationAddress);
        return std::string(result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<std::string>> offboardAll(const std::string& destinationAddress) override {
    return Promise<std::string>::async([destinationAddress]() {
      try {
        rust::String result = bark_cxx::offboard_all(destinationAddress);
        return std::string(result);
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

  std::shared_ptr<Promise<BarkFeeEstimate>> estimateOffboardAll(const std::string& destinationAddress) override {
    return Promise<BarkFeeEstimate>::async([destinationAddress]() {
      try {
        bark_cxx::BarkFeeEstimate rust_result = bark_cxx::estimate_offboard_all(destinationAddress);

        BarkFeeEstimate result;
        result.gross_amount_sat = static_cast<double>(rust_result.gross_amount_sat);
        result.fee_sat = static_cast<double>(rust_result.fee_sat);
        result.net_amount_sat = static_cast<double>(rust_result.net_amount_sat);

        std::vector<std::string> vtxos_spent;
        vtxos_spent.reserve(rust_result.vtxos_spent.size());
        for (const auto& vtxo_id : rust_result.vtxos_spent) {
          vtxos_spent.push_back(std::string(vtxo_id.data(), vtxo_id.length()));
        }
        result.vtxos_spent = vtxos_spent;

        return result;
      } catch (const rust::Error& e) {
        throw std::runtime_error(e.what());
      }
    });
  }

private:
  // Tag for logging/debugging within Nitro
  std::mutex subscriptions_mutex_;
  std::vector<std::weak_ptr<HybridBarkNotificationSubscriptionSpec>> subscriptions_;
  static constexpr auto TAG = "NitroArk";
};

} // namespace margelo::nitro::nitroark
