jest.mock('react-native-nitro-modules', () => {
  const cancelExit = jest.fn(() => Promise.resolve());
  const getExitStatus = jest.fn();
  const getExitVtxos = jest.fn();
  const progressExits = jest.fn();
  const estimateEmergencyExitFee = jest.fn(() =>
    Promise.resolve({
      exit_broadcast_fee_sat: 1200,
      claim_fee_sat: 300,
      total_fee_sat: 1500,
      fee_rate_sat_per_vb: 1.5,
      txs_to_broadcast: 4,
    })
  );
  const updateHistoryMetadata = jest.fn(() => Promise.resolve());
  const lightningPaymentResult = () =>
    Promise.resolve({
      state: 'in_progress',
      invoice: 'lntbs1example',
      payment_hash: 'payment-hash',
      amount: 1000,
      htlc_vtxos: [],
      movement_id: 42,
    });
  const payLightningInvoice = jest.fn(lightningPaymentResult);
  const payLightningOffer = jest.fn(lightningPaymentResult);
  const payLightningAddress = jest.fn(lightningPaymentResult);
  const payLightningInvoiceWithOrigin = jest.fn(lightningPaymentResult);

  return {
    NitroModules: {
      createHybridObject: () => ({
        cancelExit,
        getExitStatus,
        getExitVtxos,
        progressExits,
        estimateEmergencyExitFee,
        updateHistoryMetadata,
        payLightningInvoice,
        payLightningOffer,
        payLightningAddress,
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
  getExitStatus,
  getExitVtxos,
  progressExits,
  payLightningInvoice,
  payLightningOffer,
  payLightningAddress,
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

describe('swept exit details', () => {
  it('preserves spent inputs in current and historical exit responses', async () => {
    const details = {
      kind: 'vtxo-swept',
      tip_height: 321,
      spent_inputs: [`${'11'.repeat(32)}:0`, `${'22'.repeat(32)}:4294967295`],
    };
    const exit = {
      vtxo_id: 'vtxo-id',
      state: 'VtxoSwept',
      state_details: details,
      history: ['Start', 'VtxoSwept'],
      history_details: [{ kind: 'start', tip_height: 300 }, details],
      transactions: [],
    };
    jest.mocked(NitroArkHybridObject.getExitStatus).mockResolvedValue(exit);
    jest.mocked(NitroArkHybridObject.getExitVtxos).mockResolvedValue([
      {
        ...exit,
        amount_sat: 1000,
        txids: [],
        is_claimable: false,
        is_initialized: false,
      },
    ]);
    jest.mocked(NitroArkHybridObject.progressExits).mockResolvedValue([exit]);

    const status = await getExitStatus('vtxo-id', true, false);
    const [vtxo] = await getExitVtxos();
    const [progress] = await progressExits();
    for (const result of [status, vtxo, progress]) {
      expect(result?.state).toBe('VtxoSwept');
      expect(result?.state_details).toEqual(details);
    }
    for (const result of [status, vtxo]) {
      expect(result?.history_details).toEqual(exit.history_details);
      expect(result?.history_details[0]?.spent_inputs).toBeUndefined();
    }
    expect(NitroArkHybridObject.getExitStatus).toHaveBeenCalledWith(
      'vtxo-id',
      true,
      false
    );
  });
});

describe('estimateEmergencyExitFee', () => {
  beforeEach(() => {
    mockEstimateEmergencyExitFee.mockClear();
  });

  it.each([undefined, 0, 1, 1.2, 1.5])(
    'forwards pricing inputs with fee margin %p and preserves fractional rates',
    async (feeMargin) => {
      const result = await estimateEmergencyExitFee(
        ['vtxo-1', 'vtxo-2'],
        1500,
        'bcrt1pdestination',
        feeMargin
      );

      expect(mockEstimateEmergencyExitFee).toHaveBeenCalledWith(
        ['vtxo-1', 'vtxo-2'],
        1500,
        'bcrt1pdestination',
        feeMargin
      );
      expect(result).toEqual({
        exit_broadcast_fee_sat: 1200,
        claim_fee_sat: 300,
        total_fee_sat: 1500,
        fee_rate_sat_per_vb: 1.5,
        txs_to_broadcast: 4,
      });
    }
  );
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
      true,
      undefined
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

describe.each([
  {
    name: 'payLightningInvoice',
    native: NitroArkHybridObject.payLightningInvoice,
    call: (wait: boolean, retry?: number) =>
      payLightningInvoice('invoice', wait, undefined, retry),
    args: (wait: boolean, retry?: number) => [
      'invoice',
      wait,
      undefined,
      retry,
    ],
  },
  {
    name: 'payLightningOffer',
    native: NitroArkHybridObject.payLightningOffer,
    call: (wait: boolean, retry?: number) =>
      payLightningOffer('offer', wait, 1000, retry),
    args: (wait: boolean, retry?: number) => ['offer', wait, 1000, retry],
  },
  {
    name: 'payLightningAddress',
    native: NitroArkHybridObject.payLightningAddress,
    call: (wait: boolean, retry?: number) =>
      payLightningAddress('alice@example.com', 1000, 'Hi', wait, retry),
    args: (wait: boolean, retry?: number) => [
      'alice@example.com',
      1000,
      'Hi',
      wait,
      retry,
    ],
  },
  {
    name: 'payLightningInvoiceWithOrigin',
    native: NitroArkHybridObject.payLightningInvoiceWithOrigin,
    call: (wait: boolean, retry?: number) =>
      payLightningInvoiceWithOrigin(
        'invoice',
        { method: 'custom', value: 'destination' },
        wait,
        retry
      ),
    args: (wait: boolean, retry?: number) => [
      'invoice',
      { method: 'custom', value: 'destination' },
      wait,
      retry,
    ],
  },
])('$name retry controls', ({ native, call, args }) => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it.each([undefined, 0, 30, 0xffffffff])(
    'forwards retry duration %p independently of wait',
    async (retry) => {
      for (const wait of [false, true]) {
        const result = await call(wait, retry);
        expect(native).toHaveBeenLastCalledWith(...args(wait, retry));
        expect(result.state).toBe('in_progress');
      }
    }
  );
});
