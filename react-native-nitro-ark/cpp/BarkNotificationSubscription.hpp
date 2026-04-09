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
      : HybridObject(TAG), state_(std::make_shared<State>(std::move(subscription), std::move(onEvent))) {
    state_->worker = std::thread([state = state_]() { pumpEvents(state); });
  }

  ~BarkNotificationSubscription() override {
    stopInternal();
  }

  void stop() override {
    stopInternal();
  }

  bool isActive() override {
    if (!state_ || !state_->isActive.load()) {
      return false;
    }

    std::lock_guard<std::mutex> lock(state_->subscriptionMutex);
    return state_->subscription->is_active();
  }

private:
  struct State {
    State(rust::Box<bark_cxx::NotificationSubscription> nextSubscription,
          std::function<void(const BarkNotificationEvent&)>&& nextOnEvent)
        : subscription(std::move(nextSubscription)), onEvent(std::move(nextOnEvent)) {}

    rust::Box<bark_cxx::NotificationSubscription> subscription;
    std::function<void(const BarkNotificationEvent&)> onEvent;
    std::thread worker;
    std::atomic<bool> isActive{true};
    std::atomic<bool> cleanupScheduled{false};
    std::mutex subscriptionMutex;
  };

  void stopInternal() {
    if (!state_) {
      return;
    }

    state_->isActive.store(false);
    scheduleCleanup(state_);
  }

  static void scheduleCleanup(const std::shared_ptr<State>& state) {
    if (state->cleanupScheduled.exchange(true)) {
      return;
    }

    std::thread([state]() {
      try {
        std::lock_guard<std::mutex> lock(state->subscriptionMutex);
        if (state->subscription->is_active()) {
          state->subscription->stop();
        }
      } catch (...) {
      }

      joinWorker(state);
    }).detach();
  }

  static void joinWorker(const std::shared_ptr<State>& state) {
    if (!state->worker.joinable()) {
      return;
    }

    if (state->worker.get_id() == std::this_thread::get_id()) {
      state->worker.detach();
      return;
    }

    state->worker.join();
  }

  static void pumpEvents(const std::shared_ptr<State>& state) {
    try {
      while (state->isActive.load()) {
        bark_cxx::NotificationPollResult result;
        {
          std::lock_guard<std::mutex> lock(state->subscriptionMutex);
          result = state->subscription->wait_next(250);
        }

        if (!state->isActive.load()) {
          break;
        }

        if (result.has_event) {
          state->onEvent(convertEvent(result.event));
        }

        if (!result.is_active) {
          state->isActive.store(false);
          break;
        }
      }
    } catch (...) {
      state->isActive.store(false);
    }
  }

  static BarkMovementDestination convertDestination(const bark_cxx::BarkMovementDestination& destinationRs) {
    BarkMovementDestination destination;
    destination.destination = std::string(destinationRs.destination.data(), destinationRs.destination.length());
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

    movement.subsystem.name = std::string(movementRs.subsystem_name.data(), movementRs.subsystem_name.length());
    movement.subsystem.kind = std::string(movementRs.subsystem_kind.data(), movementRs.subsystem_kind.length());

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
  std::shared_ptr<State> state_;
};

} // namespace margelo::nitro::nitroark
