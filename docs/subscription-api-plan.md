# Bark Subscription API Plan

## Summary

The new Bark release adds `Wallet::subscribe_notifications()`, which returns a Rust `Stream` of wallet notifications. This is the first streaming/subscription API we would expose through this stack.

This is implementable with the current architecture, but not as a direct Rust `Stream` all the way into TypeScript.

The recommended design is:

1. Rust owns the real Bark subscription stream.
2. Rust exposes an opaque subscription handle over `cxx`.
3. C++ owns the Nitro callback and a worker loop/thread.
4. JavaScript receives events through a Nitro callback, not a React Native `EventEmitter`.

## What Bark Provides

In the new Bark release, `Wallet::subscribe_notifications()` returns a `NotificationStream` backed by a Tokio broadcast channel.

Current notification variants:

- `MovementCreated`
- `MovementUpdated`
- `ChannelLagging`

The stream also has helpers:

- `movements()`
- `filter_arkoor_address_movements(address)`
- `filter_lightning_payment_movements(paymentHash)`

Relevant upstream file:

- `bark/src/notification.rs`

## Important Constraints

### 1. Bark uses Rust streams

Bark gives us a Tokio/futures stream. That is good for Rust, but it is not something we can expose directly through our current `cxx` bridge as a stream type.

### 2. `cxx` does not support direct stream bridging

Current `cxx` does not provide first-class async stream FFI. Their documented workaround is channel-based bridging and opaque handle types.

That means we should not try to make `cxx` directly expose a Rust `Stream` to C++.

### 3. Nitro does support long-lived callbacks

Nitro callbacks can be stored in native memory and invoked later. Nitro explicitly treats events as long-lived callbacks, so we do not need a separate RN event-emitter pattern for this.

This is the key design point: Nitro can model the JS side of subscriptions cleanly.

## Recommended Public API

I recommend a callback-based subscription API in Nitro.

Example TypeScript shape:

```ts
export interface BarkNotificationSubscription
  extends HybridObject<{ ios: 'c++'; android: 'c++' }> {
  stop(): void;
  isActive(): boolean;
}

export interface BarkNotificationEvent {
  kind: 'movementCreated' | 'movementUpdated' | 'channelLagging';
  movement?: BarkMovement;
}

export interface NitroArk extends HybridObject<{ ios: 'c++'; android: 'c++' }> {
  subscribeNotifications(
    onEvent: (event: BarkNotificationEvent) => void
  ): BarkNotificationSubscription;

  subscribeArkoorAddressMovements(
    address: string,
    onEvent: (movement: BarkMovement) => void
  ): BarkNotificationSubscription;

  subscribeLightningPaymentMovements(
    paymentHash: string,
    onEvent: (movement: BarkMovement) => void
  ): BarkNotificationSubscription;
}
```

This gives us:

- explicit subscription start
- explicit subscription stop
- deterministic cleanup
- no JS polling loop
- no React Native event emitter dependency

## Bridging Design

## Rust Layer

Rust should own the actual Bark subscription stream.

Add an opaque Rust subscription type, for example:

```rust
pub struct NotificationSubscription { ... }
```

Responsibilities:

- create the Bark stream from the loaded wallet
- apply any requested Bark-side filter
- spawn a Tokio task that consumes the Bark stream
- forward flattened events into an internal channel
- expose blocking/non-stream APIs that `cxx` can call

Suggested Rust-side exported functions:

- `subscribe_notifications() -> Box<NotificationSubscription>`
- `subscribe_arkoor_address_movements(address: &str) -> Box<NotificationSubscription>`
- `subscribe_lightning_payment_movements(payment_hash: &str) -> Box<NotificationSubscription>`
- `wait_next(timeout_ms: u32) -> NotificationPollResult`
- `stop()`
- `is_active() -> bool`

### Why a channel-backed handle?

Because `cxx` can bridge opaque Rust types and ordinary function calls, but not direct Rust streams. So the subscription object should hide the stream and expose ordinary methods instead.

## C++ Layer

C++ should own:

- the opaque Rust subscription handle
- the Nitro callback
- the worker thread / loop
- cancellation state

Suggested C++ object:

- `BarkNotificationSubscription`

Responsibilities:

- keep the JS callback alive
- repeatedly call Rust `wait_next(...)`
- forward received events into the Nitro callback
- stop cleanly on demand
- stop on destruction

Worker loop behavior:

1. C++ creates the Rust subscription handle.
2. C++ stores the Nitro callback.
3. C++ starts a worker thread.
4. The worker thread repeatedly calls `wait_next(timeout_ms)`.
5. If an item arrives, it invokes the Nitro callback.
6. If stopped, it exits and joins cleanly.

## TypeScript / Nitro Layer

Nitro should expose subscriptions as ordinary callback-based methods returning a subscription handle.

That is a better fit than polling from JS because:

- Nitro already supports durable callbacks
- JS gets a natural subscription API
- start/stop semantics are explicit
- native side controls stream consumption

## Lifecycle Requirements

This is the most important non-obvious part.

Current wallet lifecycle in `bark-cpp/src/lib.rs`:

- wallet context is stored in a global `WalletManager`
- `close_wallet()` just drops the current wallet context

If subscriptions are added, cleanup must be defined clearly.

Required behavior:

- `subscription.stop()` must be explicit and idempotent
- subscription destructor must stop itself
- `closeWallet()` must stop all active subscriptions before dropping the wallet

Without this, we risk:

- leaked threads
- callbacks firing after wallet shutdown
- subscription tasks trying to use dropped wallet state

## Suggested First Scope

Do not start with the raw unfiltered stream unless necessary.

Lower-risk first version:

- `subscribeArkoorAddressMovements(address, callback)`
- `subscribeLightningPaymentMovements(paymentHash, callback)`
- `stop()`

Why:

- movement-only subscriptions are simpler than a tagged raw notification union
- the raw stream includes `ChannelLagging`, which requires an extra event shape
- Bark already provides the filtering helpers upstream

After that, add:

- `subscribeNotifications(callback)` for full raw access

## Event Shape Recommendation

For filtered movement subscriptions:

```ts
subscribeArkoorAddressMovements(
  address: string,
  onMovement: (movement: BarkMovement) => void
): BarkNotificationSubscription
```

For raw notifications:

```ts
export interface BarkNotificationEvent {
  kind: 'movementCreated' | 'movementUpdated' | 'channelLagging';
  movement?: BarkMovement;
}
```

This keeps the JS API simple and avoids leaking Rust enum structure directly.

## Implementation Notes

Likely files to touch:

- `bark-cpp/src/lib.rs`
- `bark-cpp/src/cxx.rs`
- new Rust file such as `bark-cpp/src/subscriptions.rs`
- `react-native-nitro-ark/src/NitroArk.nitro.ts`
- `react-native-nitro-ark/src/index.tsx`
- `react-native-nitro-ark/cpp/NitroArk.hpp`
- likely a new C++ subscription class/header in `react-native-nitro-ark/cpp/`

## Final Recommendation

Recommended first implementation:

- `subscribeArkoorAddressMovements(address, cb): BarkNotificationSubscription`
- `subscribeLightningPaymentMovements(paymentHash, cb): BarkNotificationSubscription`
- `stop()` on the returned handle

Recommended later:

- `subscribeNotifications(cb)` for full raw access including lag notifications

This design matches:

- Bark’s stream-based Rust API
- `cxx` limitations around async streams
- Nitro’s support for long-lived callbacks/events
