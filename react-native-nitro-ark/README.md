# react-native-nitro-ark

Pure C++ Nitro Modules for Ark client

## Installation

```sh
npm install react-native-nitro-ark react-native-nitro-modules
```

> `react-native-nitro-modules` is required as this library relies on [Nitro Modules](https://nitro.margelo.com/).

## Usage

- Please check the [`src/index.tsx`](./src/index.tsx) file for all methods and type definitions.

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
