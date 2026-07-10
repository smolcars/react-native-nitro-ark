#pragma once

#include "HybridWalletStateChangeSubscriptionSpec.hpp"
#include "WalletStateChangeEvent.hpp"
#include "generated/ark_cxx.h"
#include <atomic>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>
#include <utility>

namespace margelo::nitro::nitroark {

class WalletStateChangeSubscription final : public HybridWalletStateChangeSubscriptionSpec {
public:
  WalletStateChangeSubscription(rust::Box<bark_cxx::StateChangeSubscription> subscription,
                                std::function<void(const WalletStateChangeEvent&)>&& onEvent)
      : HybridObject(TAG), state_(std::make_shared<State>(std::move(subscription), std::move(onEvent))) {
    state_->worker = std::thread([state = state_]() { pumpEvents(state); });
  }

  ~WalletStateChangeSubscription() override {
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
    State(rust::Box<bark_cxx::StateChangeSubscription> nextSubscription,
          std::function<void(const WalletStateChangeEvent&)>&& nextOnEvent)
        : subscription(std::move(nextSubscription)), onEvent(std::move(nextOnEvent)) {}

    rust::Box<bark_cxx::StateChangeSubscription> subscription;
    std::function<void(const WalletStateChangeEvent&)> onEvent;
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
        bark_cxx::StateChangePollResult result;
        {
          std::lock_guard<std::mutex> lock(state->subscriptionMutex);
          result = state->subscription->wait_next(250);
        }
        if (!state->isActive.load()) {
          break;
        }
        if (result.has_event) {
          WalletStateChangeEvent event;
          event.sequence = static_cast<double>(result.event.sequence);
          event.reason = std::string(result.event.reason.data(), result.event.reason.length());
          state->onEvent(event);
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

  std::shared_ptr<State> state_;
};

} // namespace margelo::nitro::nitroark
