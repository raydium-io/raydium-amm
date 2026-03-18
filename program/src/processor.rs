//! Program state processor
#![allow(deprecated)]
use crate::{
    error::AmmError,
    instruction::{
        AmmInstruction, ConfigArgs, DepositInstruction, InitializeInstruction2,
        SetParamsInstruction, SwapInstructionBaseIn, SwapInstructionBaseOut, WithdrawInstruction,
    },
    invokers::Invokers,
    math::{
        Calculator, CheckedCeilDiv, InvariantPool, InvariantToken, RoundDirection, SwapDirection,
        U128, U256,
    },
    state::{
        AmmConfig, AmmInfo, AmmParams, AmmResetFlag, AmmState, AmmStatus, Loadable, TargetOrders,
        MAX_ORDER_LIMIT, TEN_THOUSAND,
    },
};

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    // log::sol_log_compute_units,
    program_error::ProgramError,
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

use super::log::*;
use arrform::{arrform, ArrForm};
use std::{convert::identity, mem::size_of};

pub mod srm_token {
    solana_program::declare_id!("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt");
}

pub mod msrm_token {
    solana_program::declare_id!("MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L");
}

#[cfg(feature = "testnet")]
pub mod config_feature {
    pub mod amm_owner {
        solana_program::declare_id!("75KWb5XcqPTgacQyNw9P5QU2HL3xpezEVcgsFCiJgTT");
    }
    pub mod openbook_program {
        solana_program::declare_id!("6ccSma8mmmmQXcSFpheSKrTnwsCe5pBuEpzDLFjrAsCF");
    }
    pub mod referrer_pc_wallet {
        solana_program::declare_id!("75KWb5XcqPTgacQyNw9P5QU2HL3xpezEVcgsFCiJgTT");
    }
    pub mod create_pool_fee_address {
        solana_program::declare_id!("3TRTX4dXUpp2eqxi3tvQDFYUV7SdDJjcPE3Y4mbtftaX");
    }
}
#[cfg(feature = "devnet")]
pub mod config_feature {
    pub mod amm_owner {
        solana_program::declare_id!("DRayqG9RXYi8WHgWEmRQGrUWRWbhjYWYkCRJDd6JBBak");
    }
    pub mod openbook_program {
        solana_program::declare_id!("EoTcMgcDRTJVZDMZWBoU6rhYHZfkNTVEAfz3uUJRcYGj");
    }
    pub mod referrer_pc_wallet {
        solana_program::declare_id!("4NpMfWThvJQsV9VLjUXXpn3tPv1zoQpib8wCBDc1EBzD");
    }
    pub mod create_pool_fee_address {
        solana_program::declare_id!("9y8ENuuZ3b19quffx9hQvRVygG5ky6snHfRvGpuSfeJy");
    }
}
#[cfg(not(any(feature = "testnet", feature = "devnet")))]
pub mod config_feature {
    pub mod amm_owner {
        solana_program::declare_id!("GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ");
    }
    pub mod openbook_program {
        solana_program::declare_id!("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX");
    }
    pub mod referrer_pc_wallet {
        solana_program::declare_id!("FCxGKqGSVeV1d3WsmAXt45A5iQdCS6kKCeJy3EUBigMG");
    }
    pub mod create_pool_fee_address {
        solana_program::declare_id!("7YttLkHDoNj9wyDur5pM1ejNaAvT9X4eqaYcHQqtj2G5");
    }
}

/// Suffix for amm authority seed
pub const AUTHORITY_AMM: &'static [u8] = b"amm authority";
/// Suffix for amm associated seed
pub const AMM_ASSOCIATED_SEED: &'static [u8] = b"amm_associated_seed";
/// Suffix for target associated seed
pub const TARGET_ASSOCIATED_SEED: &'static [u8] = b"target_associated_seed";
/// Suffix for amm open order associated seed
pub const OPEN_ORDER_ASSOCIATED_SEED: &'static [u8] = b"open_order_associated_seed";
/// Suffix for coin vault associated seed
pub const COIN_VAULT_ASSOCIATED_SEED: &'static [u8] = b"coin_vault_associated_seed";
/// Suffix for pc vault associated seed
pub const PC_VAULT_ASSOCIATED_SEED: &'static [u8] = b"pc_vault_associated_seed";
/// Suffix for lp mint associated seed
pub const LP_MINT_ASSOCIATED_SEED: &'static [u8] = b"lp_mint_associated_seed";
/// Amm config seed
pub const AMM_CONFIG_SEED: &'static [u8] = b"amm_config_account_seed";

pub fn get_associated_address_and_bump_seed(
    info_id: &Pubkey,
    market_address: &Pubkey,
    associated_seed: &[u8],
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &info_id.to_bytes(),
            &market_address.to_bytes(),
            &associated_seed,
        ],
        program_id,
    )
}

