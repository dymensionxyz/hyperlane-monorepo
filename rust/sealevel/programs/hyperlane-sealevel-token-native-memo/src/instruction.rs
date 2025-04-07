//! Instructions for the program.

use account_utils::{DiscriminatorData, PROGRAM_INSTRUCTION_DISCRIMINATOR};
use borsh::{BorshDeserialize, BorshSerialize};
use hyperlane_sealevel_token_lib::instruction::TransferRemote;

use crate::hyperlane_token_native_collateral_pda_seeds;

use hyperlane_sealevel_token_lib::instruction::{init_instruction as lib_init_instruction, Init};


use solana_program::{
    instruction::{AccountMeta, Instruction as SolanaInstruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};


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


/// Gets an instruction to initialize the program.
pub fn init_instruction(
    program_id: Pubkey,
    payer: Pubkey,
    init: Init,
) -> Result<SolanaInstruction, ProgramError> {
    let mut instruction = lib_init_instruction(program_id, payer, init)?;

    // Add additional account metas:
    // 0. `[writable]` The native collateral PDA account.

    let (native_collateral_key, _native_collatera_bump) =
        Pubkey::find_program_address(hyperlane_token_native_collateral_pda_seeds!(), &program_id);

    instruction
        .accounts
        .append(&mut vec![AccountMeta::new(native_collateral_key, false)]);

    Ok(instruction)
}
