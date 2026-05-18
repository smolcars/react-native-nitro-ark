import { NitroModules } from 'react-native-nitro-modules';
import type {
  NitroArk,
  BarkCreateOpts,
  BarkArkInfo,
  Bolt11Invoice,
  BarkSendManyOutput,
  ArkoorPaymentResult,
  BarkFeeEstimate,
  ExitProgressStatusResult as NitroExitProgressStatusResult,
  ExitVtxoResult as NitroExitVtxoResult,
  ExitStatusResult as NitroExitStatusResult,
  ExitTransactionPackageResult as NitroExitTransactionPackageResult,
  LightningSendResult,
  OnchainPaymentResult,
  OffchainBalanceResult,
  OnchainBalanceResult,
  NewAddressResult,
  KeyPairResult,
  MailboxAuthorizationResult,
  LightningReceive,
  BarkNotificationEvent as NitroBarkNotificationEvent,
  BarkNotificationSubscription,
  BarkMovement as NitroBarkMovement,
  BarkMovementDestination as NitroBarkMovementDestination,
  BoardResult,
} from './NitroArk.nitro';

export type VtxoState = 'Spendable' | 'Spent' | 'Locked' | 'unknown';

export type BarkVtxo = {
  amount: number; // u64
  expiry_height: number; // u32
  server_pubkey: string;
  exit_delta: number; // u16
  anchor_point: string;
  point: string;
  state: VtxoState;
};

export type MovementStatus = 'pending' | 'successful' | 'failed' | 'canceled';

export type ExitProgressState =
  | 'Start'
  | 'Processing'
  | 'AwaitingDelta'
  | 'Claimable'
  | 'ClaimInProgress'
  | 'Claimed';

export type ExitProgressStatusResult = Omit<
  NitroExitProgressStatusResult,
  'state'
> & {
  state: ExitProgressState;
};

export type ExitVtxoResult = Omit<NitroExitVtxoResult, 'state' | 'history'> & {
  state: ExitProgressState;
  history: ExitProgressState[];
};

export type ExitTransactionPackageResult = NitroExitTransactionPackageResult;

export type ExitStatusResult = Omit<
  NitroExitStatusResult,
  'state' | 'history'
> & {
  state: ExitProgressState;
  history: ExitProgressState[];
  transactions: ExitTransactionPackageResult[];
};

export type BarkMovementDestination = NitroBarkMovementDestination & {
  payment_method:
    | 'ark'
    | 'bitcoin'
    | 'output-script'
    | 'invoice'
    | 'offer'
    | 'lightning-address'
    | 'custom';
};

export type BarkMovement = NitroBarkMovement & {
  status: MovementStatus;
  sent_to: BarkMovementDestination[];
  received_on: BarkMovementDestination[];
};

export type BarkNotificationKind =
  | 'movementCreated'
  | 'movementUpdated'
  | 'channelLagging';

export type BarkNotificationEvent = Omit<
  NitroBarkNotificationEvent,
  'kind' | 'movement'
> & {
  kind: BarkNotificationKind;
  movement?: BarkMovement;
};

// Create the hybrid object instance
export const NitroArkHybridObject =
  NitroModules.createHybridObject<NitroArk>('NitroArk');

// --- Management ---

/**
 * Creates a new BIP39 mnemonic phrase.
 * @returns A promise resolving to the mnemonic string.
 */
export function createMnemonic(): Promise<string> {
  return NitroArkHybridObject.createMnemonic();
}

/**
 * Creates a new wallet at the specified directory.
 * @param datadir Path to the data directory.
 * @param opts The options for wallet creation.
 * @returns A promise that resolves on success or rejects on error.
 */
export function createWallet(
  datadir: string,
  opts: BarkCreateOpts
): Promise<void> {
  return NitroArkHybridObject.createWallet(datadir, opts);
}

/**
 * Loads an existing wallet or creates a new one at the specified directory.
 * Once loaded, the wallet state is managed internally.
 * @param datadir Path to the data directory.
 * @param config The configuration options for the wallet.
 * @returns A promise that resolves on success or rejects on error.
 */
export function loadWallet(
  datadir: string,
  config: BarkCreateOpts
): Promise<void> {
  return NitroArkHybridObject.loadWallet(datadir, config);
}

