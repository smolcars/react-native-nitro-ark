# react-native-nitro-ark

Pure C++ Nitro Modules for Ark client

## Installation

```sh
npm install react-native-nitro-ark react-native-nitro-modules
```

> `react-native-nitro-modules` is required as this library relies on [Nitro Modules](https://nitro.margelo.com/).

## Usage

- Please check the [`src/index.tsx`](./src/index.tsx) file for all methods and type definitions.

### Wallet debug info

After loading a wallet, call `debugInfo()` to inspect its public identity:

```ts
import { debugInfo } from 'react-native-nitro-ark';

const { network, mailbox_id, vtxo_xpub } = await debugInfo();
```

All three fields are strings. `vtxo_xpub` is the Base58 extended public key used to derive VTXO keys. This call reads local wallet data and rejects when no wallet is loaded. The example app exposes it under **Wallet Diagnostics → Debug Info**.

### Lightning retry duration

Each Lightning send method accepts an optional final `retryForSeconds` argument:

```ts
import { payLightningInvoice } from 'react-native-nitro-ark';

await payLightningInvoice(invoice, false, undefined, 30);
await payLightningInvoice(invoice, true, undefined, 0);
```

Omit it for the server default. `0` requests one attempt; positive values request a retry duration capped by the server. Values must be whole seconds from 0 to 4,294,967,295. Invalid values reject before starting a payment. `wait` independently controls whether the call waits for completion. Resumed payments keep the duration they started with.

This applies to `payLightningInvoice`, `payLightningOffer`, `payLightningAddress`, and `payLightningInvoiceWithOrigin`. Retry control requires a server supporting this option. The example's **Lightning Payments** section provides retry duration and wait controls.

### VTXO recovery key gap

Set `config.vtxo_key_gap_limit` when creating or opening a wallet to control how many consecutive unused key indices recovery and VTXO imports scan. It defaults to 250 when omitted. Increase it for wallets that issued many addresses without receiving into them. Values must be integers from 0 to 100,000; larger scans take more work.

### Wallet snapshots

`createWalletSnapshot` uses SQLite Online Backup to create a consistent database image while the wallet remains loaded. The destination's parent directory must exist, and an existing destination is never overwritten.

```ts
import {
  createWalletSnapshot,
  subscribeWalletStateChanges,
  validateWalletSnapshot,
} from 'react-native-nitro-ark';

const snapshot = await createWalletSnapshot(snapshotPath);
// Persist snapshot.sha256 only after uploading snapshot.path successfully.

await validateWalletSnapshot(snapshot.path, {
  network: snapshot.network,
  walletFingerprint: snapshot.walletFingerprint,
});

const subscription = subscribeWalletStateChanges((event) => {
  // Debounce databaseChanged events before creating the next snapshot.
  // An initial event is emitted so clients can reconcile on startup.
});

subscription.stop();
```

State-change `sequence` values are local to each subscription and reset on restart. They are backup scheduling signals, not durable wallet generations. Clients should compare the SHA-256 returned by a newly created startup snapshot with the last successfully uploaded snapshot.

## License

MIT

---

Made with [create-react-native-library](https://github.com/callstack/react-native-builder-bob)
