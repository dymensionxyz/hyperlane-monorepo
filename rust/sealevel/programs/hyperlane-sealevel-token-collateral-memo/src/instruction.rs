//! Instructions for the program.

use account_utils::{DiscriminatorData, PROGRAM_INSTRUCTION_DISCRIMINATOR};
use borsh::{BorshDeserialize, BorshSerialize};
use hyperlane_sealevel_token_lib::instruction::TransferRemote;

/// Instruction data for transferring `amount_or_id` token to `recipient`
/// on `destination` domain, including a memo.
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct TransferRemoteMemo {
    /// Base transfer instruction.
    pub base: TransferRemote,
    /// Arbitrary metadata.
    pub memo: Vec<u8>,
}

/// Instructions specifically for this token program
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub enum Instruction {
    /// Transfer tokens to a remote recipient, including a memo.
    TransferRemoteMemo(TransferRemoteMemo),
}

impl DiscriminatorData for Instruction {
    const DISCRIMINATOR: [u8; Self::DISCRIMINATOR_LENGTH] = PROGRAM_INSTRUCTION_DISCRIMINATOR;
}
