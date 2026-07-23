//! Program state invoker

use solana_program::{account_info::AccountInfo, program_error::ProgramError};

pub struct Invokers {}

impl Invokers {
    /// Issue a associated_spl_token `create_associated_token_account` instruction
    pub fn create_ata_spl_token<'a>(
        associated_account: AccountInfo<'a>,
        funding_account: AccountInfo<'a>,
        wallet_account: AccountInfo<'a>,
        token_mint_account: AccountInfo<'a>,
        token_program_account: AccountInfo<'a>,
        ata_program_account: AccountInfo<'a>,
        system_program_account: AccountInfo<'a>,
    ) -> Result<(), ProgramError> {
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            funding_account.key,
            wallet_account.key,
            token_mint_account.key,
            token_program_account.key,
        );
        solana_program::program::invoke_signed(
            &ix,
            &[
                associated_account,
                funding_account,
                wallet_account,
                token_mint_account,
                token_program_account,
                ata_program_account,
                system_program_account,
            ],
            &[],
        )
    }
    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn<'a>(
        token_program: AccountInfo<'a>,
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        owner: AccountInfo<'a>,
        burn_amount: u64,
    ) -> Result<(), ProgramError> {
        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            owner.key,
            &[],
            burn_amount,
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[burn_account, mint, owner, token_program],
            &[],
        )
    }

    /// Close Account
    pub fn token_close_with_authority<'a>(
        token_program: AccountInfo<'a>,
        close_account: AccountInfo<'a>,
        destination_account: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::close_account(
            token_program.key,
            close_account.key,
            destination_account.key,
            authority.key,
            &[],
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[close_account, destination_account, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn_with_authority<'a>(
        token_program: AccountInfo<'a>,
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
        burn_amount: u64,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            authority.key,
            &[],
            burn_amount,
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[burn_account, mint, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `MintTo` instruction.
    pub fn token_mint_to<'a>(
        token_program: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[mint, destination, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        owner: AccountInfo<'a>,
        deposit_amount: u64,
    ) -> Result<(), ProgramError> {
        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            owner.key,
            &[],
            deposit_amount,
        )?;
        solana_program::program::invoke_signed(
            &ix,
            &[source, destination, owner, token_program],
            &[],
        )
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer_with_authority<'a>(
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;
        solana_program::program::invoke_signed(
            &ix,
            &[source, destination, authority, token_program],
            signers,
        )
    }

    pub fn token_set_authority<'a>(
        token_program: AccountInfo<'a>,
        account: AccountInfo<'a>, // mint or token account
        authority: AccountInfo<'a>,
        new_authority: AccountInfo<'a>,
        amm_seed: &[u8],
        authority_nonce: u8,
        authority_type: spl_token::instruction::AuthorityType,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[authority_nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::set_authority(
            token_program.key,
            account.key,
            Some(new_authority.key),
            authority_type,
            authority.key,
            &[],
        )?;
        solana_program::program::invoke_signed(&ix, &[account, authority, token_program], signers)
    }
}
