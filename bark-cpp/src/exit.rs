use anyhow::{Context, bail};
use bark::vtxo::{FilterVtxos, VtxoFilter};
use bdk_wallet::bitcoin::{Address, FeeRate, Psbt};
use std::str::FromStr;

use crate::GLOBAL_WALLET_MANAGER;

pub async fn start_exit_for_entire_wallet() -> anyhow::Result<()> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            ctx.wallet
                .exit_mgr()
                .start_exit_for_entire_wallet()
                .await
                .context("Failed to start exit for entire wallet")?;
            Ok(())
        })
        .await
}

pub async fn start_exit_for_vtxos(vtxo_ids: Vec<String>) -> anyhow::Result<()> {
    if vtxo_ids.is_empty() {
        bail!("No VTXO IDs provided");
    }

    let vtxo_ids = vtxo_ids
        .into_iter()
        .map(|id| {
            bark::ark::VtxoId::from_str(&id).with_context(|| format!("Invalid VTXO ID: {id}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async move {
            for id in &vtxo_ids {
                ctx.wallet
                    .get_vtxo_by_id(*id)
                    .await
                    .with_context(|| format!("VTXO not found: {id}"))?;
            }

            let filter = VtxoFilter::new(ctx.wallet.as_ref()).include_many(vtxo_ids);
            let spendable = ctx
                .wallet
                .spendable_vtxos_with(&filter)
                .await
                .context("Error fetching spendable VTXOs")?;
            let inround = {
                let mut vtxos = ctx
                    .wallet
                    .pending_round_input_vtxos()
                    .await
                    .context("Error fetching pending round input VTXOs")?;
                filter.filter_vtxos(&mut vtxos).await?;
                vtxos
            };

            let vtxos = spendable
                .into_iter()
                .chain(inround)
                .map(|wallet_vtxo| wallet_vtxo.vtxo)
                .collect::<Vec<_>>();

            ctx.wallet
                .exit_mgr()
                .start_exit_for_vtxos(&vtxos)
                .await
                .context("Failed to start exit for VTXOs")?;
            Ok(())
        })
        .await
}

pub async fn sync_exit() -> anyhow::Result<()> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            ctx.wallet
                .sync_exits()
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
                .exit_mgr()
                .progress_exits_with_bdk(ctx.wallet.as_ref(), &mut ctx.onchain_wallet, fee_rate)
                .await
                .context("Failed to progress exits")?;
            Ok(result.unwrap_or_default())
        })
        .await
}

pub async fn get_exit_vtxos() -> anyhow::Result<Vec<bark::exit::ExitVtxo>> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async { Ok(ctx.wallet.exit_mgr().get_exit_vtxos().await) })
        .await
}

pub async fn list_claimable() -> anyhow::Result<Vec<bark::exit::ExitVtxo>> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            Ok(ctx
                .wallet
                .exit_mgr()
                .list_claimable()
                .await
                .into_iter()
                .collect())
        })
        .await
}

pub async fn get_exit_status(
    vtxo_id: String,
    include_history: bool,
    include_transactions: bool,
) -> anyhow::Result<Option<bark::exit::ExitTransactionStatus>> {
    let vtxo_id = bark::ark::VtxoId::from_str(&vtxo_id)?;
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async move {
            ctx.wallet
                .exit_mgr()
                .get_exit_status(vtxo_id, include_history, include_transactions)
                .await
                .context("Failed to get exit status")
        })
        .await
}

pub async fn has_pending_exits() -> anyhow::Result<bool> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async { Ok(ctx.wallet.exit_mgr().has_pending_exits().await) })
        .await
}

pub async fn pending_exit_total() -> anyhow::Result<bark::ark::bitcoin::Amount> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            ctx.wallet
                .exit_mgr()
                .try_pending_total()
                .context("Exit manager is currently locked")
        })
        .await
}

pub async fn all_claimable_at_height() -> anyhow::Result<Option<u32>> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async {
            Ok(ctx.wallet.exit_mgr().all_claimable_at_height().await)
        })
        .await
}

pub async fn drain_exits(
    vtxo_ids: Vec<String>,
    address: Address,
    fee_rate: Option<FeeRate>,
) -> anyhow::Result<Psbt> {
    let mut manager = GLOBAL_WALLET_MANAGER.lock().await;
    manager
        .with_context_async(|ctx| async move {
            let exit = ctx.wallet.exit_mgr();
            let inputs = vtxo_ids
                .iter()
                .map(|id| {
                    let vtxo_id = bark::ark::VtxoId::from_str(id)?;
                    Ok(vtxo_id)
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            let mut exit_vtxos = Vec::with_capacity(inputs.len());
            for (id, vtxo_id) in vtxo_ids.iter().zip(inputs) {
                let exit_vtxo = exit
                    .get_exit_vtxo(vtxo_id)
                    .await
                    .with_context(|| format!("Exit VTXO not found: {}", id))?;
                exit_vtxos.push(exit_vtxo);
            }

            exit.drain_exits(&exit_vtxos, ctx.wallet.as_ref(), address, fee_rate)
                .await
                .context("Failed to drain exits")
        })
        .await
}
