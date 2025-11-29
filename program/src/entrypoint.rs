//! Program entrypoint definitions

#![cfg(not(feature = "no-entrypoint"))]

use crate::{error::AmmError, instruction::AmmInstruction, processor::Processor};
use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult,
    program_error::PrintProgramError, pubkey::Pubkey,
};

//entrypoint!(process_instruction);

fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    if let Err(error) = Processor::process(program_id, accounts, instruction_data) {
        // catch the error so we can print it
        error.print::<AmmError>();
        return Err(error);
    }
    Ok(())
}
pub const SWAP_V2_IX_ACCOUNTS: usize = 8;
pub const SWAP_BASE_IN_V2_DISC: u8 = 16;

#[inline(always)]
unsafe fn p_entrypoint(input: *mut u8) -> Option<u64> {
    const UNINIT: core::mem::MaybeUninit<pinocchio::account_info::AccountInfo> =
        core::mem::MaybeUninit::<pinocchio::account_info::AccountInfo>::uninit();
    // Create an array of uninitialized account infos.
    let mut accounts = [UNINIT; SWAP_V2_IX_ACCOUNTS];

    let (program_id, count, instruction_data) =
        pinocchio::entrypoint::deserialize::<SWAP_V2_IX_ACCOUNTS>(input, &mut accounts);

    let accounts = core::slice::from_raw_parts(accounts.as_ptr() as _, count);
    let result = if instruction_data[0] == SWAP_BASE_IN_V2_DISC {
        Some(Processor::p_process_swap_base_in_v2(
            &program_id,
            accounts,
            &crate::instruction::SwapInstructionBaseIn {
                amount_in: core::ptr::read_unaligned(instruction_data.as_ptr().add(1) as *const u64),
                minimum_amount_out: core::ptr::read_unaligned(
                    instruction_data.as_ptr().add(9) as *const u64
                ),
            },
        ))
    } else {
        None
    };

    result.map(|value| match value {
        Ok(()) => solana_program::entrypoint::SUCCESS,
        Err(error) => solana_program::program_error::ProgramError::from(error).into(),
    })
}
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    match p_entrypoint(input) {
        Some(result) => result,
        None => {
            let (program_id, accounts, instruction_data) =
                unsafe { solana_program::entrypoint::deserialize(input) };

            match process_instruction(program_id, &accounts, instruction_data) {
                Ok(()) => solana_program::entrypoint::SUCCESS,
                Err(error) => error.into(),
            }
        }
    }
}
//solana_program::custom_heap_default!();
//solana_program::custom_panic_default!();
