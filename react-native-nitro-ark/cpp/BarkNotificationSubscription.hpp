#pragma once

#include "BarkNotificationEvent.hpp"
#include "HybridBarkNotificationSubscriptionSpec.hpp"
#include "generated/ark_cxx.h"
#include <atomic>
#include <functional>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <thread>
#include <utility>

namespace margelo::nitro::nitroark {

class BarkNotificationSubscription final : public HybridBarkNotificationSubscriptionSpec {
public:
  BarkNotificationSubscription(rust::Box<bark_cxx::NotificationSubscription> subscription,
                               std::function<void(const BarkNotificationEvent&)>&& onEvent)
      : HybridObject(TAG), subscription_(std::move(subscription)), onEvent_(std::move(onEvent)),
        worker_([this]() { pumpEvents(); }) {}

  ~BarkNotificationSubscription() override {
    stopInternal(false);
  }

  void stop() override {
    stopInternal(true);
  }

  bool isActive() override {
    if (!isActive_.load()) {
      return false;
    }

    std::lock_guard<std::mutex> lock(subscriptionMutex_);
    return subscription_->is_active();
  }

private:
  void stopInternal(bool rethrowErrors) {
    const bool wasActive = isActive_.exchange(false);

    if (wasActive) {
      try {
        std::lock_guard<std::mutex> lock(subscriptionMutex_);
        subscription_->stop();
      } catch (const std::exception& error) {
        if (rethrowErrors) {
          throw std::runtime_error(error.what());
        }
      }
    }

    joinWorker();
  }

  void joinWorker() {
    if (!worker_.joinable()) {
      return;
    }

    if (worker_.get_id() == std::this_thread::get_id()) {
      worker_.detach();
      return;
    }

    worker_.join();
  }

  void pumpEvents() {
    try {
      while (isActive_.load()) {
        bark_cxx::NotificationPollResult result;
        {
          std::lock_guard<std::mutex> lock(subscriptionMutex_);
          result = subscription_->wait_next(250);
        }

        if (!isActive_.load()) {
          break;
        }

        if (result.has_event) {
          onEvent_(convertEvent(result.event));
        }

        if (!result.is_active) {
          isActive_.store(false);
          break;
        }
      }
    } catch (...) {
      isActive_.store(false);
    }
  }

  static BarkMovementDestination convertDestination(const bark_cxx::BarkMovementDestination& destinationRs) {
    BarkMovementDestination destination;
    destination.destination =
        std::string(destinationRs.destination.data(), destinationRs.destination.length());
    destination.payment_method =
        std::string(destinationRs.payment_method.data(), destinationRs.payment_method.length());
    destination.amount_sat = static_cast<double>(destinationRs.amount_sat);
    return destination;
  }

  static BarkMovement convertMovement(const bark_cxx::BarkMovement& movementRs) {
    BarkMovement movement;
    movement.id = static_cast<double>(movementRs.id);
    movement.status = std::string(movementRs.status.data(), movementRs.status.length());
    movement.metadata_json = std::string(movementRs.metadata_json.data(), movementRs.metadata_json.length());
    movement.intended_balance_sat = static_cast<double>(movementRs.intended_balance_sat);
    movement.effective_balance_sat = static_cast<double>(movementRs.effective_balance_sat);
    movement.offchain_fee_sat = static_cast<double>(movementRs.offchain_fee_sat);
    movement.created_at = std::string(movementRs.created_at.data(), movementRs.created_at.length());
    movement.updated_at = std::string(movementRs.updated_at.data(), movementRs.updated_at.length());

    if (movementRs.completed_at.length() == 0) {
      movement.completed_at = std::nullopt;
    } else {
      movement.completed_at = std::string(movementRs.completed_at.data(), movementRs.completed_at.length());
    }

    movement.subsystem.name =
        std::string(movementRs.subsystem_name.data(), movementRs.subsystem_name.length());
    movement.subsystem.kind =
        std::string(movementRs.subsystem_kind.data(), movementRs.subsystem_kind.length());

    movement.sent_to.reserve(movementRs.sent_to.size());
    for (const auto& destinationRs : movementRs.sent_to) {
      movement.sent_to.push_back(convertDestination(destinationRs));
    }

    movement.received_on.reserve(movementRs.received_on.size());
    for (const auto& destinationRs : movementRs.received_on) {
      movement.received_on.push_back(convertDestination(destinationRs));
    }

    movement.input_vtxos.reserve(movementRs.input_vtxos.size());
    for (const auto& vtxoId : movementRs.input_vtxos) {
      movement.input_vtxos.emplace_back(std::string(vtxoId.data(), vtxoId.length()));
    }

    movement.output_vtxos.reserve(movementRs.output_vtxos.size());
    for (const auto& vtxoId : movementRs.output_vtxos) {
      movement.output_vtxos.emplace_back(std::string(vtxoId.data(), vtxoId.length()));
    }

    movement.exited_vtxos.reserve(movementRs.exited_vtxos.size());
    for (const auto& vtxoId : movementRs.exited_vtxos) {
      movement.exited_vtxos.emplace_back(std::string(vtxoId.data(), vtxoId.length()));
    }

    return movement;
  }

  static BarkNotificationEvent convertEvent(const bark_cxx::NotificationEvent& eventRs) {
    BarkNotificationEvent event;
    event.kind = std::string(eventRs.kind.data(), eventRs.kind.length());

    if (eventRs.has_movement) {
      event.movement = convertMovement(eventRs.movement);
    } else {
      event.movement = std::nullopt;
    }

    return event;
  }

private:
  rust::Box<bark_cxx::NotificationSubscription> subscription_;
  std::function<void(const BarkNotificationEvent&)> onEvent_;
  std::thread worker_;
  std::atomic<bool> isActive_{true};
  std::mutex subscriptionMutex_;
};

} // namespace margelo::nitro::nitroark
