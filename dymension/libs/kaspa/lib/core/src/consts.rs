use kaspa_consensus_core::hashing::sighash_type::{
    SigHashType, SIG_HASH_ALL, SIG_HASH_ANY_ONE_CAN_PAY, SIG_HASH_NONE,
};

pub const RELAYER_SIG_OP_COUNT: u8 = 1; // relayer UTXOs always expect one single signature

pub const KEY_MESSAGE_IDS: &str = "msg_ids";
