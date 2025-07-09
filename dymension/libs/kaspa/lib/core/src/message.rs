use std::io::Cursor;

use eyre::Result;
use hyperlane_core::{Decode, HyperlaneMessage, RawHyperlaneMessage};
use hyperlane_cosmos_rs::dymensionxyz::dymension::forward::HlMetadata;
use hyperlane_warp_route::TokenMessage;
use kaspa_hashes::Hash;
use kaspa_consensus_core::tx::TransactionOutpoint;
pub use secp256k1::Keypair as KaspaSecpKeypair;
use prost::Message;

pub fn parse_hyperlane_message(m: &RawHyperlaneMessage) -> Result<HyperlaneMessage> {
    const MIN_EXPECTED_LENGTH: usize = 77;

    if m.len() < MIN_EXPECTED_LENGTH {
        return Err(eyre::eyre!("Value cannot be zero."));
    }
    let message = HyperlaneMessage::from(m);

    Ok(message)
}

pub fn parse_hyperlane_metadata(m: &HyperlaneMessage) -> Result<TokenMessage> {
    // decode token message inside  Hyperlane message
    let mut reader = Cursor::new(m.body.as_slice());
    let token_message = TokenMessage::read_from(&mut reader)
        .map_err(|e| eyre::eyre!("Failed to parse token message: {}", e))?;

    Ok(token_message)
}

pub fn add_kaspa_metadata_hl_messsage(token_message: TokenMessage,transaction_id: Hash, utxo_index: usize) -> Result<TokenMessage> {


    let output = TransactionOutpoint {
        transaction_id: transaction_id,
        index: utxo_index as u32,
    };

    let output_bytes = bincode::serialize(&output)?;

    let mut metadata: HlMetadata;
    if token_message.metadata().is_empty() {
        metadata = HlMetadata {
            hook_forward_to_ibc: Vec::new(),
            kaspa: output_bytes,
        };
    } else {
        metadata = HlMetadata::decode(token_message.metadata())?;
        // replace kaspa value and reencode message
        metadata.kaspa = output_bytes;
    }
    Ok(TokenMessage::new(
        token_message.recipient(),
        token_message.amount(),
        metadata.encode_to_vec(),
    ))
}
