import { useState } from 'react';
import { ScrollView, StyleSheet } from 'react-native';
import * as NitroArk from 'react-native-nitro-ark';
import type { ExitVtxoResult } from 'react-native-nitro-ark';

import {
  ButtonGrid,
  CustomButton,
  InputField,
  ResultBox,
  Section,
} from '../components';
import { COLORS } from '../constants';
import type { TabProps } from '../types';

const parseOptionalFeeRate = (value: string): number | undefined => {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }

  const parsed = parseInt(trimmed, 10);
  if (isNaN(parsed) || parsed <= 0) {
    throw new Error('Fee rate must be a positive number');
  }

  return parsed;
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
  const [drainFeeRate, setDrainFeeRate] = useState('');
  const [drainDestinationAddress, setDrainDestinationAddress] = useState('');
  const [drainVtxoIdsInput, setDrainVtxoIdsInput] = useState('');

  const exitOpsDisabled = isLoading || !isWalletLoaded;

  const setSectionError = (section: string, message: string) => {
    setError((prev) => ({ ...prev, [section]: message }));
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

  const handleSyncExits = () => {
    runOperation(
      'syncExits',
      () => NitroArk.syncExits(),
      'exitLifecycle',
      () =>
        setResults((prev) => ({
          ...prev,
          exitLifecycle: 'Exit synchronization completed.',
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
            : JSON.stringify(progress, null, 2);
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
          exitStatus: JSON.stringify(exitVtxos, null, 2),
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
      <Section title="Exit Lifecycle">
        <ButtonGrid>
          <CustomButton
            title="Start Entire Wallet Exit"
            onPress={handleStartExitForEntireWallet}
            disabled={exitOpsDisabled}
            color={COLORS.warning}
          />
          <CustomButton
            title="Sync Exit"
            onPress={handleSyncExit}
            disabled={exitOpsDisabled}
          />
          <CustomButton
            title="Sync Exits"
            onPress={handleSyncExits}
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
        <ButtonGrid>
          <CustomButton
            title="Get Exit VTXOs"
            onPress={handleGetExitVtxos}
            disabled={exitOpsDisabled}
            color={COLORS.secondary}
          />
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
});
