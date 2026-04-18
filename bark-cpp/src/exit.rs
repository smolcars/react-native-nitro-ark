use anyhow::Context;
use bdk_wallet::bitcoin::FeeRate;

use crate::GLOBAL_WALLET_MANAGER;

pub async fn start_exit_for_entire_wallet() -> anyhow::Result<()> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            ctx.wallet
                .exit
                .write()
                .await
                .start_exit_for_entire_wallet()
                .await
                .context("Failed to start exit for entire wallet")?;
            Ok(())
        })
        .await
}

pub async fn sync_exit() -> anyhow::Result<()> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            ctx.wallet
                .exit
                .write()
                .await
                .sync(ctx.wallet.as_ref(), &mut ctx.onchain_wallet)
                .await
                .context("Failed to sync exit")?;
            Ok(())
        })
        .await
}

pub async fn progress_exits(
    fee_rate: Option<FeeRate>,
) -> anyhow::Result<Vec<bark::exit::ExitProgressStatus>> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            let result = ctx
                .wallet
                .exit
                .write()
                .await
                .progress_exits(ctx.wallet.as_ref(), &mut ctx.onchain_wallet, fee_rate)
                .await
                .context("Failed to progress exits")?;
            Ok(result.unwrap_or_default())
        })
        .await
}

pub async fn get_exit_vtxos() -> anyhow::Result<Vec<bark::exit::ExitVtxo>> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            Ok(ctx.wallet.exit.read().await.get_exit_vtxos().clone())
        })
        .await
}

pub async fn has_pending_exits() -> anyhow::Result<bool> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async { Ok(ctx.wallet.exit.read().await.has_pending_exits()) })
        .await
}

pub async fn pending_exit_total() -> anyhow::Result<bark::ark::bitcoin::Amount> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async { Ok(ctx.wallet.exit.read().await.pending_total()) })
        .await
}

pub async fn all_claimable_at_height() -> anyhow::Result<Option<u32>> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            Ok(ctx.wallet.exit.read().await.all_claimable_at_height().await)
        })
        .await
}

pub async fn sync_exits() -> anyhow::Result<()> {
    sync_exit().await
}