/**
 * Closes the currently loaded wallet, clearing its state from memory.
 * @returns A promise that resolves on success or rejects on error.
 */
export function closeWallet(): Promise<void> {
  return NitroArkHybridObject.closeWallet();
}

/**
 * Refreshes the server state.
 * @returns A promise that resolves on success or rejects on error.
 */
export function refreshServer(): Promise<void> {
  return NitroArkHybridObject.refreshServer();
}

/**
 * Checks if a wallet is currently loaded.
 * @returns A promise resolving to true if a wallet is loaded, false otherwise.
 */
export function isWalletLoaded(): Promise<boolean> {
  return NitroArkHybridObject.isWalletLoaded();
}

/**
 * Registers all confirmed boards with the server.
 * @returns A promise that resolves on success.
 */
export function syncPendingBoards(): Promise<void> {
  return NitroArkHybridObject.syncPendingBoards();
}

/**
 * Runs wallet maintenance tasks for offchain.
 * This includes refreshing vtxos that need to be refreshed.
 * @returns A promise that resolves on success.
 */
export function maintenance(): Promise<void> {
  return NitroArkHybridObject.maintenance();
}

/**
 * Runs wallet maintenance tasks for both offchain and onchain.
 * This includes refreshing vtxos that need to be refreshed.
 * @returns A promise that resolves on success.
 */
export function maintenanceWithOnchain(): Promise<void> {
  return NitroArkHybridObject.maintenanceWithOnchain();
}

/**
 * Runs delegated wallet maintenance tasks for offchain.
 * This includes refreshing vtxos that need to be refreshed using delegated signing.
 * @returns A promise that resolves on success.
 */
export function maintenanceDelegated(): Promise<void> {
  return NitroArkHybridObject.maintenanceDelegated();
}

/**
 * Runs delegated wallet maintenance tasks for both offchain and onchain.
 * This includes refreshing vtxos that need to be refreshed using delegated signing.
 * @returns A promise that resolves on success.
 */
export function maintenanceWithOnchainDelegated(): Promise<void> {
  return NitroArkHybridObject.maintenanceWithOnchainDelegated();
}

/**
 * Refreshes vtxos that need to be refreshed.
 * @returns A promise that resolves on success.
 */
export function maintenanceRefresh(): Promise<void> {
  return NitroArkHybridObject.maintenanceRefresh();
}

/**
 * Synchronizes the wallet with the blockchain.
 * @returns A promise that resolves on success.
 */
export function sync(): Promise<void> {
  return NitroArkHybridObject.sync();
}

/**
 * Starts unilateral exits for all eligible wallet VTXOs.
 * @returns A promise that resolves on success.
 */
export function startExitForEntireWallet(): Promise<void> {
  return NitroArkHybridObject.startExitForEntireWallet();
}

/**
 * Starts unilateral exits for selected wallet VTXOs.
 * @param vtxoIds VTXO IDs to exit.
 * @returns A promise that resolves on success.
 */
export function startExitForVtxos(vtxoIds: string[]): Promise<void> {
  return NitroArkHybridObject.startExitForVtxos(vtxoIds);
}

/**
 * Synchronizes the exit coordinator state.
 * @returns A promise that resolves on success.
 */
export function syncExit(): Promise<void> {
  return NitroArkHybridObject.syncExit();
}

/**
 * Synchronizes tracked exits without advancing their state.
 * @returns A promise that resolves on success.
 */
export function syncNoProgress(): Promise<void> {
  return NitroArkHybridObject.syncNoProgress();
}

/**
 * Progresses tracked exits by one step.
 * @param feeRateSatPerKvb Optional fee rate override in sat/kvB.
 * @returns A promise resolving to simplified exit progress entries.
 */
export function progressExits(
  feeRateSatPerKvb?: number
): Promise<ExitProgressStatusResult[]> {
  return NitroArkHybridObject.progressExits(feeRateSatPerKvb) as Promise<
    ExitProgressStatusResult[]
  >;
}

/**
 * Lists all unilateral exits currently tracked by the wallet.
 * @returns A promise resolving to simplified exit entries.
 */
export function getExitVtxos(): Promise<ExitVtxoResult[]> {
  return NitroArkHybridObject.getExitVtxos() as Promise<ExitVtxoResult[]>;
}

