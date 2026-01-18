//! Program entrypoint definitions

#![cfg(not(feature = "no-entrypoint"))]

use crate::{error::AmmError, processor::Processor};
use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult,
    program_error::PrintProgramError, pubkey::Pubkey,
};

entrypoint!(process_instruction);



/// The process_instruction function is the main entry point for the Raydium AMM program.

/// It dispatches incoming instructions to the processor after performing basic telemetry.

/// 

/// Parameters:

/// - program_id: The public key of the Raydium AMM program.

/// - accounts: The list of accounts required for the instruction.

/// - instruction_data: The encoded instruction data.

fn process_instruction<'a>(

    program_id: &Pubkey,

    accounts: &'a [AccountInfo<'a>],

    instruction_data: &[u8],

) -> ProgramResult {

    // SOVEREIGN TELEMETRY: Enhanced Logging for Entrypoint

    solana_program::msg!("Raydium Entrypoint: ProgramID: {}, AccountCount: {}", program_id, accounts.len());



    if let Err(error) = Processor::process(program_id, accounts, instruction_data) {

        // SOVEREIGN LOGIC: Context-Specific Error Reporting

        solana_program::msg!("Raydium Error: Processor failed to execute instruction. Details below:");

        error.print::<AmmError>();

        return Err(error);

    }

    Ok(())

}


