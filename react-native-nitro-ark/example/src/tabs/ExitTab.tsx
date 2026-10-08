import { useState } from 'react';
import { ScrollView, StyleSheet, Text } from 'react-native';
import * as NitroArk from 'react-native-nitro-ark';
import type { ExitVtxoResult } from 'react-native-nitro-ark';

import {
  ButtonGrid,
  CustomButton,
  InputField,
  ResultBox,
  Section,
} from '../components';
import { COLORS, formatSats } from '../constants';
import type { TabProps } from '../types';

const parseOptionalFeeRate = (value: string): number | undefined => {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }

  const parsed = Number(trimmed);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error('Fee rate must be a positive safe integer in sat/kvB');
  }

  return parsed;
};

const formatExitResults = (
  exits: Pick<ExitVtxoResult, 'vtxo_id' | 'state_details'>[]
): string => {
  const swept = exits
    .filter((exit) => exit.state_details.kind === 'vtxo-swept')
    .map(
      (exit) =>
        `Exit ${exit.vtxo_id}: swept (terminal). Required exit-chain inputs were spent onchain.\n` +
        `Spent inputs:\n${exit.state_details.spent_inputs?.join('\n') || 'None reported.'}\n` +
        'A delegated refresh may still be possible; success is not guaranteed.'
    );
  return [...swept, JSON.stringify(exits, null, 2)].join('\n\n');
};

