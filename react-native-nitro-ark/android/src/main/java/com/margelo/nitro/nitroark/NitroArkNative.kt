package com.margelo.nitro.nitroark

import android.util.Log

/**
 * Kotlin facade for JNI helpers that call directly into the NitroArk C++/Rust layer.
 * This is intended for Android-only consumers who want to bypass the JS surface.
 */
object NitroArkNative {
  data class AndroidBarkConfig(
      val ark: String,
      val vtxoRefreshExpiryThreshold: Int,
      val fallbackFeeRate: Long,
      val serverAccessToken: String? = null,
      val esplora: String? = null,
      val bitcoind: String? = null,
      val bitcoindCookie: String? = null,
      val bitcoindUser: String? = null,
      val bitcoindPass: String? = null,
      val htlcRecvClaimDelta: Int? = null,
      val vtxoExitMargin: Int? = null,
      val roundTxRequiredConfirmations: Int? = null,
  )

  init {
    // Reuse existing loader to ensure the shared library is available.
    NitroArkOnLoad.initializeNative()
  }

  /**
   * Load an existing wallet using explicit chain/config values.
   */
  fun loadWallet(
      datadir: String,
      mnemonic: String,
      regtest: Boolean = false,
      signet: Boolean = false,
      bitcoin: Boolean = true,
      birthdayHeight: Int? = null,
      config: AndroidBarkConfig
  ) {
    Log.i("NitroArkNative", "loadWallet(datadir=$datadir regtest=$regtest signet=$signet bitcoin=$bitcoin)")
    loadWalletNative(
        datadir,
        mnemonic,
        regtest,
        signet,
        bitcoin,
        birthdayHeight,
        config.ark,
        config.serverAccessToken,
        config.esplora,
        config.bitcoind,
        config.bitcoindCookie,
        config.bitcoindUser,
        config.bitcoindPass,
        config.vtxoRefreshExpiryThreshold,
        config.fallbackFeeRate,
        config.htlcRecvClaimDelta,
        config.vtxoExitMargin,
        config.roundTxRequiredConfirmations)
  }

  external fun isWalletLoaded(): Boolean
  external fun closeWallet()

  // Native entrypoint with all parameters expanded for JNI.
  private external fun loadWalletNative(
      datadir: String,
      mnemonic: String,
      regtest: Boolean,
      signet: Boolean,
      bitcoin: Boolean,
      birthdayHeight: Int?,
      ark: String,
      serverAccessToken: String?,
      esplora: String?,
      bitcoind: String?,
      bitcoindCookie: String?,
      bitcoindUser: String?,
      bitcoindPass: String?,
      vtxoRefreshExpiryThreshold: Int,
      fallbackFeeRate: Long,
      htlcRecvClaimDelta: Int?,
      vtxoExitMargin: Int?,
      roundTxRequiredConfirmations: Int?,
  )

  // Additional helpers
  external fun maintenance()
  external fun maintenanceRefresh()
  external fun tryClaimLightningReceive(
      paymentHash: String,
      wait: Boolean,
      token: String?
  )
  external fun offboardAll(destinationAddress: String): RoundStatusResult
  external fun peekKeyPair(index: Int): KeyPairResultAndroid
  external fun verifyMessage(message: String, signature: String, publicKey: String): Boolean
  fun bolt11Invoice(amountMsat: Long, description: String? = null): Bolt11InvoiceResult =
      bolt11InvoiceNative(amountMsat, description)

  private external fun bolt11InvoiceNative(amountMsat: Long, description: String?): Bolt11InvoiceResult
  external fun signMessage(message: String, index: Int): String
  external fun sync()
}
