use super::hub_to_kaspa::{
    build_withdrawal_pskt, fetch_input_utxos, fetch_input_utxos_1, filter_outputs_from_msgs,
};
use corelib::consts::RELAYER_SIG_OP_COUNT;
use corelib::escrow::EscrowPublic;
use corelib::payload::MessageIDs;
use corelib::wallet::EasyKaspaWallet;
use corelib::withdraw::{filter_pending_withdrawals, WithdrawFXG};
use eyre::Result;
use hyperlane_core::HyperlaneMessage;
use hyperlane_core::U256;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use kaspa_consensus_core::tx::TransactionOutpoint;
use kaspa_wallet_pskt::prelude::Bundle;
use tracing::info;

/// Processes given messages and returns WithdrawFXG and the very first outpoint
/// (the one preceding all the given transfers; it should be used during process indication).
pub async fn on_new_withdrawals(
    messages: Vec<HyperlaneMessage>,
    relayer: EasyKaspaWallet,
    cosmos: CosmosGrpcClient,
    escrow_public: EscrowPublic,
    min_deposit_sompi: U256,
) -> Result<Option<WithdrawFXG>> {
    info!("Kaspa relayer, getting pending withdrawals");
    let (current_anchor, pending_msgs) = filter_pending_withdrawals(messages, &cosmos)
        .await
        .map_err(|e| eyre::eyre!("Get pending withdrawals: {}", e))?;
    info!("Kaspa relayer, got pending withdrawals");

    let (valid_msgs, outputs) =
        filter_outputs_from_msgs(pending_msgs, relayer.net.address_prefix, min_deposit_sompi);

    if outputs.is_empty() {
        info!("Kaspa relayer, no valid pending withdrawals, all in batch are already processed and confirmed on hub");
        return Ok(None); // nothing to process
    }
    info!(
        "Kaspa relayer, got pending withdrawals, building PSKT, len: {}",
        outputs.len()
    );

    let escrow_utxos = fetch_input_utxos(
        &relayer.api(),
        &escrow_public.addr,
        escrow_public.redeem_script.clone(),
        escrow_public.n() as u8,
        relayer.net.network_id,
    )
    .await
    .map_err(|e| eyre::eyre!("Fetch escrow UTXOs: {}", e))?;

    // Check if the current anchor is within the list of multisig UTXOs
    if !escrow_utxos.iter().any(|(i, u)| {
        i.previous_outpoint.transaction_id == current_anchor.transaction_id
            && i.previous_outpoint.index == current_anchor.index
    }) {
        return Err(eyre::eyre!(
            "No UTXOs found for current anchor: {:?}",
            current_anchor
        ));
    }

    let relayer_address = relayer.account().change_address()?;

    let relayer_utxos = fetch_input_utxos(
        &relayer.api(),
        &relayer_address,
        vec![],
        RELAYER_SIG_OP_COUNT,
        relayer.net.network_id,
    )
    .await
    .map_err(|e| eyre::eyre!("Fetch relayer UTXOs: {}", e))?;

    let payload = MessageIDs::from(&valid_msgs).to_bytes();

    let pskt = build_withdrawal_pskt(
        [escrow_utxos, relayer_utxos].concat(),
        outputs,
        payload,
        &escrow_public,
        &relayer_address,
        relayer.net.network_id,
        min_deposit_sompi,
    )
    .map_err(|e| eyre::eyre!("Build withdrawal PSKT: {}", e))?;

    let new_anchor = TransactionOutpoint::new(pskt.calculate_id(), (pskt.outputs.len() - 1) as u32);

    // We have a bundle with one PSKT which covers all the HL messages.
    Ok(Some(WithdrawFXG::new(
        Bundle::from(pskt),
        vec![valid_msgs],
        vec![current_anchor, new_anchor],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use hyperlane_core::Decode;
    use hyperlane_warp_route::TokenMessage;
    use kaspa_hashes::Hash;
    use kaspa_wallet_core::tx::{Generator, GeneratorSettings};
    use std::io::Cursor;

    #[test]
    fn test_transaction_id_conversion() {
        // Test with valid 32-byte transaction ID
        let b64 = "Xhz2eE568YCGdKJS60F9j6ADE1GQ3UFHyvmNhGOn5zo=";
        let bytes = STANDARD.decode(b64).unwrap();
        let bz = bytes.as_slice().try_into().unwrap();
        let kaspa_tx_id = kaspa_hashes::Hash::from_bytes(bz);
        println!("kaspa_tx_id: {:?}", kaspa_tx_id);
    }

    #[test]
    fn test_decode_token_message() {
        let bytes_a: Vec<Vec<u8>> = vec![
            vec![
                223, 45, 201, 23, 84, 12, 115, 128, 168, 110, 81, 250, 212, 184, 225, 16, 26, 14,
                250, 39, 71, 58, 92, 169, 185, 124, 235, 132, 108, 196, 2, 171, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 49, 45, 2,
            ],
            vec![
                223, 45, 201, 23, 84, 12, 115, 128, 168, 110, 81, 250, 212, 184, 225, 16, 26, 14,
                250, 39, 71, 58, 92, 169, 185, 124, 235, 132, 108, 196, 2, 171, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 49, 45, 2,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 119, 53,
                148, 0,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 119, 53,
                148, 0,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 154,
                202, 0,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 131, 33,
                86, 0,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 131, 33,
                86, 0,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 131, 33,
                86, 0,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 84, 11,
                228, 0,
            ],
            vec![
                188, 255, 117, 135, 245, 116, 226, 73, 181, 73, 50, 146, 145, 35, 150, 130, 214,
                211, 72, 28, 203, 197, 153, 124, 121, 119, 10, 96, 122, 179, 236, 152, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 131, 33,
                86, 0,
            ],
        ];

        for (i, bytes) in bytes_a.iter().enumerate() {
            // Create a Cursor around the byte array for the reader
            let mut reader = Cursor::new(bytes);

            // Decode the byte array into a TokenMessage
            let token_message =
                TokenMessage::read_from(&mut reader).expect("Failed to decode TokenMessage");

            println!("#{:?}: {:?}", i, token_message);
        }
    }

    // #[test]
    // fn test_kaspa_tx_generator() {
    //     let settings = GeneratorSettings::try_new_with_context();
    //     let gen = Generator::try_new();
    // }
}
