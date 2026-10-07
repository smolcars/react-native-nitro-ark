jest.mock('react-native-nitro-modules', () => {
  const cancelExit = jest.fn(() => Promise.resolve());
  const estimateEmergencyExitFee = jest.fn(() =>
    Promise.resolve({
      exit_broadcast_fee_sat: 1200,
      claim_fee_sat: 300,
      total_fee_sat: 1500,
      fee_rate_sat_per_vb: 2,
      txs_to_broadcast: 4,
    })
  );
  const updateHistoryMetadata = jest.fn(() => Promise.resolve());
  const payLightningInvoiceWithOrigin = jest.fn(() =>
    Promise.resolve({
      state: 'in_progress',
      invoice: 'lntbs1example',
      payment_hash: 'payment-hash',
      amount: 1000,
      htlc_vtxos: [],
      movement_id: 42,
    })
  );

  return {
    NitroModules: {
      createHybridObject: () => ({
        cancelExit,
        estimateEmergencyExitFee,
        updateHistoryMetadata,
        payLightningInvoiceWithOrigin,
      }),
    },
    mockCancelExit: cancelExit,
    mockEstimateEmergencyExitFee: estimateEmergencyExitFee,
    mockUpdateHistoryMetadata: updateHistoryMetadata,
    mockPayLightningInvoiceWithOrigin: payLightningInvoiceWithOrigin,
  };
});

const {
  mockCancelExit,
  mockEstimateEmergencyExitFee,
  mockUpdateHistoryMetadata,
  mockPayLightningInvoiceWithOrigin,
} = jest.requireMock('react-native-nitro-modules') as {
  mockCancelExit: jest.MockedFunction<(vtxoId: string) => Promise<void>>;
  mockEstimateEmergencyExitFee: jest.Mock;
  mockUpdateHistoryMetadata: jest.MockedFunction<
    (movementId: number, patchJson: string) => Promise<void>
  >;
  mockPayLightningInvoiceWithOrigin: jest.Mock;
};

import {
  NitroArkHybridObject,
  cancelExit,
  estimateEmergencyExitFee,
  payLightningInvoiceWithOrigin,
  updateHistoryMetadata,
} from '../index';

describe('cancelExit', () => {
  beforeEach(() => {
    mockCancelExit.mockClear();
  });

  it('delegates the VTXO ID to the native bridge', async () => {
    await cancelExit('vtxo-id');

    expect(mockCancelExit).toHaveBeenCalledWith('vtxo-id');
  });
});

describe('estimateEmergencyExitFee', () => {
  beforeEach(() => {
    mockEstimateEmergencyExitFee.mockClear();
  });

  it('forwards the VTXOs and optional pricing inputs to the native bridge', async () => {
    const result = await estimateEmergencyExitFee(
      ['vtxo-1', 'vtxo-2'],
      2000,
      'bcrt1pdestination'
    );

    expect(mockEstimateEmergencyExitFee).toHaveBeenCalledWith(
      ['vtxo-1', 'vtxo-2'],
      2000,
      'bcrt1pdestination'
    );
    expect(result).toEqual({
      exit_broadcast_fee_sat: 1200,
      claim_fee_sat: 300,
      total_fee_sat: 1500,
      fee_rate_sat_per_vb: 2,
      txs_to_broadcast: 4,
    });
  });
});

describe('updateHistoryMetadata', () => {
  beforeEach(() => {
    mockUpdateHistoryMetadata.mockClear();
  });

  it('delegates valid movement IDs and patches to the native bridge', async () => {
    await updateHistoryMetadata(42, '{"noah":{"lnurl_pay":{"comment":"Hi"}}}');

    expect(mockUpdateHistoryMetadata).toHaveBeenCalledWith(
      42,
      '{"noah":{"lnurl_pay":{"comment":"Hi"}}}'
    );
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY, -1, 1.5, 0x100000000])(
    'rejects invalid movement ID %p',
    (movementId) => {
      expect(() => updateHistoryMetadata(movementId, '{}')).toThrow(
        'movementId must be a finite unsigned 32-bit integer'
      );
      expect(mockUpdateHistoryMetadata).not.toHaveBeenCalled();
    }
  );
});

describe('payLightningInvoiceWithOrigin', () => {
  beforeEach(() => {
    mockPayLightningInvoiceWithOrigin.mockClear();
  });

  it.each([
    { method: 'lightning-address' as const, value: 'alice@example.com' },
    { method: 'lnurl' as const, value: 'lnurl1example' },
    {
      method: 'custom' as const,
      value: 'https://example.com/lnurlp/alice',
    },
  ])('forwards the $method origin to the native bridge', async (origin) => {
    const result = await payLightningInvoiceWithOrigin(
      'lntbs1example',
      origin,
      true
    );

    expect(mockPayLightningInvoiceWithOrigin).toHaveBeenCalledWith(
      'lntbs1example',
      origin,
      true
    );
    expect(result).toEqual({
      state: 'in_progress',
      invoice: 'lntbs1example',
      payment_hash: 'payment-hash',
      amount: 1000,
      htlc_vtxos: [],
      movement_id: 42,
    });
  });

  it('exposes only supported origin methods on the raw hybrid', () => {
    const invalidOrigin: Parameters<
      typeof NitroArkHybridObject.payLightningInvoiceWithOrigin
    >[1] = {
      // @ts-expect-error -- invoices are payment destinations, not app-resolved origins.
      method: 'invoice',
      value: 'lntbs1example',
    };

    expect(invalidOrigin.value).toBe('lntbs1example');
  });
});