/**
 * Lists tracked unilateral exits that are claimable.
 * @returns A promise resolving to claimable exit entries.
 */
export function listClaimable(): Promise<ExitVtxoResult[]> {
  return NitroArkHybridObject.listClaimable() as Promise<ExitVtxoResult[]>;
}

/**
 * Gets the detailed unilateral exit status for a tracked VTXO.
 * @param vtxoId Exit VTXO ID to inspect.
 * @param includeHistory Whether to include the state transition history.
 * @param includeTransactions Whether to include exit transaction packages as hex.
 * @returns A promise resolving to the exit status, or undefined when the VTXO is not tracked as an exit.
 */
export function getExitStatus(
  vtxoId: string,
  includeHistory?: boolean,
  includeTransactions?: boolean
): Promise<ExitStatusResult | undefined> {
  return NitroArkHybridObject.getExitStatus(
    vtxoId,
    includeHistory,
    includeTransactions
  ) as Promise<ExitStatusResult | undefined>;
}

/**
 * Checks whether the wallet has exits that are still pending.
 * @returns A promise resolving to true when any tracked exit is not yet claimable.
 */
export function hasPendingExits(): Promise<boolean> {
  return NitroArkHybridObject.hasPendingExits();
}

/**
 * Returns the total amount, in sats, still waiting on pending exit confirmations.
 * @returns A promise resolving to the pending exit total in satoshis.
 */
export function pendingExitTotal(): Promise<number> {
  return NitroArkHybridObject.pendingExitTotal();
}

/**
 * Returns the earliest block height at which all tracked exits are claimable.
 * @returns A promise resolving to the claimable height, or undefined if not known yet.
 */
export function allClaimableAtHeight(): Promise<number | undefined> {
  return NitroArkHybridObject.allClaimableAtHeight();
}

/**
 * Builds a base64 PSBT that drains the selected claimable exits to an onchain address.
 * @param vtxoIds Exit VTXO IDs to claim.
 * @param destinationAddress Destination onchain address.
 * @param feeRateSatPerKvb Optional fee rate override in sat/kvB.
 * @returns A promise resolving to a base64-encoded PSBT.
 */
export function drainExits(
  vtxoIds: string[],
  destinationAddress: string,
  feeRateSatPerKvb?: number
): Promise<string> {
  return NitroArkHybridObject.drainExits(
    vtxoIds,
    destinationAddress,
    feeRateSatPerKvb
  );
}

/**
 * Extracts a raw transaction from a base64 PSBT.
 * @param psbt Base64-encoded PSBT.
 * @returns A promise resolving to transaction hex.
 */
export function extractTransaction(psbt: string): Promise<string> {
  return NitroArkHybridObject.extractTransaction(psbt);
}

/**
 * Broadcasts a raw transaction to the configured blockchain backend.
 * @param txHex Raw transaction hex.
 * @returns A promise resolving to the broadcast transaction ID.
 */
export function broadcastTransaction(txHex: string): Promise<string> {
  return NitroArkHybridObject.broadcastTransaction(txHex);
}

/**
 * Synchronizes pending rounds.
 * @returns A promise that resolves on success.
 */
export function syncPendingRounds(): Promise<void> {
  return NitroArkHybridObject.syncPendingRounds();
}

// --- Wallet Info ---

/**
 * Gets the Ark-specific information.
 * @returns A promise resolving to the BarkArkInfo object.
 */
export function getArkInfo(): Promise<BarkArkInfo> {
  return NitroArkHybridObject.getArkInfo();
}

/**
 * Gets the offchain balance for the loaded wallet.
 * @returns A promise resolving to the OffchainBalanceResult object.
 */
export function offchainBalance(): Promise<OffchainBalanceResult> {
  return NitroArkHybridObject.offchainBalance();
}

/**
 * Derives the next keypair for the store.
 * @returns A promise resolving to the KeyPairResult object.
 */
export function deriveStoreNextKeypair(): Promise<KeyPairResult> {
  return NitroArkHybridObject.deriveStoreNextKeypair();
}

/**
 * Peeks the wallet's VTXO public key (hex string).
 * @param index Index of the VTXO pubkey to retrieve.
 * @returns A promise resolving to the KeyPairResult object.
 */
