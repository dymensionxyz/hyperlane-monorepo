use anyhow::Result;
use core::escrow::EscrowPublic;
use hyperlane_core::{Decode, HyperlaneMessage, H256};
use hyperlane_cosmos_native::CosmosNativeProvider;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use hyperlane_warp_route::TokenMessage;
use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::constants::TX_VERSION;
use kaspa_consensus_core::mass::calc_storage_mass;
use kaspa_consensus_core::network::NetworkId;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
use kaspa_consensus_core::tx::{ScriptPublicKey, UtxoEntry};
use kaspa_consensus_core::tx::{
    Transaction, TransactionInput, TransactionOutpoint, TransactionOutput,
};
use kaspa_hashes;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::{RpcUtxoEntry, RpcUtxosByAddressesEntry};
use kaspa_txscript;
use kaspa_txscript::standard::pay_to_address_script;
use kaspa_wallet_core::account::Account;
use kaspa_wallet_core::tx::{is_transaction_output_dust, MassCalculator};
use kaspa_wallet_core::utxo::NetworkParams;
use kaspa_wallet_pskt::prelude::*;
use kaspa_wallet_pskt::prelude::{Signer, PSKT};
use std::io::Cursor;
use std::sync::Arc;

/// Details of a withdrawal extracted from HyperlaneMessage
#[derive(Debug, Clone)]
struct WithdrawalDetails {
    pub message_id: H256, // MessageID from HyperlaneMessage.id() TODO: where to use it?
    pub recipient: kaspa_addresses::Address,
    pub amount_sompi: u64,
}

/// Builds a single withdrawal PSKT.
///
/// Example:
///
/// The user sends 10 KAS. Multisig addr has 100 KAS. Due to the Hyperlane approach, the user
/// needs to get the whole amount they transferred, so they must get 10 KAS. However, there is
/// the transaction fee, which must be covered by the relayer. Let's say it's 1 KAS.
///
/// For that, we fetch ALL UTXOs from the multisig address and them as inputs. This will also
/// work as automatic sweeping. The change is returned as an output which is also used as
/// a new anchor.
///
/// The relayer fee is tricky. Relayer should provide some UTXOs to cover the fee. However,
/// each input increases the transaction fee, so we can't compute the concrete fee beforehand.
///
/// We have two options:
///
/// --- 1 ---
/// 1. Calculate the tx fee without relayer's UTXOs.
/// 2. Get the UTXOs that cover the fee.
/// 3. Add them as inputs.
/// 4. Calculate the fee again.
/// 5. Add additional UTXOs if needed and repeat 2-4.
///
/// Pros: As low fee as possible.
/// Cons: The relayer account is fragmented (sweeping is needed); complex flow.
///
/// --- 2 --- (Implemented)
/// Get ALL UTXOs and also use them as inputs. The change is returned as output.
///
/// Pros: Simple to handle.
/// Cons: Potentially bigger fee because of the increased number of inputs. However, it's in
/// relayer's interest to pay min fees and thus keep its account with as few UTXOs as possible.
pub async fn build_withdrawal_pskts(
    messages: Vec<&HyperlaneMessage>,
    hub_height: Option<u32>,
    cosmos_provider: &CosmosNativeProvider,
    kaspa_rpc: &impl RpcApi,
    escrow_public: &EscrowPublic,
    relayer_kaspa_account: &Arc<dyn Account>,
    network_id: NetworkId,
) -> Result<Option<PSKT<Signer>>> {
    let (outpoint, pending_messages) =
        get_pending_withdrawals(messages, cosmos_provider, hub_height).await?;

    let withdrawal_details: Vec<_> = pending_messages
        .into_iter()
        .filter_map(|m| {
            match TokenMessage::read_from(&mut Cursor::new(&m.body)) {
                Ok(msg) => {
                    let kr = match kaspa_addresses::Address::try_from(m.recipient.to_string()) {
                        Ok(addr) => Some(addr),
                        Err(e) => None, // TODO: log error?
                    }?;

                    Some(WithdrawalDetails {
                        message_id: m.id(),
                        recipient: kr,
                        amount_sompi: msg.amount().as_u64(),
                    })
                }
                Err(e) => {
                    eprintln!(
                        "Failed to parse TokenMessage for message_id {:?}: {}",
                        m.id(),
                        e
                    );
                    None
                }
            }
        })
        .collect();

    if withdrawal_details.is_empty() {
        return Ok(None);
    }

    internal_build_withdrawal_pskt(
        withdrawal_details,
        kaspa_rpc,
        escrow_public,
        relayer_kaspa_account,
        &outpoint,
        network_id,
    )
    .await
    .map(Some)
}