/// Program state handler.
pub struct Processor {}
impl Processor {
    /// Unpacks a spl_token `Account`.
    #[inline]
    pub fn unpack_token_account(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Account, AmmError> {
        if account_info.owner != token_program_id {
            Err(AmmError::InvalidSplTokenProgram)
        } else {
            spl_token::state::Account::unpack(&account_info.data.borrow())
                .map_err(|_| AmmError::ExpectedAccount)
        }
    }

    /// Unpacks a spl_token `Mint`.
    #[inline]
    pub fn unpack_mint(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Mint, AmmError> {
        if account_info.owner != token_program_id {
            Err(AmmError::InvalidSplTokenProgram)
        } else {
            spl_token::state::Mint::unpack(&account_info.data.borrow())
                .map_err(|_| AmmError::ExpectedMint)
        }
    }

    /// The Detailed calculation of pnl
    /// 1. calc last_k witch dose not take pnl: last_k = calc_pnl_x * calc_pnl_y;
    /// 2. calc current price: current_price = current_x / current_y;
    /// 3. calc x after take pnl: x_after_take_pnl = sqrt(last_k * current_price);
    /// 4. calc y after take pnl: y_after_take_pnl = x_after_take_pnl / current_price;
    ///                           y_after_take_pnl = x_after_take_pnl * current_y / current_x;
    /// 5. calc pnl_x & pnl_y:  pnl_x = current_x - x_after_take_pnl;
    ///                         pnl_y = current_y - y_after_take_pnl;
    pub fn calc_take_pnl(
        target: &TargetOrders,
        amm: &mut AmmInfo,
        total_pc_without_take_pnl: &mut u64,
        total_coin_without_take_pnl: &mut u64,
        x1: U256,
        y1: U256,
    ) -> Result<(u128, u128), ProgramError> {
        // calc pnl
        let mut delta_x: u128;
        let mut delta_y: u128;
        let calc_pc_amount = Calculator::restore_decimal(
            target.calc_pnl_x.into(),
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let calc_coin_amount = Calculator::restore_decimal(
            target.calc_pnl_y.into(),
            amm.coin_decimals,
            amm.sys_decimal_value,
        );
        let pool_pc_amount = U128::from(*total_pc_without_take_pnl);
        let pool_coin_amount = U128::from(*total_coin_without_take_pnl);
        if pool_pc_amount.checked_mul(pool_coin_amount).unwrap()
            >= (calc_pc_amount).checked_mul(calc_coin_amount).unwrap()
        {
            // last k is
            // let last_k: u128 = (target.calc_pnl_x as u128).checked_mul(target.calc_pnl_y as u128).unwrap();
            // current k is
            // let current_k: u128 = (x1 as u128).checked_mul(y1 as u128).unwrap();
            // current p is
            // let current_p: u128 = (x1 as u128).checked_div(y1 as u128).unwrap();
            let x2_power = Calculator::calc_x_power(
                target.calc_pnl_x.into(),
                target.calc_pnl_y.into(),
                x1,
                y1,
            );
            // let x2 = Calculator::sqrt(x2_power).unwrap();
            let x2 = x2_power.integer_sqrt();
            // msg!(arrform!(LOG_SIZE, "calc_take_pnl x2_power:{}, x2:{}", x2_power, x2).as_str());
            let y2 = x2.checked_mul(y1).unwrap().checked_div(x1).unwrap();
            // msg!(arrform!(LOG_SIZE, "calc_take_pnl y2:{}", y2).as_str());

            // transfer to token_coin_pnl and token_pc_pnl
            // (x1 -x2) * pnl / sys_decimal_value
            let diff_x = U128::from(x1.checked_sub(x2).unwrap().as_u128());
            let diff_y = U128::from(y1.checked_sub(y2).unwrap().as_u128());
            delta_x = diff_x
                .checked_mul(amm.fees.pnl_numerator.into())
                .unwrap()
                .checked_div(amm.fees.pnl_denominator.into())
                .unwrap()
                .as_u128();
            delta_y = diff_y
                .checked_mul(amm.fees.pnl_numerator.into())
                .unwrap()
                .checked_div(amm.fees.pnl_denominator.into())
                .unwrap()
                .as_u128();

            let diff_pc_pnl_amount =
                Calculator::restore_decimal(diff_x, amm.pc_decimals, amm.sys_decimal_value);
            let diff_coin_pnl_amount =
                Calculator::restore_decimal(diff_y, amm.coin_decimals, amm.sys_decimal_value);
            let pc_pnl_amount = diff_pc_pnl_amount
                .checked_mul(amm.fees.pnl_numerator.into())
                .unwrap()
                .checked_div(amm.fees.pnl_denominator.into())
                .unwrap()
                .as_u64();
            let coin_pnl_amount = diff_coin_pnl_amount
                .checked_mul(amm.fees.pnl_numerator.into())
                .unwrap()
                .checked_div(amm.fees.pnl_denominator.into())
                .unwrap()
                .as_u64();
            if pc_pnl_amount != 0 && coin_pnl_amount != 0 {
                // step2: save total_pnl_pc & total_pnl_coin
                amm.state_data.total_pnl_pc = amm
                    .state_data
                    .total_pnl_pc
                    .checked_add(diff_pc_pnl_amount.as_u64())
                    .unwrap();
                amm.state_data.total_pnl_coin = amm
                    .state_data
                    .total_pnl_coin
                    .checked_add(diff_coin_pnl_amount.as_u64())
                    .unwrap();
                amm.state_data.need_take_pnl_pc = amm
                    .state_data
                    .need_take_pnl_pc
                    .checked_add(pc_pnl_amount)
                    .unwrap();
                amm.state_data.need_take_pnl_coin = amm
                    .state_data
                    .need_take_pnl_coin
                    .checked_add(coin_pnl_amount)
                    .unwrap();

                // step3: update total_coin and total_pc without pnl
                *total_pc_without_take_pnl = (*total_pc_without_take_pnl)
                    .checked_sub(pc_pnl_amount)
                    .unwrap();
                *total_coin_without_take_pnl = (*total_coin_without_take_pnl)
                    .checked_sub(coin_pnl_amount)
                    .unwrap();
            } else {
                delta_x = 0;
                delta_y = 0;
            }
        } else {
            msg!(arrform!(
                LOG_SIZE,
                "calc_take_pnl error x:{}, y:{}, calc_pnl_x:{}, calc_pnl_y:{}",
                x1,
                y1,
                identity(target.calc_pnl_x),
                identity(target.calc_pnl_y)
            )
            .as_str());
            return Err(AmmError::CalcPnlError.into());
        }

        Ok((delta_x, delta_y))
    }

    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        program_id: &Pubkey,
        amm_seed: &[u8],
        nonce: u8,
    ) -> Result<Pubkey, AmmError> {
        Pubkey::create_program_address(&[amm_seed, &[nonce]], program_id)
            .map_err(|_| AmmError::InvalidProgramAddress.into())
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_amm_associated_spl_token<'a, 'b: 'a>(
        program_id: &Pubkey,
        spl_token_program_id: &Pubkey,
        market_account: &'a AccountInfo<'b>,
        associated_token_account: &'a AccountInfo<'b>,
        token_mint_account: &'a AccountInfo<'b>,
        user_wallet_account: &'a AccountInfo<'b>,
        system_program_account: &'a AccountInfo<'b>,
        rent_sysvar_account: &'a AccountInfo<'b>,
        spl_token_program_account: &'a AccountInfo<'b>,
        associated_owner_account: &'a AccountInfo<'b>,
        associated_seed: &[u8],
    ) -> ProgramResult {
        let (associated_token_address, bump_seed) = get_associated_address_and_bump_seed(
            program_id,
            &market_account.key,
            associated_seed,
            program_id,
        );
        if associated_token_address != *associated_token_account.key {
            msg!("Error: Associated token address does not match seed derivation");
            return Err(AmmError::ExpectedAccount.into());
        }
        if associated_token_account.owner == system_program_account.key {
            let associated_account_signer_seeds: &[&[_]] = &[
                &program_id.to_bytes(),
                &market_account.key.to_bytes(),
                associated_seed,
                &[bump_seed],
            ];
            let rent = &Rent::from_account_info(rent_sysvar_account)?;
            let required_lamports = rent
                .minimum_balance(spl_token::state::Account::LEN)
                .max(1)
                .saturating_sub(associated_token_account.lamports());
            if required_lamports > 0 {
                invoke(
                    &system_instruction::transfer(
                        user_wallet_account.key,
                        associated_token_account.key,
                        required_lamports,
                    ),
                    &[
                        user_wallet_account.clone(),
                        associated_token_account.clone(),
                        system_program_account.clone(),
                    ],
                )?;
            }
            invoke_signed(
                &system_instruction::allocate(
                    associated_token_account.key,
                    spl_token::state::Account::LEN as u64,
                ),
                &[
                    associated_token_account.clone(),
                    system_program_account.clone(),
                ],
                &[&associated_account_signer_seeds],
            )?;
            invoke_signed(
                &system_instruction::assign(associated_token_account.key, spl_token_program_id),
                &[
                    associated_token_account.clone(),
                    system_program_account.clone(),
                ],
                &[&associated_account_signer_seeds],
            )?;

            invoke(
                &spl_token::instruction::initialize_account(
                    spl_token_program_id,
                    associated_token_account.key,
                    token_mint_account.key,
                    associated_owner_account.key,
                )?,
                &[
                    associated_token_account.clone(),
                    token_mint_account.clone(),
                    associated_owner_account.clone(),
                    rent_sysvar_account.clone(),
                    spl_token_program_account.clone(),
                ],
            )?;
        } else {
            associated_token_address.log();
            return Err(AmmError::RepeatCreateAmm.into());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_amm_associated_spl_mint<'a, 'b: 'a>(
        program_id: &Pubkey,
        spl_token_program_id: &Pubkey,
        market_account: &'a AccountInfo<'b>,
        associated_token_account: &'a AccountInfo<'b>,
        user_wallet_account: &'a AccountInfo<'b>,
        system_program_account: &'a AccountInfo<'b>,
        rent_sysvar_account: &'a AccountInfo<'b>,
        spl_token_program_account: &'a AccountInfo<'b>,
        associated_owner_account: &'a AccountInfo<'b>,
        associated_seed: &[u8],
        mint_decimals: u8,
    ) -> ProgramResult {
        let (associated_token_address, bump_seed) = get_associated_address_and_bump_seed(
            program_id,
            &market_account.key,
            associated_seed,
            program_id,
        );
        if associated_token_address != *associated_token_account.key {
            msg!("Error: Associated mint address does not match seed derivation");
            return Err(AmmError::ExpectedMint.into());
        }
        if associated_token_account.owner == system_program_account.key {
            let associated_account_signer_seeds: &[&[_]] = &[
                &program_id.to_bytes(),
                &market_account.key.to_bytes(),
                associated_seed,
                &[bump_seed],
            ];
            let rent = &Rent::from_account_info(rent_sysvar_account)?;
            let required_lamports = rent
                .minimum_balance(spl_token::state::Mint::LEN)
                .max(1)
                .saturating_sub(associated_token_account.lamports());
            if required_lamports > 0 {
                invoke(
                    &system_instruction::transfer(
                        user_wallet_account.key,
                        associated_token_account.key,
                        required_lamports,
                    ),
                    &[
                        user_wallet_account.clone(),
                        associated_token_account.clone(),
                        system_program_account.clone(),
                    ],
                )?;
            }
            invoke_signed(
                &system_instruction::allocate(
                    associated_token_account.key,
                    spl_token::state::Mint::LEN as u64,
                ),
                &[
                    associated_token_account.clone(),
                    system_program_account.clone(),
                ],
                &[&associated_account_signer_seeds],
            )?;
            invoke_signed(
                &system_instruction::assign(associated_token_account.key, spl_token_program_id),
                &[
                    associated_token_account.clone(),
                    system_program_account.clone(),
                ],
                &[&associated_account_signer_seeds],
            )?;

            invoke(
                &spl_token::instruction::initialize_mint(
                    spl_token_program_id,
                    associated_token_account.key,
                    associated_owner_account.key,
                    None,
                    mint_decimals,
                )?,
                &[
                    associated_token_account.clone(),
                    associated_owner_account.clone(),
                    rent_sysvar_account.clone(),
                    spl_token_program_account.clone(),
                ],
            )?;
        } else {
            associated_token_address.log();
            return Err(AmmError::RepeatCreateAmm.into());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_amm_associated_account<'a, 'b: 'a>(
        program_id: &Pubkey,
        assign_to: &Pubkey,
        market_account: &'a AccountInfo<'b>,
        associated_token_account: &'a AccountInfo<'b>,
        user_wallet_account: &'a AccountInfo<'b>,
        system_program_account: &'a AccountInfo<'b>,
        rent_sysvar_account: &'a AccountInfo<'b>,
        associated_seed: &[u8],
        data_size: usize,
    ) -> ProgramResult {
        let (associated_token_address, bump_seed) = get_associated_address_and_bump_seed(
            &program_id,
            &market_account.key,
            associated_seed,
            program_id,
        );
        if associated_token_address != *associated_token_account.key {
            msg!("Error: Associated token address does not match seed derivation");
            return Err(AmmError::ExpectedAccount.into());
        }
        if associated_token_account.owner == system_program_account.key {
            let associated_account_signer_seeds: &[&[_]] = &[
                &program_id.to_bytes(),
                &market_account.key.to_bytes(),
                associated_seed,
                &[bump_seed],
            ];
            let rent = &Rent::from_account_info(rent_sysvar_account)?;
            let required_lamports = rent
                .minimum_balance(data_size)
                .max(1)
                .saturating_sub(associated_token_account.lamports());
            if required_lamports > 0 {
                invoke(
                    &system_instruction::transfer(
                        user_wallet_account.key,
                        associated_token_account.key,
                        required_lamports,
                    ),
                    &[
                        user_wallet_account.clone(),
                        associated_token_account.clone(),
                        system_program_account.clone(),
                    ],
                )?;
            }
            invoke_signed(
                &system_instruction::allocate(associated_token_account.key, data_size as u64),
                &[
                    associated_token_account.clone(),
                    system_program_account.clone(),
                ],
                &[&associated_account_signer_seeds],
            )?;
            invoke_signed(
                &system_instruction::assign(associated_token_account.key, assign_to),
                &[
                    associated_token_account.clone(),
                    system_program_account.clone(),
                ],
                &[&associated_account_signer_seeds],
            )?;
        } else {
            associated_token_address.log();
            return Err(AmmError::RepeatCreateAmm.into());
        }
        Ok(())
    }

    /// Processes an [Initialize](enum.Instruction.html).
    pub fn process_initialize2(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        init: InitializeInstruction2,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let token_program_info = next_account_info(account_info_iter)?;
        let ata_token_program_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;
        let rent_sysvar_info = next_account_info(account_info_iter)?;
        let amm_info = next_account_info(account_info_iter)?;
        let amm_authority_info = next_account_info(account_info_iter)?;
        // Won't use.
        // Can be any account.
        let _amm_open_orders_info = next_account_info(account_info_iter)?;
        let amm_lp_mint_info = next_account_info(account_info_iter)?;
        let amm_coin_mint_info = next_account_info(account_info_iter)?;
        let amm_pc_mint_info = next_account_info(account_info_iter)?;
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;
        let amm_target_orders_info = next_account_info(account_info_iter)?;
        let amm_config_info = next_account_info(account_info_iter)?;
        let create_fee_destination_info = next_account_info(account_info_iter)?;
        // Won't use.
        // Can be any account.
        let _market_program_info = next_account_info(account_info_iter)?;
        // Just a seed for AMM account.
        // Can be any account.
        let market_info = next_account_info(account_info_iter)?;

        let user_wallet_info = next_account_info(account_info_iter)?;
        let user_token_coin_info = next_account_info(account_info_iter)?;
        let user_token_pc_info = next_account_info(account_info_iter)?;
        let user_token_lp_info = next_account_info(account_info_iter)?;

        let (pda, _) = Pubkey::find_program_address(&[&AMM_CONFIG_SEED], program_id);
        if pda != *amm_config_info.key || amm_config_info.owner != program_id {
            return Err(AmmError::InvalidConfigAccount.into());
        }

        if *amm_coin_mint_info.key == *amm_pc_mint_info.key {
            return Err(AmmError::InvalidCoinMint.into());
        }

        msg!(arrform!(LOG_SIZE, "initialize2: {:?}", init).as_str());
        if !user_wallet_info.is_signer {
            return Err(AmmError::InvalidSignAccount.into());
        }
        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );
        let spl_token_program_id = token_program_info.key;
        check_assert_eq!(
            *ata_token_program_info.key,
            spl_associated_token_account::id(),
            "spl_associated_token_account",
            AmmError::InvalidSplTokenProgram
        );
        check_assert_eq!(
            *system_program_info.key,
            solana_program::system_program::id(),
            "sys_program",
            AmmError::InvalidSysProgramAddress
        );
        let (expect_amm_authority, expect_nonce) =
            Pubkey::find_program_address(&[&AUTHORITY_AMM], program_id);
        if *amm_authority_info.key != expect_amm_authority || init.nonce != expect_nonce {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        if *create_fee_destination_info.key != config_feature::create_pool_fee_address::id() {
            return Err(AmmError::InvalidFee.into());
        }
        let amm_config = AmmConfig::load_checked(&amm_config_info, program_id)?;
        // Charge the fee to create a pool
        if amm_config.create_pool_fee != 0 {
            invoke(
                &system_instruction::transfer(
                    user_wallet_info.key,
                    create_fee_destination_info.key,
                    amm_config.create_pool_fee,
                ),
                &[
                    user_wallet_info.clone(),
                    create_fee_destination_info.clone(),
                    system_program_info.clone(),
                ],
            )?;
            invoke(
                &spl_token::instruction::sync_native(
                    token_program_info.key,
                    create_fee_destination_info.key,
                )?,
                &[
                    token_program_info.clone(),
                    create_fee_destination_info.clone(),
                ],
            )?;
        }

        // unpack and check coin_mint
        let coin_mint = Self::unpack_mint(&amm_coin_mint_info, spl_token_program_id)?;
        // unpack and check pc_mint
        let pc_mint = Self::unpack_mint(&amm_pc_mint_info, spl_token_program_id)?;

        // create target_order account
        Self::generate_amm_associated_account(
            program_id,
            program_id,
            market_info,
            amm_target_orders_info,
            user_wallet_info,
            system_program_info,
            rent_sysvar_info,
            TARGET_ASSOCIATED_SEED,
            size_of::<TargetOrders>(),
        )?;

        // create lp mint account
        let lp_decimals = coin_mint.decimals;
        Self::generate_amm_associated_spl_mint(
            program_id,
            spl_token_program_id,
            market_info,
            amm_lp_mint_info,
            user_wallet_info,
            system_program_info,
            rent_sysvar_info,
            token_program_info,
            amm_authority_info,
            LP_MINT_ASSOCIATED_SEED,
            lp_decimals,
        )?;
        // create coin vault account
        Self::generate_amm_associated_spl_token(
            program_id,
            spl_token_program_id,
            market_info,
            amm_coin_vault_info,
            amm_coin_mint_info,
            user_wallet_info,
            system_program_info,
            rent_sysvar_info,
            token_program_info,
            amm_authority_info,
            COIN_VAULT_ASSOCIATED_SEED,
        )?;
        // create pc vault account
        Self::generate_amm_associated_spl_token(
            program_id,
            spl_token_program_id,
            market_info,
            amm_pc_vault_info,
            amm_pc_mint_info,
            user_wallet_info,
            system_program_info,
            rent_sysvar_info,
            token_program_info,
            amm_authority_info,
            PC_VAULT_ASSOCIATED_SEED,
        )?;
        // create amm account
        Self::generate_amm_associated_account(
            program_id,
            program_id,
            market_info,
            amm_info,
            user_wallet_info,
            system_program_info,
            rent_sysvar_info,
            AMM_ASSOCIATED_SEED,
            size_of::<AmmInfo>(),
        )?;

        // create user ata lp token
        Invokers::create_ata_spl_token(
            user_token_lp_info.clone(),
            user_wallet_info.clone(),
            user_wallet_info.clone(),
            amm_lp_mint_info.clone(),
            token_program_info.clone(),
            ata_token_program_info.clone(),
            system_program_info.clone(),
        )?;

        // transfer user tokens to vault
        Invokers::token_transfer(
            token_program_info.clone(),
            user_token_coin_info.clone(),
            amm_coin_vault_info.clone(),
            user_wallet_info.clone(),
            init.init_coin_amount,
        )?;
        Invokers::token_transfer(
            token_program_info.clone(),
            user_token_pc_info.clone(),
            amm_pc_vault_info.clone(),
            user_wallet_info.clone(),
            init.init_pc_amount,
        )?;

        // load AmmInfo
        let mut amm = AmmInfo::load_mut(&amm_info)?;
        if amm.status != AmmStatus::Uninitialized.into_u64() {
            return Err(AmmError::AlreadyInUse.into());
        }

        // unpack and check token_coin
        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        check_assert_eq!(
            amm_coin_vault.owner,
            *amm_authority_info.key,
            "coin_vault_owner",
            AmmError::InvalidOwner
        );
        if amm_coin_vault.amount == 0 {
            return Err(AmmError::InvalidSupply.into());
        }
        if amm_coin_vault.delegate.is_some() {
            return Err(AmmError::InvalidDelegate.into());
        }
        if amm_coin_vault.close_authority.is_some() {
            return Err(AmmError::InvalidCloseAuthority.into());
        }
        check_assert_eq!(
            *amm_coin_mint_info.key,
            amm_coin_vault.mint,
            "coin_mint",
            AmmError::InvalidCoinMint
        );
        // unpack and check token_pc
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;
        check_assert_eq!(
            amm_pc_vault.owner,
            *amm_authority_info.key,
            "pc_vault_owner",
            AmmError::InvalidOwner
        );
        if amm_pc_vault.amount == 0 {
            return Err(AmmError::InvalidSupply.into());
        }
        if amm_pc_vault.delegate.is_some() {
            return Err(AmmError::InvalidDelegate.into());
        }
        if amm_pc_vault.close_authority.is_some() {
            return Err(AmmError::InvalidCloseAuthority.into());
        }
        check_assert_eq!(
            *amm_pc_mint_info.key,
            amm_pc_vault.mint,
            "pc_mint",
            AmmError::InvalidPCMint
        );

        let lp_mint = Self::unpack_mint(&amm_lp_mint_info, spl_token_program_id)?;
        if lp_mint.supply != 0 {
            return Err(AmmError::InvalidSupply.into());
        }
        if COption::Some(*amm_authority_info.key) != lp_mint.mint_authority {
            return Err(AmmError::InvalidOwner.into());
        }
        if lp_mint.freeze_authority.is_some() {
            return Err(AmmError::InvalidFreezeAuthority.into());
        }

        let liquidity = Calculator::to_u64(
            U128::from(amm_pc_vault.amount)
                .checked_mul(amm_coin_vault.amount.into())
                .unwrap()
                .integer_sqrt()
                .as_u128(),
        )?;
        let user_lp_amount = liquidity
            .checked_sub((10u64).checked_pow(lp_mint.decimals.into()).unwrap())
            .ok_or(AmmError::InitLpAmountTooLess)?;

        // liquidity is measured in terms of token_a's value since both sides of
        // the pool are equal
        Invokers::token_mint_to(
            token_program_info.clone(),
            amm_lp_mint_info.clone(),
            user_token_lp_info.clone(),
            amm_authority_info.clone(),
            AUTHORITY_AMM,
            init.nonce,
            user_lp_amount,
        )?;

        amm.initialize(
            init.nonce,
            init.open_time,
            coin_mint.decimals,
            pc_mint.decimals,
            0,
            0,
        )?;
        encode_ray_log(InitLog {
            log_type: LogType::Init.into_u8(),
            time: init.open_time,
            pc_decimals: amm.pc_decimals as u8,
            coin_decimals: amm.coin_decimals as u8,
            pc_lot_size: 0,
            coin_lot_size: 0,
            pc_amount: amm_pc_vault.amount,
            coin_amount: amm_coin_vault.amount,
            market: *market_info.key,
        });
        let x = Calculator::normalize_decimal_v2(
            amm_pc_vault.amount,
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let y = Calculator::normalize_decimal_v2(
            amm_coin_vault.amount,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );
        // check and init target orders account
        if amm_target_orders_info.owner != program_id {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        let mut target_order = TargetOrders::load_mut(amm_target_orders_info)?;
        target_order.check_init(x.as_u128(), y.as_u128(), amm_info.key)?;

        amm.coin_vault = *amm_coin_vault_info.key;
        amm.pc_vault = *amm_pc_vault_info.key;
        amm.coin_vault_mint = *amm_coin_mint_info.key;
        amm.pc_vault_mint = *amm_pc_mint_info.key;
        amm.lp_mint = *amm_lp_mint_info.key;
        amm.open_orders = Pubkey::default();
        amm.market = *market_info.key;
        amm.market_program = Pubkey::default();
        amm.target_orders = *amm_target_orders_info.key;
        amm.amm_owner = config_feature::amm_owner::ID;
        amm.lp_amount = liquidity;
        amm.status = if init.open_time > (Clock::get()?.unix_timestamp as u64) {
            AmmStatus::WaitingTrade.into_u64()
        } else {
            AmmStatus::SwapOnly.into_u64()
        };
        amm.reset_flag = AmmResetFlag::ResetYes.into_u64();

        Ok(())
    }

    /// Processes an [Deposit](enum.Instruction.html).
    pub fn process_deposit(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        deposit: DepositInstruction,
    ) -> ProgramResult {
        let input_account_len = accounts.len();
        let (
            token_program_info,
            amm_info,
            amm_authority_info,
            amm_target_orders_info,
            amm_lp_mint_info,
            amm_coin_vault_info,
            amm_pc_vault_info,
            user_source_coin_info,
            user_source_pc_info,
            user_dest_lp_info,
            source_owner_info,
        ) = if input_account_len == 11 {
            // Recommended use due to openbook has not supported.
            let account_info_iter = &mut accounts.iter();
            let token_program_info = next_account_info(account_info_iter)?;

            let amm_info = next_account_info(account_info_iter)?;
            let amm_authority_info = next_account_info(account_info_iter)?;
            let amm_target_orders_info = next_account_info(account_info_iter)?;
            let amm_lp_mint_info = next_account_info(account_info_iter)?;
            let amm_coin_vault_info = next_account_info(account_info_iter)?;
            let amm_pc_vault_info = next_account_info(account_info_iter)?;

            let user_source_coin_info = next_account_info(account_info_iter)?;
            let user_source_pc_info = next_account_info(account_info_iter)?;
            let user_dest_lp_info = next_account_info(account_info_iter)?;
            let source_owner_info = next_account_info(account_info_iter)?;

            (
                token_program_info,
                amm_info,
                amm_authority_info,
                amm_target_orders_info,
                amm_lp_mint_info,
                amm_coin_vault_info,
                amm_pc_vault_info,
                user_source_coin_info,
                user_source_pc_info,
                user_dest_lp_info,
                source_owner_info,
            )
        } else {
            const ACCOUNT_LEN: usize = 14;
            if input_account_len != ACCOUNT_LEN && input_account_len != ACCOUNT_LEN + 1 {
                return Err(AmmError::WrongAccountsNumber.into());
            }
            let account_info_iter = &mut accounts.iter();
            let token_program_info = next_account_info(account_info_iter)?;

            let amm_info = next_account_info(account_info_iter)?;
            let amm_authority_info = next_account_info(account_info_iter)?;
            let _amm_open_orders_info = next_account_info(account_info_iter)?;
            let amm_target_orders_info = next_account_info(account_info_iter)?;
            let amm_lp_mint_info = next_account_info(account_info_iter)?;
            let amm_coin_vault_info = next_account_info(account_info_iter)?;
            let amm_pc_vault_info = next_account_info(account_info_iter)?;

            let _market_info = next_account_info(account_info_iter)?;

            let user_source_coin_info = next_account_info(account_info_iter)?;
            let user_source_pc_info = next_account_info(account_info_iter)?;
            let user_dest_lp_info = next_account_info(account_info_iter)?;
            let source_owner_info = next_account_info(account_info_iter)?;
            let _market_event_queue_info = next_account_info(account_info_iter)?;

            (
                token_program_info,
                amm_info,
                amm_authority_info,
                amm_target_orders_info,
                amm_lp_mint_info,
                amm_coin_vault_info,
                amm_pc_vault_info,
                user_source_coin_info,
                user_source_pc_info,
                user_dest_lp_info,
                source_owner_info,
            )
        };

        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        if deposit.max_coin_amount == 0 || deposit.max_pc_amount == 0 {
            encode_ray_log(DepositLog {
                log_type: LogType::Deposit.into_u8(),
                max_coin: deposit.max_coin_amount,
                max_pc: deposit.max_pc_amount,
                base: deposit.base_side,
                pool_coin: 0,
                pool_pc: 0,
                pool_lp: 0,
                calc_pnl_x: 0,
                calc_pnl_y: 0,
                deduct_coin: 0,
                deduct_pc: 0,
                mint_lp: 0,
            });
            return Err(AmmError::InvalidInput.into());
        }
        if !source_owner_info.is_signer {
            return Err(AmmError::InvalidSignAccount.into());
        }

        if !AmmStatus::from_u64(amm.status).deposit_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        if *amm_authority_info.key
            != Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?
        {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );
        let spl_token_program_id = token_program_info.key;
        // token_coin must be amm.coin_vault or token_source_coin must not be amm.coin_vault
        if *amm_coin_vault_info.key != amm.coin_vault
            || *user_source_coin_info.key == amm.coin_vault
        {
            return Err(AmmError::InvalidCoinVault.into());
        }
        // token_pc must be amm.pc_vault or token_source_pc must not be amm.pc_vault
        if *amm_pc_vault_info.key != amm.pc_vault || *user_source_pc_info.key == amm.pc_vault {
            return Err(AmmError::InvalidPCVault.into());
        }
        check_assert_eq!(
            *amm_lp_mint_info.key,
            amm.lp_mint,
            "lp_mint",
            AmmError::InvalidPoolMint
        );
        check_assert_eq!(
            *amm_target_orders_info.key,
            amm.target_orders,
            "target_orders",
            AmmError::InvalidTargetOrders
        );
        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;
        let user_source_coin =
            Self::unpack_token_account(&user_source_coin_info, spl_token_program_id)?;
        let user_source_pc =
            Self::unpack_token_account(&user_source_pc_info, spl_token_program_id)?;
        let mut target_orders =
            TargetOrders::load_mut_checked(&amm_target_orders_info, program_id, amm_info.key)?;
        // calc the remaining total_pc & total_coin
        let (mut total_pc_without_take_pnl, mut total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;

        let x1 = Calculator::normalize_decimal_v2(
            total_pc_without_take_pnl,
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let y1 = Calculator::normalize_decimal_v2(
            total_coin_without_take_pnl,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );
        // calc and update pnl
        let (delta_x, delta_y) = Self::calc_take_pnl(
            &target_orders,
            &mut amm,
            &mut total_pc_without_take_pnl,
            &mut total_coin_without_take_pnl,
            x1.as_u128().into(),
            y1.as_u128().into(),
        )?;
        let invariant = InvariantToken {
            token_coin: total_coin_without_take_pnl,
            token_pc: total_pc_without_take_pnl,
        };

        // let lp_mint  = Self::unpack_mint(&lp_mint_info, spl_token_program_id)?;
        if amm.lp_amount == 0 {
            encode_ray_log(DepositLog {
                log_type: LogType::Deposit.into_u8(),
                max_coin: deposit.max_coin_amount,
                max_pc: deposit.max_pc_amount,
                base: deposit.base_side,
                pool_coin: total_coin_without_take_pnl,
                pool_pc: total_pc_without_take_pnl,
                pool_lp: amm.lp_amount,
                calc_pnl_x: target_orders.calc_pnl_x,
                calc_pnl_y: target_orders.calc_pnl_y,
                deduct_coin: 0,
                deduct_pc: 0,
                mint_lp: 0,
            });
            return Err(AmmError::NotAllowZeroLP.into());
        }
        let deduct_pc_amount;
        let deduct_coin_amount;
        let mint_lp_amount;
        if deposit.base_side == 0 {
            // base coin
            deduct_pc_amount = invariant
                .exchange_coin_to_pc(deposit.max_coin_amount, RoundDirection::Ceiling)
                .ok_or(AmmError::CalculationExRateFailure)?;
            deduct_coin_amount = deposit.max_coin_amount;
            if deduct_pc_amount > deposit.max_pc_amount {
                encode_ray_log(DepositLog {
                    log_type: LogType::Deposit.into_u8(),
                    max_coin: deposit.max_coin_amount,
                    max_pc: deposit.max_pc_amount,
                    base: deposit.base_side,
                    pool_coin: total_coin_without_take_pnl,
                    pool_pc: total_pc_without_take_pnl,
                    pool_lp: amm.lp_amount,
                    calc_pnl_x: target_orders.calc_pnl_x,
                    calc_pnl_y: target_orders.calc_pnl_y,
                    deduct_coin: deduct_coin_amount,
                    deduct_pc: deduct_pc_amount,
                    mint_lp: 0,
                });
                return Err(AmmError::ExceededSlippage.into());
            }
            // base coin, check other_amount_min if need
            if deposit.other_amount_min.is_some() {
                if deduct_pc_amount < deposit.other_amount_min.unwrap() {
                    encode_ray_log(DepositLog {
                        log_type: LogType::Deposit.into_u8(),
                        max_coin: deposit.max_coin_amount,
                        max_pc: deposit.max_pc_amount,
                        base: deposit.base_side,
                        pool_coin: total_coin_without_take_pnl,
                        pool_pc: total_pc_without_take_pnl,
                        pool_lp: amm.lp_amount,
                        calc_pnl_x: target_orders.calc_pnl_x,
                        calc_pnl_y: target_orders.calc_pnl_y,
                        deduct_coin: deduct_coin_amount,
                        deduct_pc: deduct_pc_amount,
                        mint_lp: 0,
                    });
                    return Err(AmmError::ExceededSlippage.into());
                }
            }
            // coin_amount/ (total_coin_amount + coin_amount)  = output / (lp_mint.supply + output) =>  output = coin_amount / total_coin_amount * lp_mint.supply
            let invariant_coin = InvariantPool {
                token_input: deduct_coin_amount,
                token_total: total_coin_without_take_pnl,
            };
            mint_lp_amount = invariant_coin
                .exchange_token_to_pool(amm.lp_amount, RoundDirection::Floor)
                .ok_or(AmmError::CalculationExRateFailure)?;
        } else {
            // base pc
            deduct_coin_amount = invariant
                .exchange_pc_to_coin(deposit.max_pc_amount, RoundDirection::Ceiling)
                .ok_or(AmmError::CalculationExRateFailure)?;
            deduct_pc_amount = deposit.max_pc_amount;
            if deduct_coin_amount > deposit.max_coin_amount {
                encode_ray_log(DepositLog {
                    log_type: LogType::Deposit.into_u8(),
                    max_coin: deposit.max_coin_amount,
                    max_pc: deposit.max_pc_amount,
                    base: deposit.base_side,
                    pool_coin: total_coin_without_take_pnl,
                    pool_pc: total_pc_without_take_pnl,
                    pool_lp: amm.lp_amount,
                    calc_pnl_x: target_orders.calc_pnl_x,
                    calc_pnl_y: target_orders.calc_pnl_y,
                    deduct_coin: deduct_coin_amount,
                    deduct_pc: deduct_pc_amount,
                    mint_lp: 0,
                });
                return Err(AmmError::ExceededSlippage.into());
            }
            // base pc, check other_amount_min if need
            if deposit.other_amount_min.is_some() {
                if deduct_coin_amount < deposit.other_amount_min.unwrap() {
                    encode_ray_log(DepositLog {
                        log_type: LogType::Deposit.into_u8(),
                        max_coin: deposit.max_coin_amount,
                        max_pc: deposit.max_pc_amount,
                        base: deposit.base_side,
                        pool_coin: total_coin_without_take_pnl,
                        pool_pc: total_pc_without_take_pnl,
                        pool_lp: amm.lp_amount,
                        calc_pnl_x: target_orders.calc_pnl_x,
                        calc_pnl_y: target_orders.calc_pnl_y,
                        deduct_coin: deduct_coin_amount,
                        deduct_pc: deduct_pc_amount,
                        mint_lp: 0,
                    });
                    return Err(AmmError::ExceededSlippage.into());
                }
            }

            let invariant_pc = InvariantPool {
                token_input: deduct_pc_amount,
                token_total: total_pc_without_take_pnl,
            };
            // pc_amount/ (total_pc_amount + pc_amount)  = output / (lp_mint.supply + output) =>  output = pc_amount / total_pc_amount * lp_mint.supply
            mint_lp_amount = invariant_pc
                .exchange_token_to_pool(amm.lp_amount, RoundDirection::Floor)
                .ok_or(AmmError::CalculationExRateFailure)?;
        }
        encode_ray_log(DepositLog {
            log_type: LogType::Deposit.into_u8(),
            max_coin: deposit.max_coin_amount,
            max_pc: deposit.max_pc_amount,
            base: deposit.base_side,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            pool_lp: amm.lp_amount,
            calc_pnl_x: target_orders.calc_pnl_x,
            calc_pnl_y: target_orders.calc_pnl_y,
            deduct_coin: deduct_coin_amount,
            deduct_pc: deduct_pc_amount,
            mint_lp: mint_lp_amount,
        });

        if deduct_coin_amount > user_source_coin.amount || deduct_pc_amount > user_source_pc.amount
        {
            return Err(AmmError::InsufficientFunds.into());
        }
        if mint_lp_amount == 0 || deduct_coin_amount == 0 || deduct_pc_amount == 0 {
            return Err(AmmError::InvalidInput.into());
        }

        Invokers::token_transfer(
            token_program_info.clone(),
            user_source_coin_info.clone(),
            amm_coin_vault_info.clone(),
            source_owner_info.clone(),
            deduct_coin_amount,
        )?;
        Invokers::token_transfer(
            token_program_info.clone(),
            user_source_pc_info.clone(),
            amm_pc_vault_info.clone(),
            source_owner_info.clone(),
            deduct_pc_amount,
        )?;
        Invokers::token_mint_to(
            token_program_info.clone(),
            amm_lp_mint_info.clone(),
            user_dest_lp_info.clone(),
            amm_authority_info.clone(),
            AUTHORITY_AMM,
            amm.nonce as u8,
            mint_lp_amount,
        )?;
        amm.lp_amount = amm.lp_amount.checked_add(mint_lp_amount).unwrap();

        target_orders.calc_pnl_x = x1
            .checked_add(Calculator::normalize_decimal_v2(
                deduct_pc_amount,
                amm.pc_decimals,
                amm.sys_decimal_value,
            ))
            .unwrap()
            .checked_sub(U128::from(delta_x))
            .unwrap()
            .as_u128();
        target_orders.calc_pnl_y = y1
            .checked_add(Calculator::normalize_decimal_v2(
                deduct_coin_amount,
                amm.coin_decimals,
                amm.sys_decimal_value,
            ))
            .unwrap()
            .checked_sub(U128::from(delta_y))
            .unwrap()
            .as_u128();
        amm.recent_epoch = Clock::get()?.epoch;
        Ok(())
    }

    pub fn process_withdrawpnl(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let token_program_info = next_account_info(account_info_iter)?;

        let amm_info = next_account_info(account_info_iter)?;
        let amm_config_info = next_account_info(account_info_iter)?;
        let amm_authority_info = next_account_info(account_info_iter)?;
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;
        let user_pnl_coin_info = next_account_info(account_info_iter)?;
        let user_pnl_pc_info = next_account_info(account_info_iter)?;
        let pnl_owner_info = next_account_info(account_info_iter)?;
        let amm_target_orders_info = next_account_info(account_info_iter)?;

        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        if *amm_authority_info.key
            != Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?
        {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        if amm_info.owner != program_id {
            return Err(AmmError::InvalidOwner.into());
        }
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }

        let (pda, _) = Pubkey::find_program_address(&[&AMM_CONFIG_SEED], program_id);
        if pda != *amm_config_info.key || amm_config_info.owner != program_id {
            return Err(AmmError::InvalidConfigAccount.into());
        }
        let amm_config = AmmConfig::load_checked(&amm_config_info, program_id)?;

        if !pnl_owner_info.is_signer
            || (*pnl_owner_info.key != config_feature::amm_owner::ID
                && *pnl_owner_info.key != amm_config.pnl_owner)
        {
            return Err(AmmError::InvalidSignAccount.into());
        }
        // withdrawpnl in all status except Uninitialized
        if amm.status == AmmStatus::Uninitialized.into_u64() {
            msg!(&format!("withdrawpnl: status {}", identity(amm.status)));
            return Err(AmmError::InvalidStatus.into());
        }
        check_assert_eq!(
            *amm_coin_vault_info.key,
            amm.coin_vault,
            "coin_vault",
            AmmError::InvalidCoinVault
        );
        check_assert_eq!(
            *amm_pc_vault_info.key,
            amm.pc_vault,
            "pc_vault",
            AmmError::InvalidPCVault
        );
        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );

        let spl_token_program_id = token_program_info.key;

        check_assert_eq!(
            *amm_target_orders_info.key,
            amm.target_orders,
            "target_orders",
            AmmError::InvalidTargetOrders
        );
        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;
        let user_pnl_coin = Self::unpack_token_account(&user_pnl_coin_info, spl_token_program_id)?;
        let user_pnl_pc = Self::unpack_token_account(&user_pnl_pc_info, spl_token_program_id)?;
        let mut target_orders =
            TargetOrders::load_mut_checked(&amm_target_orders_info, program_id, amm_info.key)?;
        if amm_coin_vault.mint != amm.coin_vault_mint || user_pnl_coin.mint != amm.coin_vault_mint {
            return Err(AmmError::InvalidCoinMint.into());
        }
        if amm_pc_vault.mint != amm.pc_vault_mint || user_pnl_pc.mint != amm.pc_vault_mint {
            return Err(AmmError::InvalidPCMint.into());
        }

        // calc the remaining total_pc & total_coin
        let (mut total_pc_without_take_pnl, mut total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;

        msg!(arrform!(
            LOG_SIZE,
            "withdrawpnl need_take_coin:{}, need_take_pc:{}",
            identity(amm.state_data.need_take_pnl_coin),
            identity(amm.state_data.need_take_pnl_pc)
        )
        .as_str());

        let x1 = Calculator::normalize_decimal_v2(
            total_pc_without_take_pnl,
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let y1 = Calculator::normalize_decimal_v2(
            total_coin_without_take_pnl,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );
        msg!(arrform!(
            LOG_SIZE,
            "withdrawpnl total_pc:{}, total_pc:{}, x:{}, y:{}",
            total_pc_without_take_pnl,
            total_coin_without_take_pnl,
            x1,
            y1
        )
        .as_str());

        // calc and update pnl
        let (delta_x, delta_y) = Self::calc_take_pnl(
            &target_orders,
            &mut amm,
            &mut total_pc_without_take_pnl,
            &mut total_coin_without_take_pnl,
            x1.as_u128().into(),
            y1.as_u128().into(),
        )?;
        msg!(arrform!(LOG_SIZE, "withdrawpnl total_pc:{}, total_pc:{}, delta_x:{}, delta_y:{}, need_take_coin:{}, need_take_pc:{}",total_pc_without_take_pnl, total_coin_without_take_pnl, delta_x, delta_y, identity(amm.state_data.need_take_pnl_coin), identity(amm.state_data.need_take_pnl_pc)).as_str());

        if amm.state_data.need_take_pnl_coin <= amm_coin_vault.amount
            && amm.state_data.need_take_pnl_pc <= amm_pc_vault.amount
        {
            // coin & pc is enough, transfer directly
            Invokers::token_transfer_with_authority(
                token_program_info.clone(),
                amm_coin_vault_info.clone(),
                user_pnl_coin_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                amm.state_data.need_take_pnl_coin,
            )?;
            Invokers::token_transfer_with_authority(
                token_program_info.clone(),
                amm_pc_vault_info.clone(),
                user_pnl_pc_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                amm.state_data.need_take_pnl_pc,
            )?;
            // clear need take pnl
            amm.state_data.need_take_pnl_coin = 0u64;
            amm.state_data.need_take_pnl_pc = 0u64;
            // update target_orders.calc_pnl_x & target_orders.calc_pnl_y
            target_orders.calc_pnl_x = x1.checked_sub(U128::from(delta_x)).unwrap().as_u128();
            target_orders.calc_pnl_y = y1.checked_sub(U128::from(delta_y)).unwrap().as_u128();
        } else {
            // calc error
            return Err(AmmError::TakePnlError.into());
        }
        amm.recent_epoch = Clock::get()?.epoch;

        Ok(())
    }

    /// Processes an [Withdraw](enum.Instruction.html).
    pub fn process_withdraw(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        withdraw: WithdrawInstruction,
    ) -> ProgramResult {
        let input_account_len = accounts.len();
        let (
            token_program_info,
            amm_info,
            amm_authority_info,
            amm_target_orders_info,
            amm_lp_mint_info,
            amm_coin_vault_info,
            amm_pc_vault_info,
            user_source_lp_info,
            user_dest_coin_info,
            user_dest_pc_info,
            source_lp_owner_info,
        ) = if input_account_len == 11 {
            // Recommended use due to openbook has not supported.
            let account_info_iter = &mut accounts.iter();
            let token_program_info = next_account_info(account_info_iter)?;

            let amm_info = next_account_info(account_info_iter)?;
            let amm_authority_info = next_account_info(account_info_iter)?;
            let amm_target_orders_info = next_account_info(account_info_iter)?;
            let amm_lp_mint_info = next_account_info(account_info_iter)?;
            let amm_coin_vault_info = next_account_info(account_info_iter)?;
            let amm_pc_vault_info = next_account_info(account_info_iter)?;

            let user_source_lp_info = next_account_info(account_info_iter)?;
            let user_dest_coin_info = next_account_info(account_info_iter)?;
            let user_dest_pc_info = next_account_info(account_info_iter)?;
            let source_lp_owner_info = next_account_info(account_info_iter)?;

            (
                token_program_info,
                amm_info,
                amm_authority_info,
                amm_target_orders_info,
                amm_lp_mint_info,
                amm_coin_vault_info,
                amm_pc_vault_info,
                user_source_lp_info,
                user_dest_coin_info,
                user_dest_pc_info,
                source_lp_owner_info,
            )
        } else {
            const ACCOUNT_LEN: usize = 20;
            if input_account_len != ACCOUNT_LEN
                && input_account_len != ACCOUNT_LEN + 1
                && input_account_len != ACCOUNT_LEN + 2
                && input_account_len != ACCOUNT_LEN + 3
            {
                return Err(AmmError::WrongAccountsNumber.into());
            }
            let account_info_iter = &mut accounts.iter();
            let token_program_info = next_account_info(account_info_iter)?;

            let amm_info = next_account_info(account_info_iter)?;
            let amm_authority_info = next_account_info(account_info_iter)?;
            let _amm_open_orders_info = next_account_info(account_info_iter)?;
            let amm_target_orders_info = next_account_info(account_info_iter)?;
            let amm_lp_mint_info = next_account_info(account_info_iter)?;
            let amm_coin_vault_info = next_account_info(account_info_iter)?;
            let amm_pc_vault_info = next_account_info(account_info_iter)?;
            if input_account_len == ACCOUNT_LEN + 2 || input_account_len == ACCOUNT_LEN + 3 {
                let _padding_account_info1 = next_account_info(account_info_iter)?;
                let _padding_account_info2 = next_account_info(account_info_iter)?;
            }

            let _market_program_info = next_account_info(account_info_iter)?;
            let _market_info = next_account_info(account_info_iter)?;
            let _market_coin_vault_info = next_account_info(account_info_iter)?;
            let _market_pc_vault_info = next_account_info(account_info_iter)?;
            let _market_vault_signer = next_account_info(account_info_iter)?;

            let user_source_lp_info = next_account_info(account_info_iter)?;
            let user_dest_coin_info = next_account_info(account_info_iter)?;
            let user_dest_pc_info = next_account_info(account_info_iter)?;
            let source_lp_owner_info = next_account_info(account_info_iter)?;

            let _market_event_q_info = next_account_info(account_info_iter)?;
            let _market_bids_info = next_account_info(account_info_iter)?;
            let _market_asks_info = next_account_info(account_info_iter)?;

            (
                token_program_info,
                amm_info,
                amm_authority_info,
                amm_target_orders_info,
                amm_lp_mint_info,
                amm_coin_vault_info,
                amm_pc_vault_info,
                user_source_lp_info,
                user_dest_coin_info,
                user_dest_pc_info,
                source_lp_owner_info,
            )
        };

        if !source_lp_owner_info.is_signer {
            return Err(AmmError::InvalidSignAccount.into());
        }
        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        let mut target_orders =
            TargetOrders::load_mut_checked(&amm_target_orders_info, program_id, amm_info.key)?;

        if !AmmStatus::from_u64(amm.status).withdraw_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        if *amm_authority_info.key
            != Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?
        {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );
        let spl_token_program_id = token_program_info.key;
        // token_coin must be amm.coin_vault or token_dest_coin must not be amm.coin_vault
        if *amm_coin_vault_info.key != amm.coin_vault || *user_dest_coin_info.key == amm.coin_vault
        {
            return Err(AmmError::InvalidCoinVault.into());
        }
        // token_pc must be amm.pc_vault or token_dest_pc must not be amm.pc_vault
        if *amm_pc_vault_info.key != amm.pc_vault || *user_dest_pc_info.key == amm.pc_vault {
            return Err(AmmError::InvalidPCVault.into());
        }
        check_assert_eq!(
            *amm_target_orders_info.key,
            amm.target_orders,
            "target_orders",
            AmmError::InvalidTargetOrders
        );
        check_assert_eq!(
            *amm_lp_mint_info.key,
            amm.lp_mint,
            "lp_mint",
            AmmError::InvalidPoolMint
        );

        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;
        let user_dest_coin =
            Self::unpack_token_account(&user_dest_coin_info, spl_token_program_id)?;
        let user_dest_pc = Self::unpack_token_account(&user_dest_pc_info, spl_token_program_id)?;

        if user_dest_coin.mint != amm_coin_vault.mint || amm.coin_vault_mint != user_dest_coin.mint
        {
            return Err(AmmError::InvalidCoinMint.into());
        }
        if user_dest_pc.mint != amm_pc_vault.mint || amm.pc_vault_mint != user_dest_pc.mint {
            return Err(AmmError::InvalidPCMint.into());
        }

        let lp_mint = Self::unpack_mint(&amm_lp_mint_info, spl_token_program_id)?;
        let user_source_lp =
            Self::unpack_token_account(&user_source_lp_info, spl_token_program_id)?;
        if user_source_lp.mint != *amm_lp_mint_info.key {
            return Err(AmmError::InvalidTokenLP.into());
        }
        if withdraw.amount > user_source_lp.amount {
            return Err(AmmError::InsufficientFunds.into());
        }
        if withdraw.amount > lp_mint.supply || withdraw.amount >= amm.lp_amount {
            return Err(AmmError::NotAllowZeroLP.into());
        }
        let (mut total_pc_without_take_pnl, mut total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;

        let x1 = Calculator::normalize_decimal_v2(
            total_pc_without_take_pnl,
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let y1 = Calculator::normalize_decimal_v2(
            total_coin_without_take_pnl,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );

        // calc and update pnl
        let mut delta_x: u128 = 0;
        let mut delta_y: u128 = 0;
        if amm.status != AmmStatus::WithdrawOnly.into_u64() {
            (delta_x, delta_y) = Self::calc_take_pnl(
                &target_orders,
                &mut amm,
                &mut total_pc_without_take_pnl,
                &mut total_coin_without_take_pnl,
                x1.as_u128().into(),
                y1.as_u128().into(),
            )?;
        }

        // coin_amount / total_coin_amount = amount / lp_mint.supply => coin_amount = total_coin_amount * amount / pool_mint.supply
        let invariant = InvariantPool {
            token_input: withdraw.amount,
            token_total: amm.lp_amount,
        };
        let coin_amount = invariant
            .exchange_pool_to_token(total_coin_without_take_pnl, RoundDirection::Floor)
            .ok_or(AmmError::CalculationExRateFailure)?;
        let pc_amount = invariant
            .exchange_pool_to_token(total_pc_without_take_pnl, RoundDirection::Floor)
            .ok_or(AmmError::CalculationExRateFailure)?;

        encode_ray_log(WithdrawLog {
            log_type: LogType::Withdraw.into_u8(),
            withdraw_lp: withdraw.amount,
            user_lp: user_source_lp.amount,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            pool_lp: amm.lp_amount,
            calc_pnl_x: target_orders.calc_pnl_x,
            calc_pnl_y: target_orders.calc_pnl_y,
            out_coin: coin_amount,
            out_pc: pc_amount,
        });
        if withdraw.amount == 0 || coin_amount == 0 || pc_amount == 0 {
            return Err(AmmError::InvalidInput.into());
        }

        if coin_amount < amm_coin_vault.amount && pc_amount < amm_pc_vault.amount {
            if withdraw.min_coin_amount.is_some() && withdraw.min_pc_amount.is_some() {
                if withdraw.min_coin_amount.unwrap() > coin_amount
                    || withdraw.min_pc_amount.unwrap() > pc_amount
                {
                    return Err(AmmError::ExceededSlippage.into());
                }
            }
            Invokers::token_transfer_with_authority(
                token_program_info.clone(),
                amm_coin_vault_info.clone(),
                user_dest_coin_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                coin_amount,
            )?;
            Invokers::token_transfer_with_authority(
                token_program_info.clone(),
                amm_pc_vault_info.clone(),
                user_dest_pc_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                pc_amount,
            )?;
            Invokers::token_burn(
                token_program_info.clone(),
                user_source_lp_info.clone(),
                amm_lp_mint_info.clone(),
                source_lp_owner_info.clone(),
                withdraw.amount,
            )?;
            amm.lp_amount = amm.lp_amount.checked_sub(withdraw.amount).unwrap();
        } else {
            // calc error
            return Err(AmmError::TakePnlError.into());
        }

        // step4: update target_orders.calc_pnl_x & target_orders.calc_pnl_y
        target_orders.calc_pnl_x = x1
            .checked_sub(Calculator::normalize_decimal_v2(
                pc_amount,
                amm.pc_decimals,
                amm.sys_decimal_value,
            ))
            .unwrap()
            .checked_sub(U128::from(delta_x))
            .unwrap()
            .as_u128();
        target_orders.calc_pnl_y = y1
            .checked_sub(Calculator::normalize_decimal_v2(
                coin_amount,
                amm.coin_decimals,
                amm.sys_decimal_value,
            ))
            .unwrap()
            .checked_sub(U128::from(delta_y))
            .unwrap()
            .as_u128();
        amm.recent_epoch = Clock::get()?.epoch;
        Ok(())
    }

    pub fn process_swap_base_in(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        swap: SwapInstructionBaseIn,
    ) -> ProgramResult {
        const ACCOUNT_LEN: usize = 17;
        let input_account_len = accounts.len();
        if input_account_len != ACCOUNT_LEN && input_account_len != ACCOUNT_LEN + 1 {
            return Err(AmmError::WrongAccountsNumber.into());
        }
        let account_info_iter = &mut accounts.iter();
        let token_program_info = next_account_info(account_info_iter)?;

        let amm_info = next_account_info(account_info_iter)?;
        let amm_authority_info = next_account_info(account_info_iter)?;
        let _amm_open_orders_info = next_account_info(account_info_iter)?;
        if input_account_len == ACCOUNT_LEN + 1 {
            let _amm_target_orders_info = next_account_info(account_info_iter)?;
        }
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;

        let _market_program_info = next_account_info(account_info_iter)?;

        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        let _market_info = next_account_info(account_info_iter)?;
        let _market_bids_info = next_account_info(account_info_iter)?;
        let _market_asks_info = next_account_info(account_info_iter)?;
        let _market_event_queue_info = next_account_info(account_info_iter)?;
        let _market_coin_vault_info = next_account_info(account_info_iter)?;
        let _market_pc_vault_info = next_account_info(account_info_iter)?;
        let _market_vault_signer = next_account_info(account_info_iter)?;

        let user_source_info = next_account_info(account_info_iter)?;
        let user_destination_info = next_account_info(account_info_iter)?;
        let user_source_owner = next_account_info(account_info_iter)?;
        if !user_source_owner.is_signer {
            return Err(AmmError::InvalidSignAccount.into());
        }
        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );
        let spl_token_program_id = token_program_info.key;
        if *amm_authority_info.key
            != Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?
        {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        check_assert_eq!(
            *amm_coin_vault_info.key,
            amm.coin_vault,
            "coin_vault",
            AmmError::InvalidCoinVault
        );
        check_assert_eq!(
            *amm_pc_vault_info.key,
            amm.pc_vault,
            "pc_vault",
            AmmError::InvalidPCVault
        );

        if *user_source_info.key == amm.pc_vault || *user_source_info.key == amm.coin_vault {
            return Err(AmmError::InvalidUserToken.into());
        }
        if *user_destination_info.key == amm.pc_vault
            || *user_destination_info.key == amm.coin_vault
        {
            return Err(AmmError::InvalidUserToken.into());
        }

        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;

        let user_source = Self::unpack_token_account(&user_source_info, spl_token_program_id)?;
        let user_destination =
            Self::unpack_token_account(&user_destination_info, spl_token_program_id)?;

        if !AmmStatus::from_u64(amm.status).swap_permission() {
            msg!(&format!("swap_base_in: status {}", identity(amm.status)));
            let clock = Clock::get()?;
            if amm.status == AmmStatus::OrderBookOnly.into_u64()
                && (clock.unix_timestamp as u64) >= amm.state_data.orderbook_to_init_time
            {
                amm.status = AmmStatus::Initialized.into_u64();
                msg!("swap_base_in: OrderBook to Initialized");
            } else {
                return Err(AmmError::InvalidStatus.into());
            }
        } else if amm.status == AmmStatus::WaitingTrade.into_u64() {
            let clock = Clock::get()?;
            if (clock.unix_timestamp as u64) < amm.state_data.pool_open_time {
                return Err(AmmError::InvalidStatus.into());
            } else {
                amm.status = AmmStatus::SwapOnly.into_u64();
                msg!("swap_base_in: WaitingTrade to SwapOnly");
            }
        }

        let (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;

        let swap_direction;
        if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
            swap_direction = SwapDirection::Coin2PC
        } else if user_source.mint == amm_pc_vault.mint
            && user_destination.mint == amm_coin_vault.mint
        {
            swap_direction = SwapDirection::PC2Coin
        } else {
            return Err(AmmError::InvalidUserToken.into());
        }
        if user_source.amount < swap.amount_in {
            encode_ray_log(SwapBaseInLog {
                log_type: LogType::SwapBaseIn.into_u8(),
                amount_in: swap.amount_in,
                minimum_out: swap.minimum_amount_out,
                direction: swap_direction as u64,
                user_source: user_source.amount,
                pool_coin: total_coin_without_take_pnl,
                pool_pc: total_pc_without_take_pnl,
                out_amount: 0,
            });
            return Err(AmmError::InsufficientFunds.into());
        }
        let swap_fee = U128::from(swap.amount_in)
            .checked_mul(amm.fees.swap_fee_numerator.into())
            .unwrap()
            .checked_ceil_div(amm.fees.swap_fee_denominator.into())
            .unwrap();
        let swap_in_after_deduct_fee = U128::from(swap.amount_in).checked_sub(swap_fee).unwrap();
        let swap_amount_out = Calculator::swap_token_amount_base_in(
            swap_in_after_deduct_fee,
            total_pc_without_take_pnl.into(),
            total_coin_without_take_pnl.into(),
            swap_direction,
        )
        .as_u64();
        encode_ray_log(SwapBaseInLog {
            log_type: LogType::SwapBaseIn.into_u8(),
            amount_in: swap.amount_in,
            minimum_out: swap.minimum_amount_out,
            direction: swap_direction as u64,
            user_source: user_source.amount,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            out_amount: swap_amount_out,
        });
        if swap_amount_out < swap.minimum_amount_out {
            return Err(AmmError::ExceededSlippage.into());
        }
        if swap_amount_out == 0 || swap.amount_in == 0 {
            return Err(AmmError::InvalidInput.into());
        }

        match swap_direction {
            SwapDirection::Coin2PC => {
                if swap_amount_out >= total_pc_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }
                // deposit source coin to amm_coin_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_source_owner.clone(),
                    swap.amount_in,
                )?;
                // withdraw amm_pc_vault to destination pc
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap_amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_coin_in_amount = amm
                    .state_data
                    .swap_coin_in_amount
                    .checked_add(swap.amount_in.into())
                    .unwrap();
                amm.state_data.swap_pc_out_amount = amm
                    .state_data
                    .swap_pc_out_amount
                    .checked_add(swap_amount_out.into())
                    .unwrap();
                // charge coin as swap fee
                amm.state_data.swap_acc_coin_fee = amm
                    .state_data
                    .swap_acc_coin_fee
                    .checked_add(swap_fee.as_u64())
                    .unwrap();
            }
            SwapDirection::PC2Coin => {
                if swap_amount_out >= total_coin_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }
                // deposit source pc to amm_pc_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_source_owner.clone(),
                    swap.amount_in,
                )?;
                // withdraw amm_coin_vault to destination coin
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap_amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_pc_in_amount = amm
                    .state_data
                    .swap_pc_in_amount
                    .checked_add(swap.amount_in.into())
                    .unwrap();
                amm.state_data.swap_coin_out_amount = amm
                    .state_data
                    .swap_coin_out_amount
                    .checked_add(swap_amount_out.into())
                    .unwrap();
                // charge pc as swap fee
                amm.state_data.swap_acc_pc_fee = amm
                    .state_data
                    .swap_acc_pc_fee
                    .checked_add(swap_fee.as_u64())
                    .unwrap();
            }
        };
        amm.recent_epoch = Clock::get()?.epoch;

        Ok(())
    }

    pub fn process_swap_base_out(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        swap: SwapInstructionBaseOut,
    ) -> ProgramResult {
        const SWAP_ACCOUNT_NUM: usize = 17;
        let input_account_len = accounts.len();
        if input_account_len != SWAP_ACCOUNT_NUM && input_account_len != SWAP_ACCOUNT_NUM + 1 {
            return Err(AmmError::WrongAccountsNumber.into());
        }
        let account_info_iter = &mut accounts.iter();
        let token_program_info = next_account_info(account_info_iter)?;

        let amm_info = next_account_info(account_info_iter)?;
        let amm_authority_info = next_account_info(account_info_iter)?;
        let _amm_open_orders_info = next_account_info(account_info_iter)?;
        if input_account_len == SWAP_ACCOUNT_NUM + 1 {
            let _amm_target_orders_info = next_account_info(account_info_iter)?;
        }
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;

        let _market_program_info = next_account_info(account_info_iter)?;

        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }

        let _market_info = next_account_info(account_info_iter)?;
        let _market_bids_info = next_account_info(account_info_iter)?;
        let _market_asks_info = next_account_info(account_info_iter)?;
        let _market_event_queue_info = next_account_info(account_info_iter)?;
        let _market_coin_vault_info = next_account_info(account_info_iter)?;
        let _market_pc_vault_info = next_account_info(account_info_iter)?;
        let _market_vault_signer = next_account_info(account_info_iter)?;

        let user_source_info = next_account_info(account_info_iter)?;
        let user_destination_info = next_account_info(account_info_iter)?;
        let user_source_owner = next_account_info(account_info_iter)?;
        if !user_source_owner.is_signer {
            return Err(AmmError::InvalidSignAccount.into());
        }

        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );
        let spl_token_program_id = token_program_info.key;
        let authority = Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
        check_assert_eq!(
            *amm_authority_info.key,
            authority,
            "authority",
            AmmError::InvalidProgramAddress
        );
        check_assert_eq!(
            *amm_coin_vault_info.key,
            amm.coin_vault,
            "coin_vault",
            AmmError::InvalidCoinVault
        );
        check_assert_eq!(
            *amm_pc_vault_info.key,
            amm.pc_vault,
            "pc_vault",
            AmmError::InvalidPCVault
        );

        if *user_source_info.key == amm.pc_vault || *user_source_info.key == amm.coin_vault {
            return Err(AmmError::InvalidUserToken.into());
        }
        if *user_destination_info.key == amm.pc_vault
            || *user_destination_info.key == amm.coin_vault
        {
            return Err(AmmError::InvalidUserToken.into());
        }

        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;

        let user_source = Self::unpack_token_account(&user_source_info, spl_token_program_id)?;
        let user_destination =
            Self::unpack_token_account(&user_destination_info, spl_token_program_id)?;

        if !AmmStatus::from_u64(amm.status).swap_permission() {
            msg!(&format!("swap_base_out: status {}", identity(amm.status)));
            let clock = Clock::get()?;
            if amm.status == AmmStatus::OrderBookOnly.into_u64()
                && (clock.unix_timestamp as u64) >= amm.state_data.orderbook_to_init_time
            {
                amm.status = AmmStatus::Initialized.into_u64();
                msg!("swap_base_out: OrderBook to Initialized");
            } else {
                return Err(AmmError::InvalidStatus.into());
            }
        } else if amm.status == AmmStatus::WaitingTrade.into_u64() {
            let clock = Clock::get()?;
            if (clock.unix_timestamp as u64) < amm.state_data.pool_open_time {
                return Err(AmmError::InvalidStatus.into());
            } else {
                amm.status = AmmStatus::SwapOnly.into_u64();
                msg!("swap_base_out: WaitingTrade to SwapOnly");
            }
        }

        let (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;

        let swap_direction;
        if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
            swap_direction = SwapDirection::Coin2PC
        } else if user_source.mint == amm_pc_vault.mint
            && user_destination.mint == amm_coin_vault.mint
        {
            swap_direction = SwapDirection::PC2Coin
        } else {
            return Err(AmmError::InvalidUserToken.into());
        }

        let swap_in_before_add_fee = Calculator::swap_token_amount_base_out(
            swap.amount_out.into(),
            total_pc_without_take_pnl.into(),
            total_coin_without_take_pnl.into(),
            swap_direction,
        );
        // swap_in_after_add_fee * (1 - 0.0025) = swap_in_before_add_fee
        // swap_in_after_add_fee = swap_in_before_add_fee / (1 - 0.0025)
        let swap_in_after_add_fee = swap_in_before_add_fee
            .checked_mul(amm.fees.swap_fee_denominator.into())
            .unwrap()
            .checked_ceil_div(
                (amm.fees
                    .swap_fee_denominator
                    .checked_sub(amm.fees.swap_fee_numerator)
                    .unwrap())
                .into(),
            )
            .unwrap()
            .as_u64();
        let swap_fee = swap_in_after_add_fee
            .checked_sub(swap_in_before_add_fee.as_u64())
            .unwrap();
        encode_ray_log(SwapBaseOutLog {
            log_type: LogType::SwapBaseOut.into_u8(),
            max_in: swap.max_amount_in,
            amount_out: swap.amount_out,
            direction: swap_direction as u64,
            user_source: user_source.amount,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            deduct_in: swap_in_after_add_fee,
        });
        if user_source.amount < swap_in_after_add_fee {
            return Err(AmmError::InsufficientFunds.into());
        }
        if swap.max_amount_in < swap_in_after_add_fee {
            return Err(AmmError::ExceededSlippage.into());
        }
        if swap_in_after_add_fee == 0 || swap.amount_out == 0 {
            return Err(AmmError::InvalidInput.into());
        }

        match swap_direction {
            SwapDirection::Coin2PC => {
                if swap.amount_out >= total_pc_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }
                // deposit source coin to amm_coin_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_source_owner.clone(),
                    swap_in_after_add_fee,
                )?;
                // withdraw amm_pc_vault to destination pc
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap.amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_coin_in_amount = amm
                    .state_data
                    .swap_coin_in_amount
                    .checked_add(swap_in_after_add_fee.into())
                    .unwrap();
                amm.state_data.swap_pc_out_amount = amm
                    .state_data
                    .swap_pc_out_amount
                    .checked_add(Calculator::to_u128(swap.amount_out)?)
                    .unwrap();
                // charge coin as swap fee
                amm.state_data.swap_acc_coin_fee = amm
                    .state_data
                    .swap_acc_coin_fee
                    .checked_add(swap_fee)
                    .unwrap();
            }
            SwapDirection::PC2Coin => {
                if swap.amount_out >= total_coin_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }

                // deposit source pc to amm_pc_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_source_owner.clone(),
                    swap_in_after_add_fee,
                )?;
                // withdraw amm_coin_vault to destination coin
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap.amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_pc_in_amount = amm
                    .state_data
                    .swap_pc_in_amount
                    .checked_add(swap_in_after_add_fee.into())
                    .unwrap();
                amm.state_data.swap_coin_out_amount = amm
                    .state_data
                    .swap_coin_out_amount
                    .checked_add(swap.amount_out.into())
                    .unwrap();
                // charge pc as swap fee
                amm.state_data.swap_acc_pc_fee = amm
                    .state_data
                    .swap_acc_pc_fee
                    .checked_add(swap_fee)
                    .unwrap();
            }
        };
        amm.recent_epoch = Clock::get()?.epoch;

        Ok(())
    }

    pub fn process_swap_base_in_v2(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        swap: SwapInstructionBaseIn,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let token_program_info = next_account_info(account_info_iter)?;
        let amm_info = next_account_info(account_info_iter)?;
        let amm_authority_info = next_account_info(account_info_iter)?;
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;
        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        let user_source_info = next_account_info(account_info_iter)?;
        let user_destination_info = next_account_info(account_info_iter)?;
        let user_source_owner = next_account_info(account_info_iter)?;
        if !user_source_owner.is_signer {
            return Err(AmmError::InvalidSignAccount.into());
        }
        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );
        let spl_token_program_id = token_program_info.key;
        if *amm_authority_info.key
            != Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?
        {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        check_assert_eq!(
            *amm_coin_vault_info.key,
            amm.coin_vault,
            "coin_vault",
            AmmError::InvalidCoinVault
        );
        check_assert_eq!(
            *amm_pc_vault_info.key,
            amm.pc_vault,
            "pc_vault",
            AmmError::InvalidPCVault
        );

        if *user_source_info.key == amm.pc_vault || *user_source_info.key == amm.coin_vault {
            return Err(AmmError::InvalidUserToken.into());
        }
        if *user_destination_info.key == amm.pc_vault
            || *user_destination_info.key == amm.coin_vault
        {
            return Err(AmmError::InvalidUserToken.into());
        }

        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;

        let user_source = Self::unpack_token_account(&user_source_info, spl_token_program_id)?;
        let user_destination =
            Self::unpack_token_account(&user_destination_info, spl_token_program_id)?;

        if !AmmStatus::from_u64(amm.status).swap_permission() {
            msg!(&format!("swap_base_in_v2: status {}", identity(amm.status)));
            let clock = Clock::get()?;
            if amm.status == AmmStatus::OrderBookOnly.into_u64()
                && (clock.unix_timestamp as u64) >= amm.state_data.orderbook_to_init_time
            {
                amm.status = AmmStatus::Initialized.into_u64();
                msg!("swap_base_in_v2: OrderBook to Initialized");
            } else {
                return Err(AmmError::InvalidStatus.into());
            }
        } else if amm.status == AmmStatus::WaitingTrade.into_u64() {
            let clock = Clock::get()?;
            if (clock.unix_timestamp as u64) < amm.state_data.pool_open_time {
                return Err(AmmError::InvalidStatus.into());
            } else {
                amm.status = AmmStatus::SwapOnly.into_u64();
                msg!("swap_base_in_v2: WaitingTrade to SwapOnly");
            }
        }

        let (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;

        let swap_direction;
        if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
            swap_direction = SwapDirection::Coin2PC
        } else if user_source.mint == amm_pc_vault.mint
            && user_destination.mint == amm_coin_vault.mint
        {
            swap_direction = SwapDirection::PC2Coin
        } else {
            return Err(AmmError::InvalidUserToken.into());
        }
        if user_source.amount < swap.amount_in {
            encode_ray_log(SwapBaseInLog {
                log_type: LogType::SwapBaseIn.into_u8(),
                amount_in: swap.amount_in,
                minimum_out: swap.minimum_amount_out,
                direction: swap_direction as u64,
                user_source: user_source.amount,
                pool_coin: total_coin_without_take_pnl,
                pool_pc: total_pc_without_take_pnl,
                out_amount: 0,
            });
            return Err(AmmError::InsufficientFunds.into());
        }
        let swap_fee = U128::from(swap.amount_in)
            .checked_mul(amm.fees.swap_fee_numerator.into())
            .unwrap()
            .checked_ceil_div(amm.fees.swap_fee_denominator.into())
            .unwrap();
        let swap_in_after_deduct_fee = U128::from(swap.amount_in).checked_sub(swap_fee).unwrap();
        let swap_amount_out = Calculator::swap_token_amount_base_in(
            swap_in_after_deduct_fee,
            total_pc_without_take_pnl.into(),
            total_coin_without_take_pnl.into(),
            swap_direction,
        )
        .as_u64();
        encode_ray_log(SwapBaseInLog {
            log_type: LogType::SwapBaseIn.into_u8(),
            amount_in: swap.amount_in,
            minimum_out: swap.minimum_amount_out,
            direction: swap_direction as u64,
            user_source: user_source.amount,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            out_amount: swap_amount_out,
        });
        if swap_amount_out < swap.minimum_amount_out {
            return Err(AmmError::ExceededSlippage.into());
        }
        if swap_amount_out == 0 || swap.amount_in == 0 {
            return Err(AmmError::InvalidInput.into());
        }

        match swap_direction {
            SwapDirection::Coin2PC => {
                if swap_amount_out >= total_pc_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }
                // deposit source coin to amm_coin_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_source_owner.clone(),
                    swap.amount_in,
                )?;
                // withdraw amm_pc_vault to destination pc
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap_amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_coin_in_amount = amm
                    .state_data
                    .swap_coin_in_amount
                    .checked_add(swap.amount_in.into())
                    .unwrap();
                amm.state_data.swap_pc_out_amount = amm
                    .state_data
                    .swap_pc_out_amount
                    .checked_add(swap_amount_out.into())
                    .unwrap();
                // charge coin as swap fee
                amm.state_data.swap_acc_coin_fee = amm
                    .state_data
                    .swap_acc_coin_fee
                    .checked_add(swap_fee.as_u64())
                    .unwrap();
            }
            SwapDirection::PC2Coin => {
                if swap_amount_out >= total_coin_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }
                // deposit source pc to amm_pc_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_source_owner.clone(),
                    swap.amount_in,
                )?;
                // withdraw amm_coin_vault to destination coin
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap_amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_pc_in_amount = amm
                    .state_data
                    .swap_pc_in_amount
                    .checked_add(swap.amount_in.into())
                    .unwrap();
                amm.state_data.swap_coin_out_amount = amm
                    .state_data
                    .swap_coin_out_amount
                    .checked_add(swap_amount_out.into())
                    .unwrap();
                // charge pc as swap fee
                amm.state_data.swap_acc_pc_fee = amm
                    .state_data
                    .swap_acc_pc_fee
                    .checked_add(swap_fee.as_u64())
                    .unwrap();
            }
        };
        amm.recent_epoch = Clock::get()?.epoch;

        Ok(())
    }

    pub fn process_swap_base_out_v2(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        swap: SwapInstructionBaseOut,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let token_program_info = next_account_info(account_info_iter)?;
        let amm_info = next_account_info(account_info_iter)?;
        let amm_authority_info = next_account_info(account_info_iter)?;
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;
        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        let user_source_info = next_account_info(account_info_iter)?;
        let user_destination_info = next_account_info(account_info_iter)?;
        let user_source_owner = next_account_info(account_info_iter)?;
        if !user_source_owner.is_signer {
            return Err(AmmError::InvalidSignAccount.into());
        }

        check_assert_eq!(
            *token_program_info.key,
            spl_token::id(),
            "spl_token_program",
            AmmError::InvalidSplTokenProgram
        );
        let spl_token_program_id = token_program_info.key;
        let authority = Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
        check_assert_eq!(
            *amm_authority_info.key,
            authority,
            "authority",
            AmmError::InvalidProgramAddress
        );
        check_assert_eq!(
            *amm_coin_vault_info.key,
            amm.coin_vault,
            "coin_vault",
            AmmError::InvalidCoinVault
        );
        check_assert_eq!(
            *amm_pc_vault_info.key,
            amm.pc_vault,
            "pc_vault",
            AmmError::InvalidPCVault
        );

        if *user_source_info.key == amm.pc_vault || *user_source_info.key == amm.coin_vault {
            return Err(AmmError::InvalidUserToken.into());
        }
        if *user_destination_info.key == amm.pc_vault
            || *user_destination_info.key == amm.coin_vault
        {
            return Err(AmmError::InvalidUserToken.into());
        }

        let amm_coin_vault =
            Self::unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
        let amm_pc_vault = Self::unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;

        let user_source = Self::unpack_token_account(&user_source_info, spl_token_program_id)?;
        let user_destination =
            Self::unpack_token_account(&user_destination_info, spl_token_program_id)?;

        if !AmmStatus::from_u64(amm.status).swap_permission() {
            msg!(&format!(
                "swap_base_out_v2: status {}",
                identity(amm.status)
            ));
            let clock = Clock::get()?;
            if amm.status == AmmStatus::OrderBookOnly.into_u64()
                && (clock.unix_timestamp as u64) >= amm.state_data.orderbook_to_init_time
            {
                amm.status = AmmStatus::Initialized.into_u64();
                msg!("swap_base_out_v2: OrderBook to Initialized");
            } else {
                return Err(AmmError::InvalidStatus.into());
            }
        } else if amm.status == AmmStatus::WaitingTrade.into_u64() {
            let clock = Clock::get()?;
            if (clock.unix_timestamp as u64) < amm.state_data.pool_open_time {
                return Err(AmmError::InvalidStatus.into());
            } else {
                amm.status = AmmStatus::SwapOnly.into_u64();
                msg!("swap_base_out_v2: WaitingTrade to SwapOnly");
            }
        }

        let (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;

        let swap_direction;
        if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
            swap_direction = SwapDirection::Coin2PC
        } else if user_source.mint == amm_pc_vault.mint
            && user_destination.mint == amm_coin_vault.mint
        {
            swap_direction = SwapDirection::PC2Coin
        } else {
            return Err(AmmError::InvalidUserToken.into());
        }

        let swap_in_before_add_fee = Calculator::swap_token_amount_base_out(
            swap.amount_out.into(),
            total_pc_without_take_pnl.into(),
            total_coin_without_take_pnl.into(),
            swap_direction,
        );
        // swap_in_after_add_fee * (1 - 0.0025) = swap_in_before_add_fee
        // swap_in_after_add_fee = swap_in_before_add_fee / (1 - 0.0025)
        let swap_in_after_add_fee = swap_in_before_add_fee
            .checked_mul(amm.fees.swap_fee_denominator.into())
            .unwrap()
            .checked_ceil_div(
                (amm.fees
                    .swap_fee_denominator
                    .checked_sub(amm.fees.swap_fee_numerator)
                    .unwrap())
                .into(),
            )
            .unwrap()
            .as_u64();
        let swap_fee = swap_in_after_add_fee
            .checked_sub(swap_in_before_add_fee.as_u64())
            .unwrap();
        encode_ray_log(SwapBaseOutLog {
            log_type: LogType::SwapBaseOut.into_u8(),
            max_in: swap.max_amount_in,
            amount_out: swap.amount_out,
            direction: swap_direction as u64,
            user_source: user_source.amount,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            deduct_in: swap_in_after_add_fee,
        });
        if user_source.amount < swap_in_after_add_fee {
            return Err(AmmError::InsufficientFunds.into());
        }
        if swap.max_amount_in < swap_in_after_add_fee {
            return Err(AmmError::ExceededSlippage.into());
        }
        if swap_in_after_add_fee == 0 || swap.amount_out == 0 {
            return Err(AmmError::InvalidInput.into());
        }

        match swap_direction {
            SwapDirection::Coin2PC => {
                if swap.amount_out >= total_pc_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }
                // deposit source coin to amm_coin_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_source_owner.clone(),
                    swap_in_after_add_fee,
                )?;
                // withdraw amm_pc_vault to destination pc
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap.amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_coin_in_amount = amm
                    .state_data
                    .swap_coin_in_amount
                    .checked_add(swap_in_after_add_fee.into())
                    .unwrap();
                amm.state_data.swap_pc_out_amount = amm
                    .state_data
                    .swap_pc_out_amount
                    .checked_add(Calculator::to_u128(swap.amount_out)?)
                    .unwrap();
                // charge coin as swap fee
                amm.state_data.swap_acc_coin_fee = amm
                    .state_data
                    .swap_acc_coin_fee
                    .checked_add(swap_fee)
                    .unwrap();
            }
            SwapDirection::PC2Coin => {
                if swap.amount_out >= total_coin_without_take_pnl {
                    return Err(AmmError::InsufficientFunds.into());
                }
                // deposit source pc to amm_pc_vault
                Invokers::token_transfer(
                    token_program_info.clone(),
                    user_source_info.clone(),
                    amm_pc_vault_info.clone(),
                    user_source_owner.clone(),
                    swap_in_after_add_fee,
                )?;
                // withdraw amm_coin_vault to destination coin
                Invokers::token_transfer_with_authority(
                    token_program_info.clone(),
                    amm_coin_vault_info.clone(),
                    user_destination_info.clone(),
                    amm_authority_info.clone(),
                    AUTHORITY_AMM,
                    amm.nonce as u8,
                    swap.amount_out,
                )?;
                // update state_data data
                amm.state_data.swap_pc_in_amount = amm
                    .state_data
                    .swap_pc_in_amount
                    .checked_add(swap_in_after_add_fee.into())
                    .unwrap();
                amm.state_data.swap_coin_out_amount = amm
                    .state_data
                    .swap_coin_out_amount
                    .checked_add(swap.amount_out.into())
                    .unwrap();
                // charge pc as swap fee
                amm.state_data.swap_acc_pc_fee = amm
                    .state_data
                    .swap_acc_pc_fee
                    .checked_add(swap_fee)
                    .unwrap();
            }
        };
        amm.recent_epoch = Clock::get()?.epoch;

        Ok(())
    }

    pub fn process_set_params(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        setparams: SetParamsInstruction,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let token_program_info = next_account_info(account_info_iter)?;

        let amm_info = next_account_info(account_info_iter)?;
        let amm_authority_info = next_account_info(account_info_iter)?;
        let amm_open_orders_info = next_account_info(account_info_iter)?;
        let amm_target_orders_info = next_account_info(account_info_iter)?;
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;

        let amm_owner_info = next_account_info(account_info_iter)?;

        if *token_program_info.key != spl_token::ID {
            return Err(AmmError::InvalidSplTokenProgram.into());
        }
        let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
        if AmmStatus::from_u64(amm.status).orderbook_permission() {
            return Err(AmmError::InvalidStatus.into());
        }
        if *amm_authority_info.key
            != Self::authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?
        {
            return Err(AmmError::InvalidProgramAddress.into());
        }
        if amm_info.owner != program_id {
            return Err(AmmError::InvalidOwner.into());
        }
        if !amm_owner_info.is_signer || *amm_owner_info.key != config_feature::amm_owner::ID {
            return Err(AmmError::InvalidSignAccount.into());
        }
        if *amm_open_orders_info.key != amm.open_orders {
            return Err(AmmError::InvalidOpenOrders.into());
        }
        if amm.coin_vault != *amm_coin_vault_info.key {
            return Err(AmmError::InvalidCoinVault.into());
        }
        if amm.pc_vault != *amm_pc_vault_info.key {
            return Err(AmmError::InvalidPCVault.into());
        }
        if amm.target_orders != *amm_target_orders_info.key {
            return Err(AmmError::InvalidTargetOrders.into());
        }

        let param = setparams.param;
        let mut set_valid = false;
        match AmmParams::from_u64(param as u64) {
            AmmParams::Status => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if AmmStatus::valid_status(value) {
                    amm.status = value as u64;
                    set_valid = true;
                }
            }
            AmmParams::State => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if AmmState::valid_state(value) {
                    amm.state = value as u64;
                    set_valid = true;
                }
            }
            AmmParams::OrderNum => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if value > MAX_ORDER_LIMIT as u64 {
                    return Err(AmmError::InvalidInput.into());
                }
                amm.order_num = value as u64;
                set_valid = true;
            }
            AmmParams::Depth => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if value > 0 && value < 100 {
                    amm.depth = value as u64;
                    set_valid = true;
                }
            }
            AmmParams::AmountWave => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                amm.amount_wave = value;
                set_valid = true;
            }
            AmmParams::MinPriceMultiplier => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if value < amm.max_price_multiplier {
                    amm.min_price_multiplier = value;
                    set_valid = true;
                }
            }
            AmmParams::MaxPriceMultiplier => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if value > amm.max_price_multiplier {
                    amm.max_price_multiplier = value;
                    set_valid = true;
                }
            }
            AmmParams::VolMaxCutRatio => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if value <= TEN_THOUSAND {
                    amm.vol_max_cut_ratio = value;
                    set_valid = true;
                }
            }
            AmmParams::Seperate => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                if value <= TEN_THOUSAND {
                    amm.fees.min_separate_numerator = value;
                    set_valid = true;
                }
            }
            AmmParams::Fees => {
                let fees = match setparams.fees {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                fees.validate()?;
                amm.fees = fees;
                set_valid = true;
            }
            AmmParams::AmmOwner => {
                let new_pubkey = match setparams.new_pubkey {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                amm.amm_owner = new_pubkey;
                set_valid = true;
            }
            AmmParams::SetOpenTime => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                amm.state_data.pool_open_time = value as u64;
                set_valid = true;
            }
            AmmParams::LastOrderDistance => {
                let mut target = TargetOrders::load_mut_checked(
                    &amm_target_orders_info,
                    program_id,
                    amm_info.key,
                )?;
                let distance = match setparams.last_order_distance {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                target.last_order_numerator = distance.last_order_numerator;
                target.last_order_denominator = distance.last_order_denominator;
                set_valid = true;
            }
            AmmParams::InitOrderDepth => {
                amm.order_num = 7u64;
                amm.depth = 3u64;
                set_valid = true;
            }
            AmmParams::SetSwitchTime => {
                let value = match setparams.value {
                    Some(a) => a,
                    None => return Err(AmmError::InvalidInput.into()),
                };
                amm.state_data.orderbook_to_init_time = value as u64;
                set_valid = true;
            }
            AmmParams::ClearOpenTime => {
                amm.state_data.pool_open_time = 0;
                amm.state_data.orderbook_to_init_time = 0;
                set_valid = true;
            }
            AmmParams::UpdateOpenOrder => {
                set_valid = true;
            }
            _ => {
                return Err(AmmError::InvalidInput.into());
            }
        }
        if set_valid {
            amm.state = AmmState::CancelAllOrdersState.into_u64();
            amm.reset_flag = AmmResetFlag::ResetYes.into_u64();
        } else {
            return Err(AmmError::InvalidParamsSet.into());
        }
        amm.recent_epoch = Clock::get()?.epoch;
        Ok(())
    }

    /// Processes `process_create_config` instruction.
    pub fn process_create_config(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let admin_info = next_account_info(account_info_iter)?;
        let amm_config_info = next_account_info(account_info_iter)?;
        let pnl_owner_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;
        let rent_sysvar_info = next_account_info(account_info_iter)?;

        if !admin_info.is_signer || config_feature::amm_owner::id() != *admin_info.key {
            return Err(AmmError::InvalidSignAccount.into());
        }
        if *system_program_info.key != solana_program::system_program::id() {
            return Err(AmmError::InvalidSysProgramAddress.into());
        }

        let (pda, bump_seed) = Pubkey::find_program_address(&[&&AMM_CONFIG_SEED], program_id);
        if pda != *amm_config_info.key {
            return Err(AmmError::InvalidConfigAccount.into());
        }
        if amm_config_info.owner != system_program_info.key {
            return Err(AmmError::RepeatCreateConfigAccount.into());
        }
        let pda_signer_seeds: &[&[_]] = &[&AMM_CONFIG_SEED, &[bump_seed]];
        let rent = &Rent::from_account_info(rent_sysvar_info)?;
        let data_size = size_of::<AmmConfig>();
        let required_lamports = rent
            .minimum_balance(data_size)
            .max(1)
            .saturating_sub(amm_config_info.lamports());
        if required_lamports > 0 {
            invoke(
                &system_instruction::transfer(
                    admin_info.key,
                    amm_config_info.key,
                    required_lamports,
                ),
                &[
                    admin_info.clone(),
                    amm_config_info.clone(),
                    system_program_info.clone(),
                ],
            )?;
        }
        invoke_signed(
            &system_instruction::allocate(amm_config_info.key, data_size as u64),
            &[amm_config_info.clone(), system_program_info.clone()],
            &[&pda_signer_seeds],
        )?;
        invoke_signed(
            &system_instruction::assign(amm_config_info.key, &program_id),
            &[amm_config_info.clone(), system_program_info.clone()],
            &[&pda_signer_seeds],
        )?;

        let mut amm_config = AmmConfig::load_mut_checked(&amm_config_info, program_id)?;
        amm_config.pnl_owner = *pnl_owner_info.key;
        amm_config.create_pool_fee = 0;

        Ok(())
    }

    /// Processes `process_update_config` instruction.
    pub fn process_update_config(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        config_args: ConfigArgs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let admin_info = next_account_info(account_info_iter)?;
        let amm_config_info = next_account_info(account_info_iter)?;
        if !admin_info.is_signer || config_feature::amm_owner::id() != *admin_info.key {
            return Err(AmmError::InvalidSignAccount.into());
        }
        let (pda, _) = Pubkey::find_program_address(&[&AMM_CONFIG_SEED], program_id);
        if pda != *amm_config_info.key || amm_config_info.owner != program_id {
            return Err(AmmError::InvalidConfigAccount.into());
        }

        let mut amm_config = AmmConfig::load_mut_checked(&amm_config_info, program_id)?;
        match config_args.param {
            0 => {
                let pnl_owner = config_args.owner.unwrap();
                if pnl_owner == Pubkey::default() {
                    return Err(AmmError::InvalidInput.into());
                }
                amm_config.pnl_owner = pnl_owner;
            }
            1 => {
                let cancel_owner = config_args.owner.unwrap();
                if cancel_owner == Pubkey::default() {
                    return Err(AmmError::InvalidInput.into());
                }
                amm_config.cancel_owner = cancel_owner;
            }
            2 => {
                let create_pool_fee = config_args.create_pool_fee.unwrap();
                amm_config.create_pool_fee = create_pool_fee;
            }
            _ => {
                return Err(AmmError::InvalidInput.into());
            }
        }

        return Ok(());
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = AmmInstruction::unpack(input)?;
        match instruction {
            AmmInstruction::PreInitialize(_init_arg) => {
                unimplemented!("This instruction is not supported, please use Initialize2")
            }
            AmmInstruction::Initialize(_init1) => {
                unimplemented!("This instruction is not supported, please use Initialize2")
            }
            AmmInstruction::Initialize2(init2) => {
                Self::process_initialize2(program_id, accounts, init2)
            }
            AmmInstruction::MonitorStep(_monitor) => {
                unimplemented!("This instruction is not supported")
            }
            AmmInstruction::Deposit(deposit) => {
                Self::process_deposit(program_id, accounts, deposit)
            }
            AmmInstruction::Withdraw(withdraw) => {
                Self::process_withdraw(program_id, accounts, withdraw)
            }
            AmmInstruction::MigrateToOpenBook => {
                unimplemented!("This instruction is not supported")
            }
            AmmInstruction::SetParams(setparams) => {
                Self::process_set_params(program_id, accounts, setparams)
            }
            AmmInstruction::WithdrawPnl => Self::process_withdrawpnl(program_id, accounts),
            AmmInstruction::WithdrawSrm(_withdrawsrm) => {
                unimplemented!("This instruction is not supported")
            }
            AmmInstruction::SwapBaseIn(swap) => {
                Self::process_swap_base_in(program_id, accounts, swap)
            }
            AmmInstruction::SwapBaseOut(swap) => {
                Self::process_swap_base_out(program_id, accounts, swap)
            }
            AmmInstruction::SimulateInfo(_simulate) => {
                unimplemented!("This instruction is not supported")
            }
            AmmInstruction::AdminCancelOrders(_cancel) => {
                unimplemented!("This instruction is not supported")
            }
            AmmInstruction::CreateConfigAccount => {
                Self::process_create_config(program_id, accounts)
            }
            AmmInstruction::UpdateConfigAccount(config_args) => {
                Self::process_update_config(program_id, accounts, config_args)
            }
            AmmInstruction::SwapBaseInV2(swap) => {
                Self::process_swap_base_in_v2(program_id, accounts, swap)
            }
            AmmInstruction::SwapBaseOutV2(swap) => {
                Self::process_swap_base_out_v2(program_id, accounts, swap)
            }
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_calc_take_pnl() {
        let mut amm = AmmInfo::default();
        amm.initialize(0, 0, 2, 9, 1000000, 1).unwrap();
        let mut target = TargetOrders::default();
        target.calc_pnl_x = 900000000000000;
        target.calc_pnl_y = 150000000000000000000000000;

        let mut total_pc_without_take_pnl = 1343675125663;
        let mut total_coin_without_take_pnl = 117837534493793;
        let x1 = Calculator::normalize_decimal_v2(
            total_pc_without_take_pnl,
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let y1 = Calculator::normalize_decimal_v2(
            total_coin_without_take_pnl,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );

        let (delta_x, delta_y) = Processor::calc_take_pnl(
            &target,
            &mut amm,
            &mut total_pc_without_take_pnl,
            &mut total_coin_without_take_pnl,
            x1.as_u128().into(),
            y1.as_u128().into(),
        )
        .unwrap();
        println!("delta_x:{}, delta_y:{}", delta_x, delta_y);
    }

    #[test]
    fn test_calc_pnl_precision() {
        // init
        let mut amm = AmmInfo::default();
        let init_pc_amount = 5434000000u64;
        let init_coin_amount = 100000000000000u64;
        let liquidity = Calculator::to_u64(
            U128::from(init_pc_amount)
                .checked_mul(init_coin_amount.into())
                .unwrap()
                .integer_sqrt()
                .as_u128(),
        )
        .unwrap();
        amm.initialize(0, 0, 5, 9, 1000000000, 7803).unwrap();
        amm.lp_amount = liquidity;

        let x =
            Calculator::normalize_decimal_v2(5434000000, amm.pc_decimals, amm.sys_decimal_value);
        let y = Calculator::normalize_decimal_v2(
            100000000000000,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );
        let mut target = TargetOrders::default();
        target.calc_pnl_x = x.as_u128();
        target.calc_pnl_y = y.as_u128();
        println!(
             "init_pc_amount:{}, init_coin_amount:{}, liquidity:{}, sys_decimal_value:{}, calc_pnl_x:{}, calc_pnl_y:{}",
             init_pc_amount, init_coin_amount, liquidity, identity(amm.sys_decimal_value), identity(target.calc_pnl_x), identity(target.calc_pnl_y)
         );

        // withdraw
        let withdraw_lp = 2577470628u64;
        let mut total_pc_without_take_pnl = init_pc_amount;
        let mut total_coin_without_take_pnl = init_coin_amount;
        let x1 = Calculator::normalize_decimal_v2(
            total_pc_without_take_pnl,
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let y1 = Calculator::normalize_decimal_v2(
            total_coin_without_take_pnl,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );

        let (delta_x, delta_y) = Processor::calc_take_pnl(
            &target,
            &mut amm,
            &mut total_pc_without_take_pnl,
            &mut total_coin_without_take_pnl,
            x1.as_u128().into(),
            y1.as_u128().into(),
        )
        .unwrap();
        println!("delta_x:{}, delta_y:{}", delta_x, delta_y);
        // coin_amount / total_coin_amount = amount / lp_mint.supply => coin_amount = total_coin_amount * amount / pool_mint.supply
        let invariant = InvariantPool {
            token_input: withdraw_lp,
            token_total: amm.lp_amount,
        };
        let coin_amount = invariant
            .exchange_pool_to_token(total_coin_without_take_pnl, RoundDirection::Floor)
            .ok_or(AmmError::CalculationExRateFailure)
            .unwrap();
        let pc_amount = invariant
            .exchange_pool_to_token(total_pc_without_take_pnl, RoundDirection::Floor)
            .ok_or(AmmError::CalculationExRateFailure)
            .unwrap();

        amm.lp_amount = amm.lp_amount.checked_sub(withdraw_lp).unwrap();
        target.calc_pnl_x = x1
            .checked_sub(Calculator::normalize_decimal_v2(
                pc_amount,
                amm.pc_decimals,
                amm.sys_decimal_value,
            ))
            .unwrap()
            .checked_sub(U128::from(delta_x))
            .unwrap()
            .as_u128();
        target.calc_pnl_y = y1
            .checked_sub(Calculator::normalize_decimal_v2(
                coin_amount,
                amm.coin_decimals,
                amm.sys_decimal_value,
            ))
            .unwrap()
            .checked_sub(U128::from(delta_y))
            .unwrap()
            .as_u128();
        total_pc_without_take_pnl = total_pc_without_take_pnl.checked_sub(pc_amount).unwrap();
        total_coin_without_take_pnl = total_coin_without_take_pnl
            .checked_sub(coin_amount)
            .unwrap();
        println!(
             "withdraw calc_pnl_x:{}, calc_pnl_y:{}, total_pc_without_take_pnl:{}, total_coin_without_take_pnl:{}",
             identity(target.calc_pnl_x), identity(target.calc_pnl_y), total_pc_without_take_pnl, total_coin_without_take_pnl
         );

        // withdraw 2
        let x1 = Calculator::normalize_decimal_v2(
            total_pc_without_take_pnl,
            amm.pc_decimals,
            amm.sys_decimal_value,
        );
        let y1 = Calculator::normalize_decimal_v2(
            total_coin_without_take_pnl,
            amm.coin_decimals,
            amm.sys_decimal_value,
        );

        let (delta_x, delta_y) = Processor::calc_take_pnl(
            &target,
            &mut amm,
            &mut total_pc_without_take_pnl,
            &mut total_coin_without_take_pnl,
            x1.as_u128().into(),
            y1.as_u128().into(),
        )
        .unwrap();
        println!("delta_x:{}, delta_y:{}", delta_x, delta_y);
    }

    #[test]
    fn test_swap_base_in() {
        let amount_in = 212854295571_u64;
        let total_coin_without_take_pnl = 77043918330755_u64;
        let total_pc_without_take_pnl = 1511361338135_u64;

        let swap_direction = SwapDirection::Coin2PC;
        let mut amm = AmmInfo::default();
        amm.initialize(0, 0, 2, 9, 1000000, 1).unwrap();

        let swap_fee = U128::from(amount_in)
            .checked_mul(amm.fees.swap_fee_numerator.into())
            .unwrap()
            .checked_ceil_div(amm.fees.swap_fee_denominator.into())
            .unwrap();

        let swap_in_after_deduct_fee = U128::from(amount_in).checked_sub(swap_fee).unwrap();
        let swap_amount_out = Calculator::swap_token_amount_base_in(
            swap_in_after_deduct_fee,
            total_pc_without_take_pnl.into(),
            total_coin_without_take_pnl.into(),
            swap_direction,
        )
        .as_u64();

        println!("swap_amount_out:{}", swap_amount_out);
    }
}