export function peekKeyPair(index: number): Promise<KeyPairResult> {
  return NitroArkHybridObject.peekKeyPair(index);
}

/**
 * Peeks a derived address without advancing the wallet's address index.
 * @param index Index of the address to preview.
 * @returns A promise resolving to the NewAddressResult object.
 */
export function peekAddress(index: number): Promise<NewAddressResult> {
  return NitroArkHybridObject.peekAddress(index);
}

/**
 * Gets the wallet's Address.
 * @returns A promise resolving to NewAddressResult object.
 */
export function newAddress(): Promise<NewAddressResult> {
  return NitroArkHybridObject.newAddress();
}

/**
 * Signs a message with the private key at the specified index.
 * @param message The message to sign.
 * @param index The index of the keypair to use for signing.
 * @returns A promise resolving to the signature string.
 */
export function signMessage(message: string, index: number): Promise<string> {
  return NitroArkHybridObject.signMessage(message, index);
}

/**
 * Signs a message with the private key at the specified index.
 * @param message The message to sign.
 * @param mnemonic The BIP39 mnemonic phrase to use for signing.
 * @param network The network to use for signing.
 * @param index The index of the keypair to use for signing.
 * @returns A promise resolving to the signature string.
 */
export function signMesssageWithMnemonic(
  message: string,
  mnemonic: string,
  network: string,
  index: number
): Promise<string> {
  return NitroArkHybridObject.signMesssageWithMnemonic(
    message,
    mnemonic,
    network,
    index
  );
}

/**
 * Derives a keypair from a mnemonic.
 * @param mnemonic The mnemonic to derive the keypair from.
 * @param network The network to derive the keypair for.
 * @param index The index to derive the keypair from.
 * @returns A promise resolving to the KeyPairResult object.
 */

export function deriveKeypairFromMnemonic(
  mnemonic: string,
  network: string,
  index: number
): Promise<KeyPairResult> {
  return NitroArkHybridObject.deriveKeypairFromMnemonic(
    mnemonic,
    network,
    index
  );
}

/**
 * Verifies a signed message.
 * @param message The original message.
 * @param signature The signature to verify.
 * @param publicKey The public key corresponding to the private key used for signing.
 * @returns A promise resolving to true if the signature is valid, false otherwise.
 */
export function verifyMessage(
  message: string,
  signature: string,
  publicKey: string
): Promise<boolean> {
  return NitroArkHybridObject.verifyMessage(message, signature, publicKey);
}

/**
 * Gets the mailbox keypair for the loaded wallet.
 * @returns A promise resolving to a KeyPairResult object.
 */
export function mailboxKeypair(): Promise<KeyPairResult> {
  return NitroArkHybridObject.mailboxKeypair() as Promise<KeyPairResult>;
}

/**
 * Gets a mailbox authorization for the loaded wallet.
 * @param authorizationExpiry Unix timestamp (seconds) for when the authorization expires.
 * @returns A promise resolving to a MailboxAuthorizationResult object.
 */
export function mailboxAuthorization(
  authorizationExpiry: number
): Promise<MailboxAuthorizationResult> {
  return NitroArkHybridObject.mailboxAuthorization(
    authorizationExpiry
  ) as Promise<MailboxAuthorizationResult>;
}

/**
 * Subscribes to all Bark wallet notifications.
 * @param onEvent Callback invoked whenever a notification is emitted.
 * @returns A subscription handle that can be stopped.
 */
export function subscribeNotifications(
  onEvent: (event: BarkNotificationEvent) => void
): BarkNotificationSubscription {
  return NitroArkHybridObject.subscribeNotifications(
    onEvent as (event: NitroBarkNotificationEvent) => void
  );
}

/**
 * Subscribes to notifications related to a specific Arkoor address.
 * @param address Arkoor address to filter by.
 * @param onEvent Callback invoked whenever a matching notification is emitted.
 * @returns A subscription handle that can be stopped.
 */
export function subscribeArkoorAddressMovements(
  address: string,
  onEvent: (event: BarkNotificationEvent) => void
): BarkNotificationSubscription {
  return NitroArkHybridObject.subscribeArkoorAddressMovements(
    address,
    onEvent as (event: NitroBarkNotificationEvent) => void
  );
}

