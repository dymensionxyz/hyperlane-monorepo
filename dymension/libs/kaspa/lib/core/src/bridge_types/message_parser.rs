/// Bridge-agnostic message parsing utilities
use super::{BridgeMessage, TokenTransfer};
use eyre::Result;

/// Parse a bridge message from raw bytes
pub fn parse_bridge_message(bytes: &[u8]) -> Result<BridgeMessage> {
    BridgeMessage::from_bytes(bytes)
}

/// Parse token transfer from message body
/// Expected format: recipient (32 bytes) + amount (16 bytes, little-endian u128) + metadata (rest)
pub fn parse_token_transfer(body: &[u8]) -> Result<TokenTransfer> {
    if body.len() < 48 {
        return Err(eyre::eyre!(
            "Token transfer body too short: {} bytes, expected at least 48",
            body.len()
        ));
    }

    let recipient_bytes = &body[0..32];
    let recipient = hex::encode(recipient_bytes);

    let amount_bytes: [u8; 16] = body[32..48]
        .try_into()
        .map_err(|_| eyre::eyre!("Failed to parse amount bytes"))?;
    let amount = u128::from_le_bytes(amount_bytes);

    let metadata = body[48..].to_vec();

    Ok(TokenTransfer {
        recipient,
        amount,
        metadata,
    })
}

/// Parse withdrawal amount from bridge message
pub fn parse_withdrawal_amount(msg: &BridgeMessage) -> Option<u128> {
    match parse_token_transfer(&msg.body) {
        Ok(transfer) => Some(transfer.amount),
        Err(e) => {
            tracing::error!(
                error = ?e,
                "Failed to parse token transfer for withdrawal amount"
            );
            None
        }
    }
}

/// Calculate total withdrawal amount from messages
pub fn calculate_total_withdrawal_amount(msgs: &[BridgeMessage]) -> u128 {
    msgs.iter().filter_map(parse_withdrawal_amount).sum()
}

/// Encode token transfer to message body
pub fn encode_token_transfer(transfer: &TokenTransfer) -> Result<Vec<u8>> {
    let recipient_bytes = hex::decode(&transfer.recipient)?;
    if recipient_bytes.len() != 32 {
        return Err(eyre::eyre!(
            "Recipient must be 32 bytes, got {}",
            recipient_bytes.len()
        ));
    }

    let mut body = Vec::with_capacity(48 + transfer.metadata.len());
    body.extend_from_slice(&recipient_bytes);
    body.extend_from_slice(&transfer.amount.to_le_bytes());
    body.extend_from_slice(&transfer.metadata);

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_transfer_roundtrip() {
        let transfer = TokenTransfer {
            recipient: hex::encode([1u8; 32]),
            amount: 1_000_000_u128,
            metadata: vec![1, 2, 3, 4],
        };

        let encoded = encode_token_transfer(&transfer).unwrap();
        let decoded = parse_token_transfer(&encoded).unwrap();

        assert_eq!(decoded.recipient, transfer.recipient);
        assert_eq!(decoded.amount, transfer.amount);
        assert_eq!(decoded.metadata, transfer.metadata);
    }

    #[test]
    fn test_parse_withdrawal_amount() {
        let transfer = TokenTransfer {
            recipient: hex::encode([1u8; 32]),
            amount: 5_000_000_u128,
            metadata: vec![],
        };

        let body = encode_token_transfer(&transfer).unwrap();
        let msg = BridgeMessage::new(1, 100, 1, [1u8; 32], 2, [2u8; 32], body);

        let amount = parse_withdrawal_amount(&msg).unwrap();
        assert_eq!(amount, 5_000_000_u128);
    }

    #[test]
    fn test_calculate_total_withdrawal_amount() {
        let transfers = vec![
            TokenTransfer {
                recipient: hex::encode([1u8; 32]),
                amount: 1_000_000_u128,
                metadata: vec![],
            },
            TokenTransfer {
                recipient: hex::encode([2u8; 32]),
                amount: 2_000_000_u128,
                metadata: vec![],
            },
            TokenTransfer {
                recipient: hex::encode([3u8; 32]),
                amount: 3_000_000_u128,
                metadata: vec![],
            },
        ];

        let messages: Vec<BridgeMessage> = transfers
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let body = encode_token_transfer(t).unwrap();
                BridgeMessage::new(1, i as u32, 1, [1u8; 32], 2, [2u8; 32], body)
            })
            .collect();

        let total = calculate_total_withdrawal_amount(&messages);
        assert_eq!(total, 6_000_000_u128);
    }

    #[test]
    fn test_parse_token_transfer_too_short() {
        let short_body = vec![1, 2, 3];
        let result = parse_token_transfer(&short_body);
        assert!(result.is_err());
    }
}
