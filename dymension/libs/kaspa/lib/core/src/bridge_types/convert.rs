/// Conversion utilities between Hyperlane types and bridge-agnostic types
/// This module provides the adapter layer for rust/main to use
use super::{BridgeMessage, DepositResult};

#[cfg(feature = "hyperlane-compat")]
use hyperlane_core::{HyperlaneMessage, H256, U256};

/// Convert HyperlaneMessage to BridgeMessage
#[cfg(feature = "hyperlane-compat")]
pub fn hyperlane_to_bridge(msg: &HyperlaneMessage) -> BridgeMessage {
    BridgeMessage {
        version: msg.version,
        nonce: msg.nonce,
        origin_domain: msg.origin,
        sender: msg.sender.into(),
        destination_domain: msg.destination,
        recipient: msg.recipient.into(),
        body: msg.body.clone(),
    }
}

/// Convert BridgeMessage to HyperlaneMessage
#[cfg(feature = "hyperlane-compat")]
pub fn bridge_to_hyperlane(msg: &BridgeMessage) -> HyperlaneMessage {
    HyperlaneMessage {
        version: msg.version,
        nonce: msg.nonce,
        origin: msg.origin_domain,
        sender: H256::from(msg.sender),
        destination: msg.destination_domain,
        recipient: H256::from(msg.recipient),
        body: msg.body.clone(),
    }
}

/// Convert U256 to u128 (with overflow check)
#[cfg(feature = "hyperlane-compat")]
pub fn u256_to_u128(amount: U256) -> Option<u128> {
    if amount > U256::from(u128::MAX) {
        None
    } else {
        Some(amount.as_u128())
    }
}

/// Convert u128 to U256
#[cfg(feature = "hyperlane-compat")]
pub fn u128_to_u256(amount: u128) -> U256 {
    U256::from(amount)
}

/// Convert H256 to [u8; 32]
#[cfg(feature = "hyperlane-compat")]
pub fn h256_to_bytes(h: H256) -> [u8; 32] {
    h.into()
}

/// Convert [u8; 32] to H256
#[cfg(feature = "hyperlane-compat")]
pub fn bytes_to_h256(bytes: [u8; 32]) -> H256 {
    H256::from(bytes)
}

/// Convert DepositFXG (Hyperlane) to DepositResult (bridge-agnostic)
#[cfg(feature = "hyperlane-compat")]
pub fn depositfxg_to_deposit_result(deposit: &crate::deposit::DepositFXG) -> DepositResult {
    DepositResult {
        tx_hash: deposit.tx_id.clone(),
        utxo_index: deposit.utxo_index,
        amount: u256_to_u128(deposit.amount).unwrap_or(u128::MAX),
        accepting_block_hash: deposit.accepting_block_hash.clone(),
        containing_block_hash: deposit.containing_block_hash.clone(),
        message: hyperlane_to_bridge(&deposit.hl_message),
        confirmation_count: 0,
    }
}

/// Convert DepositResult (bridge-agnostic) to DepositFXG (Hyperlane)
#[cfg(feature = "hyperlane-compat")]
pub fn deposit_result_to_depositfxg(result: &DepositResult) -> crate::deposit::DepositFXG {
    crate::deposit::DepositFXG {
        amount: u128_to_u256(result.amount),
        tx_id: result.tx_hash.clone(),
        utxo_index: result.utxo_index,
        accepting_block_hash: result.accepting_block_hash.clone(),
        hl_message: bridge_to_hyperlane(&result.message),
        containing_block_hash: result.containing_block_hash.clone(),
    }
}

#[cfg(all(test, feature = "hyperlane-compat"))]
mod tests {
    use super::*;

    #[test]
    fn test_hyperlane_bridge_roundtrip() {
        let hl_msg = HyperlaneMessage {
            version: 1,
            nonce: 100,
            origin: 1,
            sender: H256::from([1u8; 32]),
            destination: 2,
            recipient: H256::from([2u8; 32]),
            body: vec![1, 2, 3, 4],
        };

        let bridge_msg = hyperlane_to_bridge(&hl_msg);
        let hl_msg_back = bridge_to_hyperlane(&bridge_msg);

        assert_eq!(hl_msg.version, hl_msg_back.version);
        assert_eq!(hl_msg.nonce, hl_msg_back.nonce);
        assert_eq!(hl_msg.origin, hl_msg_back.origin);
        assert_eq!(hl_msg.sender, hl_msg_back.sender);
        assert_eq!(hl_msg.destination, hl_msg_back.destination);
        assert_eq!(hl_msg.recipient, hl_msg_back.recipient);
        assert_eq!(hl_msg.body, hl_msg_back.body);
    }

    #[test]
    fn test_u256_u128_conversion() {
        let amount_u128 = 1000000u128;
        let amount_u256 = u128_to_u256(amount_u128);
        let amount_back = u256_to_u128(amount_u256).unwrap();

        assert_eq!(amount_u128, amount_back);
    }

    #[test]
    fn test_u256_overflow() {
        let large_u256 = U256::from(u128::MAX) + U256::from(1);
        assert!(u256_to_u128(large_u256).is_none());
    }

    #[test]
    fn test_h256_bytes_roundtrip() {
        let bytes = [42u8; 32];
        let h = bytes_to_h256(bytes);
        let bytes_back = h256_to_bytes(h);

        assert_eq!(bytes, bytes_back);
    }
}