/**
 * Subscribes to notifications related to a specific Lightning payment hash.
 * @param paymentHash Lightning payment hash to filter by.
 * @param onEvent Callback invoked whenever a matching notification is emitted.
 * @returns A subscription handle that can be stopped.
 */
export function subscribeLightningPaymentMovements(
  paymentHash: string,
  onEvent: (event: BarkNotificationEvent) => void
): BarkNotificationSubscription {
  return NitroArkHybridObject.subscribeLightningPaymentMovements(
    paymentHash,
    onEvent as (event: NitroBarkNotificationEvent) => void
  );
}

/**
 * Gets a paginated list of wallet history (balance changes).
 * @returns A promise resolving to an array of BarkMovement objects.
 */
export function history(): Promise<BarkMovement[]> {
  return NitroArkHybridObject.history() as Promise<BarkMovement[]>;
}

/**
 * Gets the list of VTXOs as a JSON string for the loaded wallet.
 * @param no_sync If true, skips synchronization with the blockchain. Defaults to false.
 * @returns A promise resolving BarkVtxo[] array.
 */
export function vtxos(): Promise<BarkVtxo[]> {
  return NitroArkHybridObject.vtxos() as Promise<BarkVtxo[]>;
}

/**
 * Gets the first expiring VTXO blockheight for the loaded wallet.
 * @returns A promise resolving to the first expiring VTXO blockheight.
 */
export function getFirstExpiringVtxoBlockheight(): Promise<number | undefined> {
  return NitroArkHybridObject.getFirstExpiringVtxoBlockheight();
}

/**
 * Gets the next required refresh blockheight for the loaded wallet for the first expiring VTXO.
 * @returns A promise resolving to the next required refresh blockheight.
 */
export function getNextRequiredRefreshBlockheight(): Promise<
  number | undefined
> {
  return NitroArkHybridObject.getNextRequiredRefreshBlockheight();
}

/**
 * Gets the list of expiring VTXOs as a JSON Object of type BarkVtxo.
 * @param threshold The block height threshold to check for expiring VTXOs.
 * @returns A promise resolving BarkVtxo[] array.
 */

export function getExpiringVtxos(threshold: number): Promise<BarkVtxo[]> {
  return NitroArkHybridObject.getExpiringVtxos(threshold) as Promise<
    BarkVtxo[]
  >;
}

// --- Onchain Operations ---

/**
 * Gets the onchain balance for the loaded wallet.
 * @returns A promise resolving to the OnchainBalanceResult object.
 */
export function onchainBalance(): Promise<OnchainBalanceResult> {
  return NitroArkHybridObject.onchainBalance();
}

/**
 * Synchronizes the onchain state of the wallet.
 * @returns A promise that resolves on success.
 */
export function onchainSync(): Promise<void> {
  return NitroArkHybridObject.onchainSync();
}

/**
 * Gets the list of unspent onchain outputs as a JSON Object of type BarkVtxo.
 * @returns A promise resolving to the JSON string of unspent outputs.
 */
export function onchainListUnspent(): Promise<string> {
  return NitroArkHybridObject.onchainListUnspent();
}

/**
 * Gets the list of onchain UTXOs as a JSON string for the loaded wallet.
 * @returns A promise resolving to the JSON string of UTXOs.
 */
export function onchainUtxos(): Promise<string> {
  return NitroArkHybridObject.onchainUtxos();
}

/**
 * Gets a fresh onchain address for the loaded wallet.
 * @returns A promise resolving to the Bitcoin address string.
 */
export function onchainAddress(): Promise<string> {
  return NitroArkHybridObject.onchainAddress();
}

/**
 * Sends funds using the onchain wallet.
 * @param destination The destination Bitcoin address.
 * @param amountSat The amount to send in satoshis.
 * @returns A promise resolving to the OnchainPaymentResult object
 */
export function onchainSend(
  destination: string,
  amountSat: number
): Promise<OnchainPaymentResult> {
  return NitroArkHybridObject.onchainSend(destination, amountSat);
}

/**
 * Sends all funds from the onchain wallet to a destination address.
 * @param destination The destination Bitcoin address.
 * @returns A promise resolving to the transaction ID string.
 */