async fn internal_build_withdrawal_pskt(
    withdrawal_details: Vec<WithdrawalDetails>,
    kaspa_rpc: &impl RpcApi,
    escrow_public: &EscrowPublic,
    relayer_account: &Arc<dyn Account>,
    current_anchor: &TransactionOutpoint,
    network_id: NetworkId,
) -> Result<PSKT<Signer>> {
    //////////////////
    //     UTXO     //
    //////////////////

    // Get all available UTXOs from multisig
    let escrow_utxos = get_utxo_to_spend(escrow_public.addr.clone(), kaspa_rpc, network_id).await?;

    // Check if the current anchor is withing the list of multisig UTXOs
    if !escrow_utxos.iter().any(|u| {
        u.outpoint.transaction_id == current_anchor.transaction_id
            && u.outpoint.index == current_anchor.index
    }) {
        return Err(anyhow::anyhow!(
            "No UTXOs found for current anchor: {:?}",
            current_anchor
        ));
    }

    let relayer_utxos = get_utxo_to_spend(
        // TODO: receive_address or change_address??
        relayer_account.receive_address()?.clone(),
        kaspa_rpc,
        network_id,
    )
    .await?;

    //////////////////
    //   Balances   //
    //////////////////

    // TODO: Confirm if we can have an overflow here
    // 1 KAS = 10^8 (dust denom).
    // 10^19 < 2^26 < 10^20
    // This means the multisig must hold at most 10^19 (dust denom) => 10^11 KAS
    // Given that 1 KAS = $10^-2, the max balance is $1B, but this might change
    // in case of hyperinflation

    let escrow_balance = escrow_utxos
        .iter()
        .fold(0, |acc, u| acc + u.utxo_entry.amount);

    let withdrawal_balance = withdrawal_details
        .iter()
        .fold(0, |acc, w| acc + w.amount_sompi);

    if escrow_balance < withdrawal_balance {
        return Err(anyhow::anyhow!(
            "Insufficient funds in escrow: {} < {}",
            escrow_balance,
            withdrawal_balance
        ));
    }

    let relayer_balance = relayer_utxos
        .iter()
        .fold(0, |acc, u| acc + u.utxo_entry.amount);

    ////////////////////
    // Input & Output //
    ////////////////////

    // Iterate through escrow and relayer UTXO – they would be transaction inputs.
    // Create a vector of "populated" inputs: TransactionInput and UtxoEntry.
    let inputs: Vec<(TransactionInput, UtxoEntry)> = escrow_utxos
        .into_iter()
        .chain(relayer_utxos.into_iter())
        .enumerate()
        .map(|(index, utxo)| {
            (
                // signature_script is empty, reference: https://github.com/kaspanet/rusty-kaspa/blob/v1.0.0/wallet/pskt/src/pskt.rs#L138
                TransactionInput::new(
                    kaspa_consensus_core::tx::TransactionOutpoint::from(utxo.outpoint),
                    vec![], // signature_script
                    index as u64,
                    escrow_public.n() as u8,
                ),
                UtxoEntry::from(utxo.utxo_entry),
            )
        })
        .collect();

    let outputs: Vec<TransactionOutput> = withdrawal_details
        .into_iter()
        .map(|w| {
            TransactionOutput::new(
                w.amount_sompi,
                ScriptPublicKey::from(pay_to_address_script(&w.recipient)),
            )
        })
        .collect();

    // Copy of https://github.com/kaspanet/rusty-kaspa/blob/v1.0.0/wallet/pskt/src/pskt.rs#L131-157
    let transaction = Transaction::new(
        TX_VERSION,
        inputs.iter().map(|(input, _)| (*input).clone()).collect(),
        outputs.clone(),
        0, // no tx lock time
        SUBNETWORK_ID_NATIVE,
        0,
        vec![], // empty payload
    );

    //////////////////
    //     Fee      //
    //////////////////

    let p = Params::from(network_id);
    let mc = MassCalculator::new(&p);

    let storage_mass = calc_storage_mass(
        false,
        inputs.iter().map(|(_, entry)| entry.into()),
        outputs.iter().map(|output| output.into()),
        p.storage_mass_parameter,
    )
    .ok_or(kaspa_wallet_core::error::Error::MassCalculationError)?;

    let compute_mass = mc.calc_compute_mass_for_unsigned_consensus_transaction(
        &transaction,
        escrow_public.n() as u16,
    );

    // Multiply the fee by 1.1 to give some space for adding change utxos
    let tx_fee = mc.calc_fee_for_mass(mc.combine_mass(storage_mass, compute_mass)) * 11 / 10;

    if relayer_balance < tx_fee {
        return Err(anyhow::anyhow!(
            "Insufficient relayer funds to cover tx fee: {} < {}",
            relayer_balance,
            tx_fee
        ));
    }

    //////////////////
    //    Change    //
    //////////////////

    // escrow_balance - withdrawal_balance > 0 as checked above
    let escrow_change = TransactionOutput::new(
        escrow_balance - withdrawal_balance,
        ScriptPublicKey::from(pay_to_address_script(&escrow_public.addr)),
    );

    // relayer_balance - tx_fee as checked above
    // TODO: receive_address or change_address??
    let relayer_change = TransactionOutput::new(
        relayer_balance - tx_fee,
        ScriptPublicKey::from(pay_to_address_script(
            &relayer_account.change_address()?.clone(),
        )),
    );

    //////////////////
    //     PSKT     //
    //////////////////

    // Create a transaction builder based on the tx – all inputs and outputs are saved.
    let populated_inputs: Vec<(&TransactionInput, &UtxoEntry)> =
        inputs.iter().map(|(input, utxo)| (input, utxo)).collect();

    let inner = Inner::try_from((transaction, populated_inputs))?;
    let pskt = <PSKT<Constructor>>::from(inner);

    if !is_transaction_output_dust(&escrow_change) {
        pskt = pskt.output(escrow_change);
    }

    if !is_transaction_output_dust(&relayer_change) {
        pskt = pskt.output(relayer_change);
    }

    Ok(pskt.no_more_inputs().no_more_outputs().signer())
}