export const ExitTab = ({
  results,
  setResults,
  error,
  setError,
  isLoading,
  isWalletLoaded,
  runOperation,
}: TabProps) => {
  const [progressFeeRate, setProgressFeeRate] = useState('');
  const [cancelVtxoId, setCancelVtxoId] = useState('');
  const [statusVtxoId, setStatusVtxoId] = useState('');
  const [drainFeeRate, setDrainFeeRate] = useState('');
  const [drainDestinationAddress, setDrainDestinationAddress] = useState('');
  const [drainVtxoIdsInput, setDrainVtxoIdsInput] = useState('');
  const [estimateVtxoIdsInput, setEstimateVtxoIdsInput] = useState('');
  const [estimateFeeRate, setEstimateFeeRate] = useState('');
  const [estimateDestinationAddress, setEstimateDestinationAddress] =
    useState('');
  const [estimateFeeMargin, setEstimateFeeMargin] = useState('');

  const exitOpsDisabled = isLoading || !isWalletLoaded;

  const setSectionError = (section: string, message: string) => {
    setError((prev) => ({ ...prev, [section]: message }));
  };

  const handleEstimateExitFee = () => {
    const vtxoIds = estimateVtxoIdsInput
      .split(',')
      .map((id) => id.trim())
      .filter(Boolean);
    let feeRateSatPerKvb: number | undefined;
    try {
      feeRateSatPerKvb = parseOptionalFeeRate(estimateFeeRate);
    } catch (err: any) {
      setSectionError('exitEstimate', err.message);
      return;
    }
    const feeMargin =
      estimateFeeMargin.trim() === '' ? undefined : Number(estimateFeeMargin);

    runOperation(
      'estimateEmergencyExitFee',
      () =>
        NitroArk.estimateEmergencyExitFee(
          vtxoIds,
          feeRateSatPerKvb,
          estimateDestinationAddress.trim() || undefined,
          feeMargin
        ),
      'exitEstimate',
      (estimate) => {
        setResults((prev) => ({
          ...prev,
          exitEstimate: [
            `Upfront broadcast funding: ${formatSats(estimate.exit_broadcast_fee_sat)}`,
            `Later claim fee (deducted from recovered funds): ${formatSats(estimate.claim_fee_sat)}`,
            `Total cost: ${formatSats(estimate.total_fee_sat)}`,
            `Base broadcast fee rate (before margin): ${estimate.fee_rate_sat_per_vb} sat/vB`,
            `Transactions requiring broadcast/CPFP: ${estimate.txs_to_broadcast}`,
          ].join('\n'),
        }));
      }
    );
  };

  const handleStartExitForEntireWallet = () => {
    runOperation(
      'startExitForEntireWallet',
      () => NitroArk.startExitForEntireWallet(),
      'exitLifecycle',
      () =>
        setResults((prev) => ({
          ...prev,
          exitLifecycle: 'Started unilateral exit for the entire wallet.',
        }))
    );
  };

  const handleSyncExit = () => {
    runOperation(
      'syncExit',
      () => NitroArk.syncExit(),
      'exitLifecycle',
      () =>
        setResults((prev) => ({
          ...prev,
          exitLifecycle: 'Exit coordinator sync completed.',
        }))
    );
  };

  const handleCancelExit = () => {
    const vtxoId = cancelVtxoId.trim();
    if (!vtxoId) {
      setSectionError('exitLifecycle', 'A VTXO ID is required');
      return;
    }

    runOperation(
      'cancelExit',
      () => NitroArk.cancelExit(vtxoId),
      'exitLifecycle',
      () =>
        setResults((prev) => ({
          ...prev,
          exitLifecycle: `Canceled unilateral exit for ${vtxoId}.`,
        }))
    );
  };

  const handleProgressExits = () => {
    let feeRateSatPerKvb: number | undefined;
    try {
      feeRateSatPerKvb = parseOptionalFeeRate(progressFeeRate);
    } catch (err: any) {
      setSectionError('exitProgress', err.message);
      return;
    }

    runOperation(
      'progressExits',
      () => NitroArk.progressExits(feeRateSatPerKvb),
      'exitProgress',
      (progress) => {
        const summary =
          progress.length === 0
            ? 'No tracked exits still require progression.'
            : formatExitResults(progress);
        setResults((prev) => ({
          ...prev,
          exitProgress: summary,
        }));
      }
    );
  };

  const handleGetExitVtxos = () => {
    runOperation(
      'getExitVtxos',
      () => NitroArk.getExitVtxos(),
      'exitStatus',
      (exitVtxos: ExitVtxoResult[]) => {
        const claimableIds = exitVtxos
          .filter((exitVtxo) => exitVtxo.is_claimable)
          .map((exitVtxo) => exitVtxo.vtxo_id);

        if (claimableIds.length > 0) {
          setDrainVtxoIdsInput(claimableIds.join(', '));
        }

        setResults((prev) => ({
          ...prev,
          exitStatus: formatExitResults(exitVtxos),
        }));
      }
    );
  };

  const handleGetExitStatus = () => {
    const vtxoId = statusVtxoId.trim();
    if (!vtxoId) {
      setSectionError('exitStatus', 'A VTXO ID is required');
      return;
    }

    runOperation(
      'getExitStatus',
      () => NitroArk.getExitStatus(vtxoId, true, false),
      'exitStatus',
      (status) => {
        setResults((prev) => ({
          ...prev,
          exitStatus: status
            ? formatExitResults([status])
            : `No exit status found for ${vtxoId}.`,
        }));
      }
    );
  };

  const handleHasPendingExits = () => {
    runOperation(
      'hasPendingExits',
      () => NitroArk.hasPendingExits(),
      'exitStatus',
      (hasPendingExits) => {
        setResults((prev) => ({
          ...prev,
          exitStatus: hasPendingExits
            ? 'There are exits still pending confirmation or progression.'
            : 'No pending exits remain.',
        }));
      }
    );
  };

  const handlePendingExitTotal = () => {
    runOperation(
      'pendingExitTotal',
      () => NitroArk.pendingExitTotal(),
      'exitStatus',
      (pendingTotal) => {
        setResults((prev) => ({
          ...prev,
          exitStatus: `${pendingTotal.toLocaleString()} sats are still pending exit confirmation.`,
        }));
      }
    );
  };

  const handleAllClaimableAtHeight = () => {
    runOperation(
      'allClaimableAtHeight',
      () => NitroArk.allClaimableAtHeight(),
      'exitStatus',
      (blockHeight) => {
        setResults((prev) => ({
          ...prev,
          exitStatus:
            blockHeight === undefined
              ? 'Claimable height is not known yet for all tracked exits.'
              : `All tracked exits are claimable by block height ${blockHeight}.`,
        }));
      }
    );
  };

  const handleDrainExits = () => {
    if (!drainDestinationAddress.trim() || !drainVtxoIdsInput.trim()) {
      setSectionError(
        'exitDrain',
        'Destination address and at least one exit VTXO ID are required'
      );
      return;
    }

    const vtxoIds = drainVtxoIdsInput
      .split(',')
      .map((id) => id.trim())
      .filter(Boolean);

    if (vtxoIds.length === 0) {
      setSectionError('exitDrain', 'At least one exit VTXO ID is required');
      return;
    }

    let feeRateSatPerKvb: number | undefined;
    try {
      feeRateSatPerKvb = parseOptionalFeeRate(drainFeeRate);
    } catch (err: any) {
      setSectionError('exitDrain', err.message);
      return;
    }

    runOperation(
      'drainExits',
      () =>
        NitroArk.drainExits(
          vtxoIds,
          drainDestinationAddress.trim(),
          feeRateSatPerKvb
        ),
      'exitDrain',
      (psbt) => {
        setResults((prev) => ({
          ...prev,
          exitDrain: `Drain PSBT (base64):\n\n${psbt}`,
        }));
      }
    );
  };

  return (
    <ScrollView style={styles.container} showsVerticalScrollIndicator={false}>
      <Section title="Exit Fee Estimate">
        <InputField
          label="VTXO IDs (empty = none)"
          value={estimateVtxoIdsInput}
          onChangeText={setEstimateVtxoIdsInput}
          placeholder="Comma-separated VTXO IDs"
          multiline
        />
        <InputField
          label="Fee Rate Override (sat/kvB)"
          value={estimateFeeRate}
          onChangeText={setEstimateFeeRate}
          placeholder="Optional; e.g., 1500 = 1.5 sat/vB"
          keyboardType="numeric"
        />
        <InputField
          label="Claim Destination Address"
          value={estimateDestinationAddress}
          onChangeText={setEstimateDestinationAddress}
          placeholder="Optional; defaults to a P2TR output"
        />
        <InputField
          label="Broadcast Fee Multiplier"
          value={estimateFeeMargin}
          onChangeText={setEstimateFeeMargin}
          placeholder="Default 1.2 (20% margin)"
          keyboardType="numeric"
        />
        <Text style={styles.estimateHelp}>
          Estimates selected VTXOs only; an empty selection costs zero. Sync
          Exit first for fresh chain state. The multiplier applies to broadcast
          funding: 1 adds no margin, and 0 produces a zero broadcast estimate.
          The claim fee is deducted later from recovered funds. Estimating works
          before funding the onchain wallet.
        </Text>
        <ButtonGrid>
          <CustomButton
            title="Estimate Exit Fee"
            onPress={handleEstimateExitFee}
            disabled={exitOpsDisabled}
            color={COLORS.primary}
          />
        </ButtonGrid>
        <ResultBox result={results.exitEstimate} error={error.exitEstimate} />
      </Section>

      <Section title="Exit Lifecycle">
        <InputField
          label="VTXO ID to Cancel"
          value={cancelVtxoId}
          onChangeText={setCancelVtxoId}
          placeholder="Enter an in-flight exit VTXO ID"
        />
        <ButtonGrid>
          <CustomButton
            title="Start Entire Wallet Exit"
            onPress={handleStartExitForEntireWallet}
            disabled={exitOpsDisabled}
            color={COLORS.warning}
          />
          <CustomButton
            title="Cancel Exit"
            onPress={handleCancelExit}
            disabled={exitOpsDisabled}
            color={COLORS.warning}
          />
          <CustomButton
            title="Sync Exit"
            onPress={handleSyncExit}
            disabled={exitOpsDisabled}
          />
        </ButtonGrid>
        <ResultBox result={results.exitLifecycle} error={error.exitLifecycle} />
      </Section>

      <Section title="Progress Exits">
        <InputField
          label="Fee Rate Override (sat/kvB)"
          value={progressFeeRate}
          onChangeText={setProgressFeeRate}
          placeholder="Optional"
          keyboardType="numeric"
        />
        <ButtonGrid>
          <CustomButton
            title="Progress Exits"
            onPress={handleProgressExits}
            disabled={exitOpsDisabled}
            color={COLORS.primary}
          />
        </ButtonGrid>
        <ResultBox result={results.exitProgress} error={error.exitProgress} />
      </Section>

      <Section title="Exit Overview">
        <InputField
          label="Exit VTXO ID to Inspect"
          value={statusVtxoId}
          onChangeText={setStatusVtxoId}
          placeholder="Enter a live or finished exit VTXO ID"
        />
        <ButtonGrid>
          <CustomButton
            title="Get Exit Status"
            onPress={handleGetExitStatus}
            disabled={exitOpsDisabled}
            color={COLORS.secondary}
          />
          <CustomButton
            title="Get Exit VTXOs"
            onPress={handleGetExitVtxos}
            disabled={exitOpsDisabled}
            color={COLORS.secondary}
          />
        </ButtonGrid>
        <ButtonGrid>
          <CustomButton
            title="Has Pending Exits"
            onPress={handleHasPendingExits}
            disabled={exitOpsDisabled}
          />
          <CustomButton
            title="Pending Exit Total"
            onPress={handlePendingExitTotal}
            disabled={exitOpsDisabled}
          />
        </ButtonGrid>
        <ButtonGrid>
          <CustomButton
            title="All Claimable Height"
            onPress={handleAllClaimableAtHeight}
            disabled={exitOpsDisabled}
          />
        </ButtonGrid>
        <ResultBox result={results.exitStatus} error={error.exitStatus} />
      </Section>

      <Section title="Drain Claimable Exits">
        <InputField
          label="Destination Address"
          value={drainDestinationAddress}
          onChangeText={setDrainDestinationAddress}
          placeholder="bc1q... or tb1q..."
        />
        <InputField
          label="Exit VTXO IDs"
          value={drainVtxoIdsInput}
          onChangeText={setDrainVtxoIdsInput}
          placeholder="Comma-separated exit VTXO IDs"
          multiline
        />
        <InputField
          label="Fee Rate Override (sat/kvB)"
          value={drainFeeRate}
          onChangeText={setDrainFeeRate}
          placeholder="Optional"
          keyboardType="numeric"
        />
        <ButtonGrid>
          <CustomButton
            title="Drain Exits"
            onPress={handleDrainExits}
            disabled={exitOpsDisabled}
            color={COLORS.success}
          />
        </ButtonGrid>
        <ResultBox result={results.exitDrain} error={error.exitDrain} />
      </Section>
    </ScrollView>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: COLORS.background,
  },
  estimateHelp: {
    color: COLORS.textMuted,
    marginBottom: 12,
  },
});
