use anyhow::Context;

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

pub async fn sync_exits() -> anyhow::Result<()> {
    sync_exit().await
}
