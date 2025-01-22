//! Program entrypoint definitions
#![cfg(all(target_arch = "bpf", not(feature = "no-entrypoint")))]

use crate::{error::AmmError, processor::Processor};
use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult,
    msg, program_error::PrintProgramError, pubkey::Pubkey,
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
        // Log the error context
        msg!("Error occurred during instruction processing: {:?}", error);

        // Print the error and return it
        error.print::<AmmError>();
        return Err(error);
    }

    Ok(())
}
