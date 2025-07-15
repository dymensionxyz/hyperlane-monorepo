use hyperlane_core::H256;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::TransactionOutpoint as HubTransactionOutpoint;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::hashing::sighash_type::{
    SigHashType, SIG_HASH_ALL, SIG_HASH_ANY_ONE_CAN_PAY,
};
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutpoint};
use kaspa_hashes::Hash as KaspaHash;
use kaspa_txscript::pay_to_address_script;
use std::collections::HashSet;
use std::hash::Hash;

pub fn get_recipient_address(recipient: H256, prefix: Prefix) -> Address {
    Address::new(
        prefix,
        Version::PubKey, // should always be PubKey
        recipient.as_bytes(),
    )
}

pub fn get_recipient_script_pubkey(recipient: H256, prefix: Prefix) -> ScriptPublicKey {
    ScriptPublicKey::from(pay_to_address_script(&get_recipient_address(
        recipient, prefix,
    )))
}

pub fn get_recipient_script_pubkey_address(address: &Address) -> ScriptPublicKey {
    ScriptPublicKey::from(pay_to_address_script(address))
}

pub fn input_sighash_type() -> SigHashType {
    SigHashType::from_u8(SIG_HASH_ALL.to_u8() | SIG_HASH_ANY_ONE_CAN_PAY.to_u8()).unwrap()
}

pub fn check_sighash_type(t: SigHashType) -> bool {
    t.is_sighash_all() && t.is_sighash_anyone_can_pay()
}

pub fn hub_outpoint_to_kaspa_outpoint(o: &HubTransactionOutpoint) -> eyre::Result<TransactionOutpoint> {
    Ok(TransactionOutpoint {
        transaction_id: KaspaHash::from_bytes(
            o.transaction_id
                .as_slice()
                .try_into()
                .map_err(|e| eyre::eyre!("Invalid outpoint tx ID: {}", e))?,
        ),
        index: o.index,
    })
}

pub fn kaspa_outpoint_to_hub_outpoint(o: &TransactionOutpoint) -> HubTransactionOutpoint {
    HubTransactionOutpoint {
        transaction_id: o.transaction_id.as_bytes().to_vec(),
        index: o.index,
    }
}

/// Find the first duplicate if any.
pub fn find_duplicate<T>(v: &[T]) -> Option<T>
where
    T: Eq + Hash + Clone,
{
    let mut seen = HashSet::new();
    v.iter().find(|&item| !seen.insert(item)).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_sighash_type() {
        assert!(check_sighash_type(input_sighash_type()));
    }
}