export function onchainDrain(destination: string): Promise<string> {
  return NitroArkHybridObject.onchainDrain(destination);
}

/**
 * Sends funds to multiple recipients using the onchain wallet.
 * @param outputs An array of objects containing destination address and amountSat.
 * @returns A promise resolving to the transaction ID string.
 */
export function onchainSendMany(
  outputs: BarkSendManyOutput[]
): Promise<string> {
  return NitroArkHybridObject.onchainSendMany(outputs);
}

// --- Lightning Operations ---

/**
 * Creates a Bolt 11 invoice.
 * @param amountMsat The amount in millisatoshis for the invoice.
 * @param description Optional invoice description/memo.
 * @returns A promise resolving to Bolt11Invoice object.
 */
export function bolt11Invoice(
  amountMsat: number,
  description?: string
): Promise<Bolt11Invoice> {
  return NitroArkHybridObject.bolt11Invoice(amountMsat, description);
}

/**
 * Gets the status of a Lightning receive.
 * @param paymentHash The payment hash of the Lightning receive.
 * @returns A promise resolving to the Lightning receive status.
 */
export function lightningReceiveStatus(
  paymentHash: string
): Promise<LightningReceive | undefined> {
  return NitroArkHybridObject.lightningReceiveStatus(paymentHash);
}

/**
 * Checks if a Lightning payment has been received and returns the preimage if available.
 * @param paymentHash The payment hash of the Lightning payment.
 * @param wait Whether to wait for the payment to be received.
 * @returns A promise resolving to the preimage string if payment received, or null if not.
 */
export function checkLightningPayment(
  paymentHash: string,
  wait: boolean
): Promise<string | null> {
  return NitroArkHybridObject.checkLightningPayment(paymentHash, wait);
}

/**
 * Attempts to claim a Lightning payment, optionally using a claim token.
 * @param paymentHash The payment hash of the Lightning payment.
 * @param wait Whether to wait for the claim to complete.
 * @param token Optional claim token used when no spendable VTXOs are owned.
 * @returns A promise resolving to the claimed LightningReceive if successful, or null if not.
 */
export function tryClaimLightningReceive(
  paymentHash: string,
  wait: boolean,
  token?: string
): Promise<LightningReceive> {
  return NitroArkHybridObject.tryClaimLightningReceive(
    paymentHash,
    wait,
    token
  );
}

/**
 * Checks and claims all open Lightning receives.
 * @param wait Whether to wait for the claim to complete.
 * @returns A promise that resolves on success or rejects on error.
 */
export function tryClaimAllLightningReceives(wait: boolean): Promise<void> {
  return NitroArkHybridObject.tryClaimAllLightningReceives(wait);
}

/**
 * Pays a Bolt11 Lightning invoice.
 * @param destination The Lightning invoice.
 * @param amountSat The amount in satoshis to send. Use 0 for invoice amount.
 * @returns A promise resolving to a LightningSendResult object
 */
export function payLightningInvoice(
  destination: string,
  amountSat?: number
): Promise<LightningSendResult> {
  return NitroArkHybridObject.payLightningInvoice(destination, amountSat);
}

/**
 * Sends a payment to a Bolt12 offer.
 * @param offer The Bolt12 offer.
 * @param amountSat The amount in satoshis to send. Use 0 for invoice amount.
 * @returns A promise resolving to a LightningSendResult object
 */
export function payLightningOffer(
  offer: string,
  amountSat?: number
): Promise<LightningSendResult> {
  return NitroArkHybridObject.payLightningOffer(offer, amountSat);
}

/**
 * Sends a payment to a Lightning Address.
 * @param addr The Lightning Address.
 * @param amountSat The amount in satoshis to send.
 * @param comment An optional comment.
 * @returns A promise resolving to a LightningSendResult object
 */
export function payLightningAddress(
  addr: string,
  amountSat: number,
  comment: string
): Promise<LightningSendResult> {
  return NitroArkHybridObject.payLightningAddress(addr, amountSat, comment);
}

/**
 * Estimates the fee for a Lightning send.
 * @param amountSat The amount in satoshis to send.
 * @returns A promise resolving to the fee estimate.
 */
export function estimateLightningSendFee(
  amountSat: number
): Promise<BarkFeeEstimate> {
  return NitroArkHybridObject.estimateLightningSendFee(amountSat);
}

