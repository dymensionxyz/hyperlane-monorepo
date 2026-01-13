use dymension_kaspa::ops::confirmation::ConfirmationFXG;
use dymension_kaspa::relayer::confirm::expensive_trace_transactions;
use dymension_kaspa::KaspaProvider;
use eyre::Result;
use hyperlane_core::{ChainResult, HyperlaneChain, Signature};
use hyperlane_cosmos::native::{h512_to_cosmos_hash, CosmosNativeMailbox, ModuleQueryClient};
use hyperlane_cosmos::CosmosProvider;
use kaspa_consensus_core::tx::TransactionOutpoint;
use tracing::{error, info};

/// Ensures the hub anchor is in sync with the Kaspa escrow state.
///
/// This function checks if the hub's committed anchor outpoint exists in the current
/// escrow UTXOs. If not, it traces the transaction lineage and submits a confirmation
/// to update the hub anchor.
///
/// # Arguments
/// * `provider` - Kaspa provider for RPC and escrow info
/// * `hub_mailbox` - Hub mailbox for querying anchor and submitting confirmations
/// * `src_escrow` - Source escrow address (where anchor/old UTXOs are)
/// * `dst_escrow` - Destination escrow address (where current UTXOs are)
/// * `format_signatures` - Function to format signatures for hub submission
///
/// In normal operation, src and dst are the same. For post-migration sync,
/// src is the old escrow and dst is the new escrow.
pub async fn ensure_hub_synced<F>(
    provider: &KaspaProvider,
    hub_mailbox: &CosmosNativeMailbox,
    src_escrow: &str,
    dst_escrow: &str,
    format_signatures: F,
) -> Result<()>
where
    F: Fn(&mut Vec<Signature>) -> ChainResult<Vec<u8>>,
{
    info!(
        src_escrow,
        dst_escrow, "Checking if hub is out of sync with Kaspa escrow"
    );

    // Query hub for current anchor
    let provider_box = hub_mailbox.provider();
    let cosmos_prov = provider_box
        .as_any()
        .downcast_ref::<CosmosProvider<ModuleQueryClient>>()
        .expect("Hub mailbox provider must be CosmosProvider");
    let resp = cosmos_prov.query().outpoint(None).await?;
    let anchor_old = resp
        .outpoint
        .map(|o| TransactionOutpoint {
            transaction_id: kaspa_hashes::Hash::from_bytes(
                o.transaction_id.as_slice().try_into().unwrap(),
            ),
            index: o.index,
        })
        .ok_or_else(|| eyre::eyre!("No outpoint found on hub"))?;

    info!(
        anchor_tx = %anchor_old.transaction_id,
        anchor_index = anchor_old.index,
        "Got hub anchor"
    );

    // Fetch UTXOs from destination escrow (current escrow)
    let dst_addr: dymension_kaspa::KaspaAddress = dst_escrow
        .try_into()
        .map_err(|e| eyre::eyre!("Invalid dst escrow address: {}", e))?;

    let utxos = provider
        .rpc_with_reconnect(|api| {
            let addr = dst_addr.clone();
            async move {
                api.get_utxos_by_addresses(vec![addr])
                    .await
                    .map_err(|e| eyre::eyre!("Fetch UTXOs: {}", e))
            }
        })
        .await?;

    info!(utxo_count = utxos.len(), "Fetched destination escrow UTXOs");

    // Check if anchor is in current UTXOs - if yes, already synced
    let synced = utxos.iter().any(|utxo| {
        utxo.outpoint.transaction_id == anchor_old.transaction_id
            && utxo.outpoint.index == anchor_old.index
    });

    if synced {
        info!("Hub anchor found in escrow UTXOs, already synced");
        return Ok(());
    }

    info!("Hub anchor not in escrow UTXOs, syncing by tracing and confirming");

    // Try to trace from each UTXO back to the anchor
    let mut found = false;
    for utxo in utxos {
        let candidate = TransactionOutpoint::from(utxo.outpoint);

        let trace_result = expensive_trace_transactions(
            &provider.rest().client.client,
            src_escrow,
            dst_escrow,
            candidate.clone(),
            anchor_old,
        )
        .await;

        match trace_result {
            Ok(fxg) => {
                info!("Traced lineage, submitting confirmation to hub");
                confirm_withdrawal_on_hub(provider, hub_mailbox, fxg, &format_signatures).await?;
                found = true;
                break;
            }
            Err(e) => {
                error!(
                    error = ?e,
                    candidate_tx = %candidate.transaction_id,
                    "Trace failed for candidate"
                );
                continue;
            }
        }
    }

    if !found {
        return Err(eyre::eyre!(
            "No valid trace found from any UTXO to hub anchor"
        ));
    }

    info!("Hub synced successfully");
    Ok(())
}

/// Submit a confirmation to the hub to update the anchor.
async fn confirm_withdrawal_on_hub<F>(
    provider: &KaspaProvider,
    hub_mailbox: &CosmosNativeMailbox,
    fxg: ConfirmationFXG,
    format_signatures: &F,
) -> Result<()>
where
    F: Fn(&mut Vec<Signature>) -> ChainResult<Vec<u8>>,
{
    use dym_kas_core::finality::is_safe_against_reorg;

    // Check finality of the new anchor
    let anchor_new = fxg.outpoints.last().ok_or_else(|| {
        eyre::eyre!("No outpoints in confirmation FXG")
    })?;

    let finality = is_safe_against_reorg(
        &provider.rest().client.client,
        &anchor_new.transaction_id.to_string(),
        None,
    )
    .await
    .map_err(|e| eyre::eyre!("Finality check failed: {}", e))?;

    if !finality.is_final() {
        return Err(eyre::eyre!(
            "New anchor not final: {}/{} confirmations",
            finality.confirmations,
            finality.required_confirmations
        ));
    }

    info!(
        confirmations = finality.confirmations,
        "Finality check passed"
    );

    // Get confirmation signatures from validators
    let mut sigs = provider
        .validators()
        .get_confirmation_sigs(&fxg)
        .await
        .map_err(|e| eyre::eyre!("Get confirmation sigs: {}", e))?;

    info!(sig_count = sigs.len(), "Got confirmation signatures");

    // Format signatures
    let formatted = format_signatures(&mut sigs)
        .map_err(|e| eyre::eyre!("Format signatures: {}", e))?;

    // Submit to hub
    let outcome = hub_mailbox
        .indicate_progress(&formatted, &fxg.progress_indication)
        .await
        .map_err(|e| eyre::eyre!("Indicate progress: {}", e))?;

    let tx_hash = h512_to_cosmos_hash(outcome.transaction_id);

    if !outcome.executed {
        return Err(eyre::eyre!(
            "Confirmation TX not executed, hash: {:?}",
            tx_hash
        ));
    }

    info!(tx_hash = ?tx_hash, "Confirmation submitted to hub");
    Ok(())
}
