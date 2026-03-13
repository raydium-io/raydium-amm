//! Program entrypoint definitions

#![cfg(not(feature = "no-entrypoint"))]

use crate::{error::AmmError, processor::Processor};
use num_traits::FromPrimitive;
use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey,
};

// Entrypoint macro that defines the program's entrypoint
entrypoint!(process_instruction);

/// Main program entrypoint function
///
/// # Arguments
/// * `program_id` - The public key of the program being invoked.
/// * `accounts` - A slice of account information for the program.
/// * `instruction_data` - Serialized input data for the instruction.
///
/// # Returns
/// * `ProgramResult` - Returns Ok(()) on success, or an error.
fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    // Log the program ID and the number of accounts received
    msg!("Program ID: {}", program_id);
    msg!("Number of accounts: {}", accounts.len());

    // Call the Processor to handle the program logic
    if let Err(error) = Processor::process(program_id, accounts, instruction_data) {
        // catch the error so we can print it
        if let ProgramError::Custom(custom_error) = error {
            if let Some(amm_error) = AmmError::from_u32(custom_error) {
                msg!("AMM error: {}", amm_error);
            } else {
                msg!("Unknown custom error: {}", custom_error);
            }
        } else {
            msg!("Program error: {:?}", error);
        }
        return Err(error);
    }
    Ok(())
}