// --- Ark Operations ---

/**
 * Boards a specific amount from the onchain wallet into Ark.
 * @param amountSat The amount in satoshis to board.
 * @returns A promise resolving to a BoardResult object
 */
export function boardAmount(amountSat: number): Promise<BoardResult> {
  return NitroArkHybridObject.boardAmount(amountSat);
}

/**
 * Boards all available funds from the onchain wallet into Ark.
 * @returns A promise resolving to a BoardResult object.
 */
export function boardAll(): Promise<BoardResult> {
  return NitroArkHybridObject.boardAll();
}

/**
 * Validates an Arkoor address.
 * @param address The Arkoor address to validate.
 * @returns A promise resolving to void.
 */
export function validateArkoorAddress(address: string): Promise<void> {
  return NitroArkHybridObject.validateArkoorAddress(address);
}

/**
 * Sends an Arkoor payment.
 * @param destination The destination Arkoor address.
 * @param amountSat The amount in satoshis to send.
 * @returns A promise resolving to the ArkoorPaymentResult object
 */
export function sendArkoorPayment(
  destination: string,
  amountSat: number
): Promise<ArkoorPaymentResult> {
  return NitroArkHybridObject.sendArkoorPayment(destination, amountSat);
}

/**
 * Estimates the fee for an Arkoor payment.
 * @param amountSat The amount in satoshis to send.
 * @returns A promise resolving to the fee estimate.
 */
export function estimateArkoorPaymentFee(
  amountSat: number
): Promise<BarkFeeEstimate> {
  return NitroArkHybridObject.estimateArkoorPaymentFee(amountSat);
}

/**
 * Sends an onchain payment via an Ark round.
 * @param destination The destination Bitcoin address.
 * @param amountSat The amount in satoshis to send.
 * @returns A promise resolving to txid string.
 */
export function sendOnchain(
  destination: string,
  amountSat: number
): Promise<string> {
  return NitroArkHybridObject.sendOnchain(destination, amountSat);
}

/**
 * Estimates the fee for sending an onchain payment via an Ark round.
 * @param destination The destination Bitcoin address.
 * @param amountSat The amount in satoshis to send.
 * @returns A promise resolving to the fee estimate.
 */
export function estimateSendOnchain(
  destination: string,
  amountSat: number
): Promise<BarkFeeEstimate> {
  return NitroArkHybridObject.estimateSendOnchain(destination, amountSat);
}

// --- Offboarding / Exiting ---

/**
 * Offboards specific VTXOs to a destination address.
 * @param vtxoIds Array of VtxoId strings to offboard.
 * @param destinationAddress Destination Bitcoin address (if empty, sends to internal wallet).
 * @returns A promise resolving to the txid string.
 */
export function offboardSpecific(
  vtxoIds: string[],
  destinationAddress: string
): Promise<string> {
  return NitroArkHybridObject.offboardSpecific(vtxoIds, destinationAddress);
}

/**
 * Offboards all VTXOs to a destination address.
 * @param destinationAddress Destination Bitcoin address (if empty, sends to internal wallet).
 * @returns A promise resolving to the txid string.
 */
export function offboardAll(destinationAddress: string): Promise<string> {
  return NitroArkHybridObject.offboardAll(destinationAddress);
}

/**
 * Estimates the fee for offboarding all Ark funds to an onchain address.
 * @param destinationAddress The destination Bitcoin address.
 * @returns A promise resolving to the fee estimate.
 */
export function estimateOffboardAll(
  destinationAddress: string
): Promise<BarkFeeEstimate> {
  return NitroArkHybridObject.estimateOffboardAll(destinationAddress);
}

// --- Re-export types and enums ---
export type {
  NitroArk,
  BarkCreateOpts,
  BarkConfigOpts,
  BarkArkInfo,
  Bolt11Invoice,
  BoardResult,
  BarkSendManyOutput,
  ArkoorPaymentResult,
  BarkFeeEstimate,
  LightningSendResult,
  OnchainPaymentResult,
  OffchainBalanceResult,
  OnchainBalanceResult,
  NewAddressResult,
  KeyPairResult,
  LightningReceive,
} from './NitroArk.nitro';
