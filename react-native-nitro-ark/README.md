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

### Emergency exit estimates

Estimate selected VTXOs before starting an exit or funding the onchain wallet:

```ts
import { estimateEmergencyExitFee, syncExit } from 'react-native-nitro-ark';

await syncExit();
const estimate = await estimateEmergencyExitFee(
  vtxoIds,
  1500, // Optional sat/kvB override: 1.5 sat/vB, applied to broadcast and claim
  undefined, // Optional claim destination; defaults to a P2TR output
  1.5 // Optional broadcast multiplier: adds 50%
);
```

Omit `feeMargin` for Bark's default `1.2` multiplier. `1` adds no margin; `0` produces a zero broadcast estimate. Values must be finite and nonnegative. The multiplier affects only `exit_broadcast_fee_sat`; Bark rejects calculated fees that exceed its amount limits. Fee-rate overrides must be nonnegative safe integers in sat/kvB.

`exit_broadcast_fee_sat` is the funding target from confirmed onchain funds. `claim_fee_sat` is deducted from recovered funds later. `total_fee_sat` is their sum. `fee_rate_sat_per_vb` reports the base broadcast rate before the multiplier, preserving fractional values; without an override, the claim uses a separate regular rate. `txs_to_broadcast` counts transactions still requiring broadcast/CPFP.

Already-confirmed transactions and sufficiently funded mempool packages avoid additional broadcast costs. Call `syncExit()` for fresh tracked chain state. Estimating does not start or progress an exit, and an empty VTXO list estimates zero fees. The example's **Exit Fee Estimate** section exposes these inputs and the funding breakdown.

### Swept exit details

When a required exit-chain input was spent onchain, Bark reports the terminal
`VtxoSwept` state with `state_details.kind === 'vtxo-swept'`. Its
`spent_inputs` field contains outpoint strings (`txid:vout`), including an empty
array when none are reported. The field is undefined for other state kinds.

```ts
import { getExitStatus } from 'react-native-nitro-ark';

const status = await getExitStatus(vtxoId, true, false);
if (status?.state_details.kind === 'vtxo-swept') {
  console.log(status.state_details.spent_inputs);
}
```

The field is preserved in exit progress, VTXO listings, and detailed status,
including `history_details` when available. Use `getExitStatus()` to inspect a
finished exit: finished exits are no longer actively tracked after loading the
wallet. The example's **Exit Overview → Get Exit Status** shows the spent inputs
and explains that the exit cannot continue. A delegated refresh may still be
possible, depending on what spent the inputs; it is not guaranteed to succeed.

### VTXO recovery key gap

Set `config.vtxo_key_gap_limit` when creating or opening a wallet to control how many consecutive unused key indices recovery and VTXO imports scan. It defaults to 250 when omitted. Increase it for wallets that issued many addresses without receiving into them. Values must be integers from 0 to 100,000; larger scans take more work.

### Restore from the Ark server

`restoreWalletFromArkServer(datadir, opts)` opens a fresh wallet using the original mnemonic and scans both seed-linked mailboxes. It then replays regular mailbox messages and syncs pending rounds, including delegated outputs completed while local data was missing. `createWallet` and `loadWallet` retain their existing behavior.

```ts
import { restoreWalletFromArkServer } from 'react-native-nitro-ark';

const result = await restoreWalletFromArkServer(
  newDirectory,
  optionsWithOriginalSeed
);
if (result.status === 'failed' || !result.report?.is_complete) {
  // Preserve this directory and inspect result.error and the report's candidates.
}
```

Close any loaded wallet first. The destination must be absent or empty; existing data is never overwritten. Validation and initialization errors reject the promise and may leave a partial database. Once the call resolves, the wallet is loaded even if `status` is `failed`.

`completed` means the scan and follow-up sync calls finished. Check `report.is_complete` separately: `failed` candidates could not be decided; `foreign` candidates could not be matched within the key gap. The report also groups `recovered`, `skipped` (spent or in-flight), and `exited` VTXOs. Each group contains `vtxo_ids` and `known_amount_sat`; unknown amounts are excluded from the sum. This is the seed scan's report, not a delegated-round completion guarantee; inspect `syncPendingRounds()` for remaining rounds. Follow-up RPC failures return `status: 'failed'` while preserving any scan report.

Recovery trusts the server's records and status responses. An empty or complete report does not prove the server supplied every VTXO. Onchain recovery still requires `onchainSync()`; `birthday_height` does not change this scan. Existing-wallet rescans are unavailable with the pinned Bark API; repeat a full scan in another fresh directory, preserving previous data. The example app has a masked seed input and restore results under **Restore from Ark Server**.

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
