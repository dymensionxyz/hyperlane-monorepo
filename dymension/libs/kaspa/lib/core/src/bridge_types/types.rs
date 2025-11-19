/// Bridge-agnostic types for Kaspa bridge
/// These types have NO dependencies on Hyperlane and can be used independently
use eyre::Result;
use serde::{Deserialize, Serialize};

/// Generic bridge message without Hyperlane dependency
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeMessage {
    pub version: u8,
    pub nonce: u32,
    pub origin_domain: u32,
    pub sender: [u8; 32],
    pub destination_domain: u32,
    pub recipient: [u8; 32],
    pub body: Vec<u8>,
}

impl BridgeMessage {
    pub fn new(
        version: u8,
        nonce: u32,
        origin_domain: u32,
        sender: [u8; 32],
        destination_domain: u32,
        recipient: [u8; 32],
        body: Vec<u8>,
    ) -> Self {
        Self {
            version,
            nonce,
            origin_domain,
            sender,
            destination_domain,
            recipient,
            body,
        }
    }

    pub fn id(&self) -> [u8; 32] {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update([self.version]);
        hasher.update(self.nonce.to_be_bytes());
        hasher.update(self.origin_domain.to_be_bytes());
        hasher.update(self.sender);
        hasher.update(self.destination_domain.to_be_bytes());
        hasher.update(self.recipient);
        hasher.update(&self.body);
        hasher.finalize().into()
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.version);
        buf.extend_from_slice(&self.nonce.to_be_bytes());
        buf.extend_from_slice(&self.origin_domain.to_be_bytes());
        buf.extend_from_slice(&self.sender);
        buf.extend_from_slice(&self.destination_domain.to_be_bytes());
        buf.extend_from_slice(&self.recipient);
        buf.extend_from_slice(&self.body);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 77 {
            return Err(eyre::eyre!("Message too short: {} bytes", bytes.len()));
        }

        let version = bytes[0];
        let nonce = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        let origin_domain = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);

        let mut sender = [0u8; 32];
        sender.copy_from_slice(&bytes[9..41]);

        let destination_domain = u32::from_be_bytes([bytes[41], bytes[42], bytes[43], bytes[44]]);

        let mut recipient = [0u8; 32];
        recipient.copy_from_slice(&bytes[45..77]);

        let body = bytes[77..].to_vec();

        Ok(Self {
            version,
            nonce,
            origin_domain,
            sender,
            destination_domain,
            recipient,
            body,
        })
    }
}

/// Token transfer details parsed from message body
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenTransfer {
    pub recipient: String,
    pub amount: u128,
    pub metadata: Vec<u8>,
}

impl TokenTransfer {
    pub fn new(recipient: String, amount: u128, metadata: Vec<u8>) -> Self {
        Self {
            recipient,
            amount,
            metadata,
        }
    }
}

/// Deposit result from Kaspa blockchain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositResult {
    pub tx_hash: String,
    pub utxo_index: usize,
    pub amount: u128,
    pub accepting_block_hash: String,
    pub containing_block_hash: String,
    pub message: BridgeMessage,
    pub confirmation_count: u64,
}

/// Withdrawal request to Kaspa blockchain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    pub recipient: String,
    pub amount: u128,
    pub message_id: [u8; 32],
    pub message: BridgeMessage,
}

/// Withdrawal status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithdrawalStatus {
    Pending,
    Processed { tx_hash: String },
    Failed { reason: String },
}

/// Transaction hash type
pub type TxHash = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_message_serialization() {
        let msg = BridgeMessage::new(1, 100, 1, [1u8; 32], 2, [2u8; 32], vec![1, 2, 3, 4]);

        let bytes = msg.to_vec();
        let parsed = BridgeMessage::from_bytes(&bytes).unwrap();

        assert_eq!(msg, parsed);
    }

    #[test]
    fn test_bridge_message_id() {
        let msg = BridgeMessage::new(1, 100, 1, [1u8; 32], 2, [2u8; 32], vec![1, 2, 3, 4]);

        let id1 = msg.id();
        let id2 = msg.id();

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_bridge_message_from_short_bytes_fails() {
        let short_bytes = vec![0u8; 50];
        let result = BridgeMessage::from_bytes(&short_bytes);
        assert!(result.is_err());
    }
}
