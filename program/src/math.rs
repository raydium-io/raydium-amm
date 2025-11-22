//! Defines PreciseNumber, a U256 wrapper with float-like operations
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::unknown_clippy_lints)]
#![allow(clippy::manual_range_contains)]

use crate::{error::AmmError, state::AmmInfo};
use serum_dex::{
    matching::Side,
    state::{EventView, MarketState, OpenOrders, ToAlignedBytes},
};
use solana_program::{account_info::AccountInfo, log::sol_log_compute_units, msg};
use std::{cmp::Eq, convert::identity, convert::TryInto};
use uint::construct_uint;

construct_uint! {
    pub struct U256(4);
}
construct_uint! {
    pub struct U128(2);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum SwapDirection {
    /// Input token pc, output token coin
    PC2Coin = 1u64,
    /// Input token coin, output token pc
    Coin2PC = 2u64,
}

/// The direction to round.  Used for pool token to trading token conversions to
/// avoid losing value on any deposit or withdrawal.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoundDirection {
    /// Floor the value, ie. 1.9 => 1, 1.1 => 1, 1.5 => 1
    Floor,
    /// Ceiling the value, ie. 1.9 => 2, 1.1 => 2, 1.5 => 2
    Ceiling,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Calculator {}

impl Calculator {
    pub fn to_u128(val: u64) -> Result<u128, AmmError> {
        val.try_into().map_err(|_| AmmError::ConversionFailure)
    }

    pub fn to_u64(val: u128) -> Result<u64, AmmError> {
        val.try_into().map_err(|_| AmmError::ConversionFailure)
    }

    pub fn calc_x_power(last_x: U256, last_y: U256, current_x: U256, current_y: U256) -> Result<U256, AmmError> {
        // Safety: Changed to Result to prevent runtime panics on overflow
        last_x
            .checked_mul(last_y)
            .ok_or(AmmError::CalculationFailure)?
            .checked_mul(current_x)
            .ok_or(AmmError::CalculationFailure)?
            .checked_div(current_y)
            .ok_or(AmmError::CalculationFailure)
    }

    // OPTIMIZATION: Pre-allocate vector capacity to avoid re-allocations in BPF
    pub fn fibonacci(order_num: u64) -> Vec<u64> {
        let mut fb = Vec::with_capacity(order_num as usize);
        for i in 0..order_num {
            if i == 0 {
                fb.push(0u64);
            } else if i == 1 {
                fb.push(1u64);
            } else if i == 2 {
                // Note: Custom logic from original code (0, 1, 2...) instead of (0, 1, 1...)
                fb.push(2u64);
            } else {
                let ret = fb[(i - 1u64) as usize] + fb[(i - 2u64) as usize];
                fb.push(ret);
            };
        }
        fb
    }

    pub fn normalize_decimal(val: u64, native_decimal: u64, sys_decimal_value: u64) -> Result<u64, AmmError> {
        let ret_mut = U128::from(val)
            .checked_mul(sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        let denominator = U128::from(10)
            .checked_pow(native_decimal.into())
            .ok_or(AmmError::CalculationFailure)?;

        let result_u128 = ret_mut
            .checked_div(denominator)
            .ok_or(AmmError::CalculationFailure)?;
            
        Self::to_u64(result_u128.as_u128())
    }

    pub fn restore_decimal(val: U128, native_decimal: u64, sys_decimal_value: u64) -> Result<U128, AmmError> {
        let multiplier = U128::from(10)
            .checked_pow(native_decimal.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        val.checked_mul(multiplier)
            .ok_or(AmmError::CalculationFailure)?
            .checked_div(sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)
    }

    pub fn normalize_decimal_v2(val: u64, native_decimal: u64, sys_decimal_value: u64) -> Result<U128, AmmError> {
        let ret_mut = U128::from(val)
            .checked_mul(sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        let denominator = U128::from(10)
            .checked_pow(native_decimal.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        ret_mut
            .checked_div(denominator)
            .ok_or(AmmError::CalculationFailure)
    }

    pub fn floor_lot(val: u64, lot_size: u64) -> Result<u64, AmmError> {
        let unit = val.checked_div(lot_size).ok_or(AmmError::CalculationFailure)?;
        unit.checked_mul(lot_size).ok_or(AmmError::CalculationFailure)
    }

    pub fn ceil_lot(val: u64, lot_size: u64) -> Result<u64, AmmError> {
        let unit = (val as u128)
            .checked_ceil_div(lot_size as u128)
            .ok_or(AmmError::CalculationFailure)?;
            
        Self::to_u64(unit)?
            .checked_mul(lot_size)
            .ok_or(AmmError::CalculationFailure)
    }

    // convert internal pc_lot_size -> srm pc_lot_size
    pub fn convert_out_pc_lot_size(
        pc_decimals: u8,
        coin_decimals: u8,
        pc_lot_size: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let numerator = U128::from(pc_lot_size)
            .checked_mul(coin_lot_size.into())
            .ok_or(AmmError::CalculationFailure)?
            .checked_mul(U128::from(10).checked_pow(pc_decimals.into()).ok_or(AmmError::CalculationFailure)?)
            .ok_or(AmmError::CalculationFailure)?;
            
        let denominator = U128::from(sys_decimal_value)
            .checked_mul(U128::from(10).checked_pow(coin_decimals.into()).ok_or(AmmError::CalculationFailure)?)
            .ok_or(AmmError::CalculationFailure)?;
            
        let result = numerator.checked_div(denominator).ok_or(AmmError::CalculationFailure)?;
        Self::to_u64(result.as_u128())
    }

    // convert srm pc_lot_size -> internal pc_lot_size
    pub fn convert_in_pc_lot_size(
        pc_decimals: u8,
        coin_decimals: u8,
        pc_lot_size: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let num_part1 = U128::from(pc_lot_size)
            .checked_mul(sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        let num_part2 = U128::from(10)
            .checked_pow(coin_decimals.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        let numerator = num_part1.checked_mul(num_part2).ok_or(AmmError::CalculationFailure)?;

        let den_part1 = U128::from(coin_lot_size);
        let den_part2 = U128::from(10)
            .checked_pow(pc_decimals.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        let denominator = den_part1.checked_mul(den_part2).ok_or(AmmError::CalculationFailure)?;

        let result = numerator.checked_div(denominator).ok_or(AmmError::CalculationFailure)?;
        Self::to_u64(result.as_u128())
    }

    pub fn convert_in_price(val: u64, pc_lot_size: u64) -> Result<u64, AmmError> {
        val.checked_mul(pc_lot_size).ok_or(AmmError::CalculationFailure)
    }

    pub fn convert_price_out(val: u64, pc_lot_size: u64) -> Result<u64, AmmError> {
        val.checked_div(pc_lot_size).ok_or(AmmError::CalculationFailure)
    }

    pub fn convert_in_vol(
        val: u64,
        coin_decimal: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let volume = U128::from(val)
            .checked_mul(coin_lot_size.into())
            .ok_or(AmmError::CalculationFailure)?
            .checked_mul(sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)?
            .checked_div(U128::from(10).checked_pow(coin_decimal.into()).ok_or(AmmError::CalculationFailure)?)
            .ok_or(AmmError::CalculationFailure)?;
            
        Self::to_u64(volume.as_u128())
    }

    pub fn convert_vol_out(
        val: u64,
        coin_decimal: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let numerator = U128::from(val)
            .checked_mul(U128::from(10).checked_pow(coin_decimal.into()).ok_or(AmmError::CalculationFailure)?)
            .ok_or(AmmError::CalculationFailure)?;
            
        let denominator = U128::from(coin_lot_size)
            .checked_mul(sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)?;
            
        let volume = numerator.checked_div(denominator).ok_or(AmmError::CalculationFailure)?;
        Self::to_u64(volume.as_u128())
    }

    pub fn calc_exact_vault_in_serum<'a>(
        open_orders: &'a OpenOrders,
        market_state: &'a Box<MarketState>,
        event_q_account: &'a AccountInfo,
        amm_open_account: &'a AccountInfo,
    ) -> Result<(u64, u64), AmmError> {
        // Uses Serum's load_event_queue_mut which returns Result usually, unwrapped here implies trust in data availability.
        // Keeping unwrap() here only if load_event_queue_mut signature requires it or if error handling is done upstream, 
        // but ideally this should also be safe.
        let event_q = market_state.load_event_queue_mut(event_q_account).map_err(|_| AmmError::InvalidEventQueue)?;
        
        let mut native_pc_total = open_orders.native_pc_total;
        let mut native_coin_total = open_orders.native_coin_total;
        
        msg!("calc_exact len:{}", event_q.len());
        sol_log_compute_units();
        
        for event in event_q.iter() {
            if identity(event.owner) != (*amm_open_account.key).to_aligned_bytes() {
                continue;
            }
            
            // Safety: Handle potential view parsing error
            if let Ok(view) = event.as_view() {
                match view {
                    EventView::Fill {
                        side,
                        maker,
                        native_qty_paid,
                        native_qty_received,
                        ..
                    } => {
                        match side {
                            Side::Bid if maker => {
                                native_pc_total = native_pc_total.checked_sub(native_qty_paid).ok_or(AmmError::CalculationFailure)?;
                                native_coin_total = native_coin_total.checked_add(native_qty_received).ok_or(AmmError::CalculationFailure)?;
                            }
                            Side::Ask if maker => {
                                native_coin_total = native_coin_total.checked_sub(native_qty_paid).ok_or(AmmError::CalculationFailure)?;
                                native_pc_total = native_pc_total.checked_add(native_qty_received).ok_or(AmmError::CalculationFailure)?;
                            }
                            _ => (),
                        };
                    }
                    _ => continue,
                }
            }
        }
        sol_log_compute_units();
        Ok((native_pc_total, native_coin_total))
    }

    pub fn calc_total_without_take_pnl<'a>(
        pc_amount: u64,
        coin_amount: u64,
        open_orders: &'a OpenOrders,
        amm: &'a AmmInfo,
        market_state: &'a Box<MarketState>,
        event_q_account: &'a AccountInfo,
        amm_open_account: &'a AccountInfo,
    ) -> Result<(u64, u64), AmmError> {
        let (pc_total_in_serum, coin_total_in_serum) = Self::calc_exact_vault_in_serum(
            open_orders,
            market_state,
            event_q_account,
            amm_open_account,
        )?;

        let total_pc_without_take_pnl = pc_amount
            .checked_add(pc_total_in_serum)
            .ok_or(AmmError::CheckedAddOverflow)?
            .checked_sub(amm.state_data.need_take_pnl_pc)
            .ok_or(AmmError::CheckedSubOverflow)?;
            
        let total_coin_without_take_pnl = coin_amount
            .checked_add(coin_total_in_serum)
            .ok_or(AmmError::CheckedAddOverflow)?
            .checked_sub(amm.state_data.need_take_pnl_coin)
            .ok_or(AmmError::CheckedSubOverflow)?;
            
        Ok((total_pc_without_take_pnl, total_coin_without_take_pnl))
    }

    pub fn calc_total_without_take_pnl_no_orderbook<'a>(
        pc_amount: u64,
        coin_amount: u64,
        amm: &'a AmmInfo,
    ) -> Result<(u64, u64), AmmError> {
        let total_pc_without_take_pnl = pc_amount
            .checked_sub(amm.state_data.need_take_pnl_pc)
            .ok_or(AmmError::CheckedSubOverflow)?;
        let total_coin_without_take_pnl = coin_amount
            .checked_sub(amm.state_data.need_take_pnl_coin)
            .ok_or(AmmError::CheckedSubOverflow)?;
        Ok((total_pc_without_take_pnl, total_coin_without_take_pnl))
    }

    pub fn get_max_buy_size_at_price(price: u64, x: u128, y: u128, amm: &AmmInfo) -> Result<u64, AmmError> {
        let price_with_fee = U128::from(price)
            .checked_mul(U128::from(amm.fees.trade_fee_denominator + amm.fees.trade_fee_numerator))
            .ok_or(AmmError::CalculationFailure)?
            .checked_div(U128::from(amm.fees.trade_fee_denominator))
            .ok_or(AmmError::CalculationFailure)?;
            
        let mut max_size = U128::from(x)
            .checked_mul(amm.sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)?
            .checked_div(price_with_fee)
            .ok_or(AmmError::CalculationFailure)?;
            
        max_size = max_size.saturating_sub(y.into());
        Self::to_u64(max_size.as_u128())
    }

    pub fn get_max_sell_size_at_price(price: u64, x: u128, y: u128, amm: &AmmInfo) -> Result<u64, AmmError> {
        let price_with_fee = U128::from(price)
            .checked_mul(amm.fees.trade_fee_denominator.into())
            .ok_or(AmmError::CalculationFailure)?
            .checked_div(U128::from(amm.fees.trade_fee_denominator + amm.fees.trade_fee_numerator))
            .ok_or(AmmError::CalculationFailure)?;
            
        let second_part = U128::from(x)
            .checked_mul(amm.sys_decimal_value.into())
            .ok_or(AmmError::CalculationFailure)?
            .checked_div(price_with_fee)
            .ok_or(AmmError::CalculationFailure)?;

        let max_size = U128::from(y).saturating_sub(second_part);
        Self::to_u64(max_size.as_u128())
    }

    pub fn swap_token_amount_base_in(
        amount_in: U128,
        total_pc_without_take_pnl: U128,
        total_coin_without_take_pnl: U128,
        swap_direction: SwapDirection,
    ) -> Result<U128, AmmError> {
        match swap_direction {
            SwapDirection::Coin2PC => {
                let denominator = total_coin_without_take_pnl.checked_add(amount_in).ok_or(AmmError::CalculationFailure)?;
                total_pc_without_take_pnl
                    .checked_mul(amount_in)
                    .ok_or(AmmError::CalculationFailure)?
                    .checked_div(denominator)
                    .ok_or(AmmError::CalculationFailure)
            }
            SwapDirection::PC2Coin => {
                let denominator = total_pc_without_take_pnl.checked_add(amount_in).ok_or(AmmError::CalculationFailure)?;
                total_coin_without_take_pnl
                    .checked_mul(amount_in)
                    .ok_or(AmmError::CalculationFailure)?
                    .checked_div(denominator)
                    .ok_or(AmmError::CalculationFailure)
            }
        }
    }

    pub fn swap_token_amount_base_out(
        amount_out: U128,
        total_pc_without_take_pnl: U128,
        total_coin_without_take_pnl: U128,
        swap_direction: SwapDirection,
    ) -> Result<U128, AmmError> {
        match swap_direction {
            SwapDirection::Coin2PC => {
                let denominator = total_pc_without_take_pnl.checked_sub(amount_out).ok_or(AmmError::CalculationFailure)?;
                total_coin_without_take_pnl
                    .checked_mul(amount_out)
                    .ok_or(AmmError::CalculationFailure)?
                    .checked_ceil_div(denominator)
                    .ok_or(AmmError::CalculationFailure)
            }
            SwapDirection::PC2Coin => {
                let denominator = total_coin_without_take_pnl.checked_sub(amount_out).ok_or(AmmError::CalculationFailure)?;
                total_pc_without_take_pnl
                    .checked_mul(amount_out)
                    .ok_or(AmmError::CalculationFailure)?
                    .checked_ceil_div(denominator)
                    .ok_or(AmmError::CalculationFailure)
            }
        }
    }
}

/// The invariant calculator.
pub struct InvariantToken {
    /// Token coin
    pub token_coin: u64,
    /// Token pc
    pub token_pc: u64,
}

impl InvariantToken {
    /// Exchange rate
    pub fn exchange_coin_to_pc(
        &self,
        token_coin: u64,
        round_direction: RoundDirection,
    ) -> Option<u64> {
        let result_u128 = if round_direction == RoundDirection::Floor {
            U128::from(token_coin)
                .checked_mul(self.token_pc.into())?
                .checked_div(self.token_coin.into())?
        } else {
            U128::from(token_coin)
                .checked_mul(self.token_pc.into())?
                .checked_ceil_div(self.token_coin.into())?
        };
        Some(result_u128.as_u64())
    }

    /// Exchange rate
    pub fn exchange_pc_to_coin(
        &self,
        token_pc: u64,
        round_direction: RoundDirection,
    ) -> Option<u64> {
        let result_u128 = if round_direction == RoundDirection::Floor {
            U128::from(token_pc)
                .checked_mul(self.token_coin.into())?
                .checked_div(self.token_pc.into())?
        } else {
            U128::from(token_pc)
                .checked_mul(self.token_coin.into())?
                .checked_ceil_div(self.token_pc.into())?
        };
        Some(result_u128.as_u64())
    }
}

/// The invariant calculator.
pub struct InvariantPool {
    /// Token input
    pub token_input: u64,
    /// Token total
    pub token_total: u64,
}
impl InvariantPool {
    /// Exchange rate
    pub fn exchange_pool_to_token(
        &self,
        token_total_amount: u64,
        round_direction: RoundDirection,
    ) -> Option<u64> {
        let result_u128 = if round_direction == RoundDirection::Floor {
            U128::from(token_total_amount)
                .checked_mul(self.token_input.into())?
                .checked_div(self.token_total.into())?
        } else {
            U128::from(token_total_amount)
                .checked_mul(self.token_input.into())?
                .checked_ceil_div(self.token_total.into())?
        };
        Some(result_u128.as_u64())
    }
    /// Exchange rate
    pub fn exchange_token_to_pool(
        &self,
        pool_total_amount: u64,
        round_direction: RoundDirection,
    ) -> Option<u64> {
        let result_u128 = if round_direction == RoundDirection::Floor {
            U128::from(pool_total_amount)
                .checked_mul(self.token_input.into())?
                .checked_div(self.token_total.into())?
        } else {
            U128::from(pool_total_amount)
                .checked_mul(self.token_input.into())?
                .checked_ceil_div(self.token_total.into())?
        };
        Some(result_u128.as_u64())
    }
}

pub trait CheckedCeilDiv: Sized {
    /// Perform ceiling division
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self>;
}

impl CheckedCeilDiv for u128 {
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self> {
        let quotient = self.checked_div(rhs)?;
        let remainder = self.checked_rem(rhs)?;
        if remainder != 0 {
            quotient.checked_add(1)
        } else {
            Some(quotient)
        }
    }
}

impl CheckedCeilDiv for U128 {
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self> {
        let quotient = self.checked_div(rhs)?;
        let remainder = self.checked_rem(rhs)?;
        if remainder != U128::zero() {
            quotient.checked_add(U128::one())
        } else {
            Some(quotient)
        }
    }
}