async fn get_utxo_to_spend(
    addr: kaspa_addresses::Address,
    kaspa_rpc: &impl RpcApi,
    network_id: NetworkId,
) -> Result<Vec<RpcUtxosByAddressesEntry>> {
    let mut utxos = kaspa_rpc
        .get_utxos_by_addresses(vec![addr.clone()])
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get escrow UTXOs: {}", e))?;

    // Descending order – older UTXOs first
    utxos.sort_by_key(|u| std::cmp::Reverse(u.utxo_entry.block_daa_score));

    let mut selected = Vec::new();
    let mut total_in = 0u64;

    let block = kaspa_rpc
        .get_block_dag_info()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get block DAG info: {}", e))?;
    let current_daa_score = block.virtual_daa_score;

    for utxo in utxos {
        if !is_mature(&utxo.utxo_entry, current_daa_score, network_id) {
            continue;
        }

        total_in += utxo.utxo_entry.amount;
        selected.push(utxo);
    }

    Ok(selected)
}

fn is_mature(utxo: &RpcUtxoEntry, current_daa_score: u64, network_id: NetworkId) -> bool {
    match maturity_progress(utxo, current_daa_score, network_id) {
        Some(_) => false,
        None => true,
    }
}

// Copy https://github.com/kaspanet/rusty-kaspa/blob/v1.0.0/wallet/core/src/storage/transaction/record.rs
fn maturity_progress(
    utxo: &RpcUtxoEntry,
    current_daa_score: u64,
    network_id: NetworkId,
) -> Option<f64> {
    let params = NetworkParams::from(network_id);
    let maturity = if utxo.is_coinbase {
        params.coinbase_transaction_maturity_period_daa()
    } else {
        params.user_transaction_maturity_period_daa()
    };

    if current_daa_score < utxo.block_daa_score + maturity {
        Some((current_daa_score - utxo.block_daa_score) as f64 / maturity as f64)
    } else {
        None
    }
}

async fn get_pending_withdrawals(
    withdrawals: Vec<&HyperlaneMessage>,
    cosmos_provider: &CosmosNativeProvider,
    height: Option<u32>,
) -> Result<(TransactionOutpoint, Vec<HyperlaneMessage>)> {
    // A list of withdrawal IDs to request their statuses from the Hub
    let withdrawal_ids: Vec<_> = withdrawals
        .iter()
        .map(|m| WithdrawalId {
            message_id: m.id().to_string(),
        })
        .collect();

    // Request withdrawal statuses from the Hub
    let resp = match height {
        Some(h) => {
            cosmos_provider
                .grpc()
                .withdrawal_status(withdrawal_ids, Some(h))
                .await
        }
        None => {
            cosmos_provider
                .grpc()
                .withdrawal_status(withdrawal_ids, None)
                .await
        }
    }
    .map_err(|e| anyhow::anyhow!("Failed to query outpoint from x/kas module: {}", e))?;

    let outpoint_data = resp
        .outpoint
        .ok_or_else(|| anyhow::anyhow!("No outpoint data in response"))?;

    if outpoint_data.transaction_id.len() != 32 {
        return Err(anyhow::anyhow!(
            "Invalid transaction ID length: expected 32 bytes, got {}",
            outpoint_data.transaction_id.len()
        ));
    }

    // Convert the transaction ID to kaspa transaction ID
    let kaspa_tx_id = kaspa_hashes::Hash::from_bytes(
        outpoint_data
            .transaction_id
            .as_slice()
            .try_into()
            .map_err(|e| anyhow::anyhow!("Failed to convert transaction ID to array: {:?}", e))?,
    );

    // resp.status is a list of the same length as withdrawals. If status == WithdrawalStatus::Unprocessed,
    // then the respective element of withdrawals is Unprocessed.
    let pending_withdrawals: Vec<_> = resp
        .status
        .into_iter()
        .enumerate()
        .filter_map(|(idx, status)| match WithdrawalStatus::from_i32(status) {
            Some(WithdrawalStatus::Unprocessed) => Some(withdrawals[idx].clone()),
            _ => None, // Ignore other statuses
        })
        .collect();

    Ok((
        TransactionOutpoint {
            transaction_id: kaspa_tx_id,
            index: outpoint_data.index,
        },
        pending_withdrawals,
    ))
}
