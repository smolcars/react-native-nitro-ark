import { useEffect, useRef, useState } from 'react';
import { View, ScrollView, StyleSheet, Text } from 'react-native';
import * as NitroArk from 'react-native-nitro-ark';

import {
  CustomButton,
  InputField,
  ResultBox,
  Section,
  ButtonGrid,
} from '../components';
import { COLORS } from '../constants';
import type { TabProps } from '../types';

export const ReceiveTab = ({
  mnemonic,
  results,
  setResults,
  error,
  setError,
  isLoading,
  runOperation,
}: TabProps) => {
  const [invoiceAmount, setInvoiceAmount] = useState('1000');
  const [paymentHash, setPaymentHash] = useState('');
  const [claimToken, setClaimToken] = useState('');
  const [lastArkAddress, setLastArkAddress] = useState<string>('');
  const [lastInvoice, setLastInvoice] = useState<{
    invoice: string;
    paymentHash: string;
  } | null>(null);
  const [arkSubscriptionStatus, setArkSubscriptionStatus] =
    useState('Inactive');
  const [arkSubscriptionLog, setArkSubscriptionLog] = useState<string>('');
  const [lightningSubscriptionStatus, setLightningSubscriptionStatus] =
    useState('Inactive');
  const [lightningSubscriptionLog, setLightningSubscriptionLog] =
    useState<string>('');

  const arkSubscriptionRef = useRef<ReturnType<
    typeof NitroArk.subscribeArkoorAddressMovements
  > | null>(null);
  const lightningSubscriptionRef = useRef<ReturnType<
    typeof NitroArk.subscribeLightningPaymentMovements
  > | null>(null);

  const canUseWallet = !!mnemonic;
  const walletOpsDisabled = isLoading || !canUseWallet;

  const appendSubscriptionLog = (
    updateLog: React.Dispatch<React.SetStateAction<string>>,
    line: string
  ) => {
    updateLog((prev) => (prev ? `${line}\n${prev}` : line));
  };

  const formatSubscriptionEvent = (
    event: Parameters<typeof NitroArk.subscribeNotifications>[0] extends (
      arg: infer T
    ) => void
      ? T
      : never
  ) => {
    const movement = event.movement;
    const summary = movement
      ? `status=${movement.status}, id=${movement.id}, subsystem=${movement.subsystem.kind}, amount=${movement.effective_balance_sat}`
      : 'no movement payload';
    return `${new Date().toLocaleTimeString()} ${event.kind} ${summary}`;
  };

  const stopArkSubscription = () => {
    arkSubscriptionRef.current?.stop();
    arkSubscriptionRef.current = null;
    setArkSubscriptionStatus('Inactive');
  };

  const stopLightningSubscription = () => {
    lightningSubscriptionRef.current?.stop();
    lightningSubscriptionRef.current = null;
    setLightningSubscriptionStatus('Inactive');
  };

  const startArkSubscription = (address: string) => {
    if (!address) {
      setError((prev) => ({
        ...prev,
        arkSubscription: 'Generate or enter an Ark address first',
      }));
      return;
    }

    try {
      stopArkSubscription();
      setError((prev) => ({ ...prev, arkSubscription: '' }));
      setArkSubscriptionLog('');
      setArkSubscriptionStatus('Listening for address movements...');
      arkSubscriptionRef.current = NitroArk.subscribeArkoorAddressMovements(
        address,
        (event) => {
          const line = formatSubscriptionEvent(event);
          appendSubscriptionLog(setArkSubscriptionLog, line);
          setArkSubscriptionStatus(
            `Last event: ${event.kind}${event.movement ? ` (${event.movement.status})` : ''}`
          );
          setResults((prev) => ({
            ...prev,
            arkSubscription: line,
          }));
        }
      );
    } catch (err: any) {
      setArkSubscriptionStatus('Failed to start');
      setError((prev) => ({
        ...prev,
        arkSubscription: err?.message || 'Failed to start Ark subscription',
      }));
    }
  };

  const startLightningSubscription = (nextPaymentHash: string) => {
    if (!nextPaymentHash) {
      setError((prev) => ({
        ...prev,
        lightningSubscription:
          'Create an invoice or provide a payment hash first',
      }));
      return;
    }

    try {
      stopLightningSubscription();
      setError((prev) => ({ ...prev, lightningSubscription: '' }));
      setLightningSubscriptionLog('');
      setLightningSubscriptionStatus('Listening for invoice movements...');
      lightningSubscriptionRef.current =
        NitroArk.subscribeLightningPaymentMovements(
          nextPaymentHash,
          (event) => {
            const line = formatSubscriptionEvent(event);
            appendSubscriptionLog(setLightningSubscriptionLog, line);
            setLightningSubscriptionStatus(
              `Last event: ${event.kind}${event.movement ? ` (${event.movement.status})` : ''}`
            );
            setResults((prev) => ({
              ...prev,
              lightningSubscription: line,
            }));
          }
        );
    } catch (err: any) {
      setLightningSubscriptionStatus('Failed to start');
      setError((prev) => ({
        ...prev,
        lightningSubscription:
          err?.message || 'Failed to start Lightning subscription',
      }));
    }
  };

  useEffect(() => {
    return () => {
      stopArkSubscription();
      stopLightningSubscription();
    };
  }, []);

  // --- Create Invoice ---
  const handleCreateInvoice = () => {
    const amount = parseInt(invoiceAmount, 10);
    if (isNaN(amount) || amount <= 0) {
      setError((prev) => ({ ...prev, invoice: 'Valid amount required' }));
      return;
    }
    runOperation(
      'bolt11Invoice',
      () => NitroArk.bolt11Invoice(amount),
      'invoice',
      (invoice) => {
        setLastInvoice({
          invoice: invoice.payment_request,
          paymentHash: invoice.payment_hash,
        });
        setPaymentHash(invoice.payment_hash);
        startLightningSubscription(invoice.payment_hash);
        setResults((prev) => ({
          ...prev,
          invoice: `Invoice created and subscription armed.\n\nPayment Request:\n${invoice.payment_request}\n\nPayment Hash:\n${invoice.payment_hash}`,
        }));
      }
    );
  };

  // --- Lightning Receive Status ---
  const handleLightningReceiveStatus = () => {
    if (!paymentHash) {
      setError((prev) => ({ ...prev, status: 'Payment hash required' }));
      return;
    }
    runOperation(
      'lightningReceiveStatus',
      () => NitroArk.lightningReceiveStatus(paymentHash),
      'status'
    );
  };

  // --- Claim Lightning Receive ---
  const handleTryClaimLightningReceive = () => {
    if (!paymentHash) {
      setError((prev) => ({ ...prev, claim: 'Payment hash required' }));
      return;
    }
    const token = claimToken.trim() || undefined;
    runOperation(
      'tryClaimLightningReceive',
      () => NitroArk.tryClaimLightningReceive(paymentHash, false, token),
      'claim',
      () =>
        setResults((prev) => ({
          ...prev,
          claim: 'Successfully claimed payment!',
        }))
    );
  };

  const handleTryClaimLightningReceiveWait = () => {
    if (!paymentHash) {
      setError((prev) => ({ ...prev, claim: 'Payment hash required' }));
      return;
    }
    const token = claimToken.trim() || undefined;
    runOperation(
      'tryClaimLightningReceive (wait)',
      () => NitroArk.tryClaimLightningReceive(paymentHash, true, token),
      'claim',
      () =>
        setResults((prev) => ({
          ...prev,
          claim: 'Successfully claimed payment!',
        }))
    );
  };

  const handleTryClaimAllLightningReceives = () => {
    runOperation(
      'tryClaimAllLightningReceives',
      () => NitroArk.tryClaimAllLightningReceives(false),
      'claim',
      () =>
        setResults((prev) => ({
          ...prev,
          claim: 'Successfully claimed all pending receives!',
        }))
    );
  };

  const handleTryClaimAllLightningReceivesWait = () => {
    runOperation(
      'tryClaimAllLightningReceives (wait)',
      () => NitroArk.tryClaimAllLightningReceives(true),
      'claim',
      () =>
        setResults((prev) => ({
          ...prev,
          claim: 'Successfully claimed all pending receives!',
        }))
    );
  };

  // --- Address Generation ---
  const handleNewAddress = () => {
    runOperation(
      'newAddress',
      () => NitroArk.newAddress(),
      'address',
      (address) => {
        setLastArkAddress(address.address);
        startArkSubscription(address.address);
        setResults((prev) => ({
          ...prev,
          address: `Address created and subscription armed.\n\nArk Address:\n${address.address}\n\nArk ID:\n${address.ark_id}\n\nUser Pubkey:\n${address.user_pubkey}`,
        }));
      }
    );
  };

  const handlePeekAddress = () => {
    runOperation(
      'peekAddress',
      () => NitroArk.peekAddress(0),
      'address',
      (address) => {
        setLastArkAddress(address.address);
        setResults((prev) => ({
          ...prev,
          address: `Preview address ready.\n\nArk Address:\n${address.address}\n\nArk ID:\n${address.ark_id}\n\nUser Pubkey:\n${address.user_pubkey}`,
        }));
      }
    );
  };

  const handleGetOnchainAddress = () => {
    runOperation('onchainAddress', () => NitroArk.onchainAddress(), 'address');
  };

  return (
    <ScrollView style={styles.container} showsVerticalScrollIndicator={false}>
      {/* Addresses */}
      <Section title="Receive Addresses">
        <ButtonGrid>
          <CustomButton
            title="New Ark Address"
            onPress={handleNewAddress}
            disabled={walletOpsDisabled}
            color={COLORS.success}
          />
          <CustomButton
            title="Peak Address"
            onPress={handlePeekAddress}
            disabled={walletOpsDisabled}
          />
        </ButtonGrid>
        <ButtonGrid>
          <CustomButton
            title="Onchain Address"
            onPress={handleGetOnchainAddress}
            disabled={walletOpsDisabled}
          />
        </ButtonGrid>
        <ResultBox result={results.address} error={error.address} />
        <View style={styles.subscriptionBox}>
          <Text style={styles.subscriptionLabel}>Tracked Ark Address</Text>
          <Text style={styles.subscriptionValue} selectable>
            {lastArkAddress || 'No address selected yet'}
          </Text>
          <Text style={styles.subscriptionStatus}>{arkSubscriptionStatus}</Text>
          <ButtonGrid>
            <CustomButton
              title="Start Address Subscription"
              onPress={() => startArkSubscription(lastArkAddress)}
              disabled={walletOpsDisabled || !lastArkAddress}
              color={COLORS.success}
            />
            <CustomButton
              title="Stop Address Subscription"
              onPress={stopArkSubscription}
              disabled={!arkSubscriptionRef.current}
              color={COLORS.warning}
            />
          </ButtonGrid>
          <ResultBox
            result={arkSubscriptionLog}
            error={error.arkSubscription}
          />
        </View>
      </Section>

      {/* Create Invoice */}
      <Section title="Create Invoice">
        <InputField
          label="Amount (sats)"
          value={invoiceAmount}
          onChangeText={setInvoiceAmount}
          placeholder="e.g., 1000"
          keyboardType="numeric"
        />
        <ButtonGrid>
          <CustomButton
            title="Create Invoice"
            onPress={handleCreateInvoice}
            disabled={walletOpsDisabled}
            color={COLORS.success}
          />
        </ButtonGrid>

        {lastInvoice && (
          <View style={styles.invoiceBox}>
            <Text style={styles.invoiceLabel}>Last Created Invoice:</Text>
            <Text style={styles.invoiceText} selectable>
              {lastInvoice.invoice}
            </Text>
            <Text style={styles.invoiceLabel}>Payment Hash:</Text>
            <Text style={styles.invoiceHashText} selectable>
              {lastInvoice.paymentHash}
            </Text>
          </View>
        )}

        <ResultBox result={results.invoice} error={error.invoice} />
        <View style={styles.subscriptionBox}>
          <Text style={styles.subscriptionLabel}>Tracked Invoice Hash</Text>
          <Text style={styles.subscriptionValue} selectable>
            {paymentHash || 'No invoice selected yet'}
          </Text>
          <Text style={styles.subscriptionStatus}>
            {lightningSubscriptionStatus}
          </Text>
          <ButtonGrid>
            <CustomButton
              title="Start Invoice Subscription"
              onPress={() => startLightningSubscription(paymentHash)}
              disabled={walletOpsDisabled || !paymentHash}
              color={COLORS.success}
            />
            <CustomButton
              title="Stop Invoice Subscription"
              onPress={stopLightningSubscription}
              disabled={!lightningSubscriptionRef.current}
              color={COLORS.warning}
            />
          </ButtonGrid>
          <ResultBox
            result={lightningSubscriptionLog}
            error={error.lightningSubscription}
          />
        </View>
      </Section>

      {/* Check Receive Status */}
      <Section title="Check Receive Status">
        <InputField
          label="Payment Hash"
          value={paymentHash}
          onChangeText={setPaymentHash}
          placeholder="Enter payment hash"
        />
        <ButtonGrid>
          <CustomButton
            title="Get Receive Status"
            onPress={handleLightningReceiveStatus}
            disabled={walletOpsDisabled}
          />
        </ButtonGrid>
        <ResultBox result={results.status} error={error.status} />
      </Section>

      {/* Claim Payments */}
      <Section title="Claim Payments">
        <InputField
          label="Claim Token (optional)"
          value={claimToken}
          onChangeText={setClaimToken}
          placeholder="Optional token for claiming without VTXOs"
        />
        <ButtonGrid>
          <CustomButton
            title="Claim"
            onPress={handleTryClaimLightningReceive}
            disabled={walletOpsDisabled}
            color={COLORS.success}
          />
          <CustomButton
            title="Claim (Wait)"
            onPress={handleTryClaimLightningReceiveWait}
            disabled={walletOpsDisabled}
            color={COLORS.success}
          />
        </ButtonGrid>
        <ButtonGrid>
          <CustomButton
            title="Claim All"
            onPress={handleTryClaimAllLightningReceives}
            disabled={walletOpsDisabled}
            color={COLORS.warning}
          />
          <CustomButton
            title="Claim All (Wait)"
            onPress={handleTryClaimAllLightningReceivesWait}
            disabled={walletOpsDisabled}
            color={COLORS.warning}
          />
        </ButtonGrid>
        <ResultBox result={results.claim} error={error.claim} />
      </Section>

      <View style={styles.bottomPadding} />
    </ScrollView>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: COLORS.background,
    padding: 16,
  },
  invoiceBox: {
    marginTop: 12,
    padding: 12,
    backgroundColor: COLORS.surfaceLight,
    borderRadius: 8,
  },
  invoiceLabel: {
    fontSize: 12,
    color: COLORS.textSecondary,
    marginBottom: 4,
    fontWeight: '600',
  },
  invoiceText: {
    fontSize: 11,
    color: COLORS.text,
    fontFamily: 'monospace',
    marginBottom: 12,
  },
  invoiceHashText: {
    fontSize: 11,
    color: COLORS.primary,
    fontFamily: 'monospace',
  },
  subscriptionBox: {
    marginTop: 12,
    padding: 12,
    backgroundColor: COLORS.surfaceLight,
    borderRadius: 8,
  },
  subscriptionLabel: {
    fontSize: 12,
    color: COLORS.textSecondary,
    marginBottom: 4,
    fontWeight: '600',
  },
  subscriptionValue: {
    fontSize: 11,
    color: COLORS.text,
    fontFamily: 'monospace',
    marginBottom: 8,
  },
  subscriptionStatus: {
    fontSize: 12,
    color: COLORS.success,
    marginBottom: 8,
    fontWeight: '500',
  },
  bottomPadding: {
    height: 40,
  },
});
