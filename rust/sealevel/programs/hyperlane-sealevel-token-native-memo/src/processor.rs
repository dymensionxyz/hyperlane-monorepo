//! Program processor.

use crate::instruction::{Instruction, TransferRemoteMemo};
use account_utils::DiscriminatorDecode;
use hyperlane_sealevel_token_lib::{
    instruction::Instruction as TokenIxn, processor::HyperlaneSealevelToken,
};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, msg, pubkey::Pubkey};

use hyperlane_sealevel_message_recipient_interface::MessageRecipientInstruction;
use hyperlane_sealevel_token_native::plugin::NativePlugin;
use hyperlane_sealevel_token_native::processor::process_instruction as process_native_instruction;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Processes an instruction.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // First, check if the instruction has a discriminant relating to
    // the message recipient interface.
    if MessageRecipientInstruction::decode(instruction_data).is_ok() {
        return process_native_instruction(program_id, accounts, instruction_data);
    }
    if TokenIxn::decode(instruction_data).is_ok() {
        return process_native_instruction(program_id, accounts, instruction_data);
    }

    match Instruction::decode(instruction_data)? {
        Instruction::TransferRemoteMemo(xfer) => transfer_remote_memo(program_id, accounts, xfer),
    }
    .map_err(|err| {
        msg!("{}", err);
        err
    })
}

fn transfer_remote_memo(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    transfer: TransferRemoteMemo,
) -> ProgramResult {
    let base = transfer.base;
    let memo = transfer.memo;
    HyperlaneSealevelToken::<NativePlugin>::transfer_remote_memo(program_id, accounts, base, memo)
}
