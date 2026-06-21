//! Defines the Calculator struct, providing safety-critical arithmetic operations
//! for the Automated Market Maker (AMM) logic on Solana.
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
use std::{cmp::Eq, convert::TryInto};
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

/// The direction to round. Used for pool token to trading token conversions to
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

    /// Calculates the constant product invariant component: x * y * current_x / current_y.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn calc_x_power(last_x: U256, last_y: U256, current_x: U256, current_y: U256) -> Result<U256, AmmError> {
        let xy = last_x.checked_mul(last_y).ok_or(AmmError::CheckedMulOverflow)?;
        let xy_cx = xy.checked_mul(current_x).ok_or(AmmError::CheckedMulOverflow)?;
        let x_power = xy_cx.checked_div(current_y).ok_or(AmmError::CheckedDivByZero)?;
        Ok(x_power)
    }

    /// Generates the standard Fibonacci sequence: 0, 1, 1, 2, 3, 5, 8, ...
    pub fn fibonacci(order_num: u64) -> Vec<u64> {
        let mut fb = Vec::with_capacity(order_num as usize);
        
        for i in 0..order_num {
            if i == 0 {
                fb.push(0u64);
            } else if i == 1 {
                fb.push(1u64);
            } else {
                // F(n) = F(n-1) + F(n-2)
                let ret = fb[(i - 1u64) as usize].checked_add(fb[(i - 2u64) as usize]).unwrap_or_else(|| u64::MAX);
                fb.push(ret);
            };
        }
        fb
    }

    /// Converts a u64 amount from native decimals to system decimals (returns U128).
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn normalize_decimal(val: u64, native_decimal: u64, sys_decimal_value: u64) -> Result<U128, AmmError> {
        // e.g., (1.23 * 10^9) -> (1.23 * 10^6)
        let native_pow = U128::from(10).checked_pow(native_decimal.into()).ok_or(AmmError::CheckedPowOverflow)?;

        let ret_mut = (U128::from(val))
            .checked_mul(sys_decimal_value.into())
            .ok_or(AmmError::CheckedMulOverflow)?;

        let ret = ret_mut
            .checked_div(native_pow)
            .ok_or(AmmError::CheckedDivByZero)?;
        
        Ok(ret)
    }

    /// Converts a U128 amount from system decimals back to native decimals.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn restore_decimal(val: U128, native_decimal: u64, sys_decimal_value: u64) -> Result<U128, AmmError> {
        // e.g., (1.23 * 10^6) -> (1.23 * 10^9)
        let native_pow = U128::from(10).checked_pow(native_decimal.into()).ok_or(AmmError::CheckedPowOverflow)?;

        let ret_mut = val
            .checked_mul(native_pow)
            .ok_or(AmmError::CheckedMulOverflow)?;

        let ret = ret_mut.checked_div(sys_decimal_value.into()).ok_or(AmmError::CheckedDivByZero)?;
        
        Ok(ret)
    }

    /// Helper to floor a value to the nearest lot size.
    pub fn floor_lot(val: u64, lot_size: u64) -> Result<u64, AmmError> {
        // all numbers are in normalized decimal already
        if lot_size == 0 {
            return Err(AmmError::CheckedDivByZero);
        }
        let unit: u64 = val.checked_div(lot_size).ok_or(AmmError::CheckedDivByZero)?;
        let ret: u64 = unit.checked_mul(lot_size).ok_or(AmmError::CheckedMulOverflow)?;
        Ok(ret)
    }

    /// Helper to ceil a value to the nearest lot size.
    /// CRITICAL: Replaced final unwraps with safe error handling.
    pub fn ceil_lot(val: u64, lot_size: u64) -> Result<u64, AmmError> {
        let unit: u128 = (val as u128).checked_ceil_div(lot_size as u128).ok_or(AmmError::CheckedDivByZero)?;
        let ret: u64 = Self::to_u64(unit.checked_mul(lot_size as u128).ok_or(AmmError::CheckedMulOverflow)?).map_err(|_| AmmError::ConversionFailure)?;
        Ok(ret)
    }

    /*
        Lot Size Conversion Logic (SRM <-> Internal)
    */

    /// Convert internal pc_lot_size -> srm pc_lot_size
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn convert_out_pc_lot_size(
        pc_decimals: u8,
        coin_decimals: u8,
        pc_lot_size: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let pc_pow = U128::from(10).checked_pow(pc_decimals.into()).ok_or(AmmError::CheckedPowOverflow)?;
        let coin_pow = U128::from(10).checked_pow(coin_decimals.into()).ok_or(AmmError::CheckedPowOverflow)?;

        let numerator = U128::from(pc_lot_size)
            .checked_mul(coin_lot_size.into()).ok_or(AmmError::CheckedMulOverflow)?
            .checked_mul(pc_pow).ok_or(AmmError::CheckedMulOverflow)?;

        let denominator = U128::from(sys_decimal_value)
            .checked_mul(coin_pow).ok_or(AmmError::CheckedMulOverflow)?;
        
        let native_lot_size_u128 = numerator.checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?;

        Self::to_u64(native_lot_size_u128.as_u128())
    }

    /// Convert srm pc_lot_size -> internal pc_lot_size
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn convert_in_pc_lot_size(
        pc_decimals: u8,
        coin_decimals: u8,
        pc_lot_size: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let pc_pow = U128::from(10).checked_pow(pc_decimals.into()).ok_or(AmmError::CheckedPowOverflow)?;
        let coin_pow = U128::from(10).checked_pow(coin_decimals.into()).ok_or(AmmError::CheckedPowOverflow)?;

        let numerator = U128::from(pc_lot_size)
            .checked_mul(sys_decimal_value.into()).ok_or(AmmError::CheckedMulOverflow)?
            .checked_mul(coin_pow).ok_or(AmmError::CheckedMulOverflow)?;
        
        let denominator = U128::from(coin_lot_size)
            .checked_mul(pc_pow).ok_or(AmmError::CheckedMulOverflow)?;

        let native_lot_size_u128 = numerator.checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?;
        
        Self::to_u64(native_lot_size_u128.as_u128())
    }

    /// Convert srm price -> internal price
    pub fn convert_in_price(val: u64, pc_lot_size: u64) -> Result<u64, AmmError> {
        val.checked_mul(pc_lot_size).ok_or(AmmError::CheckedMulOverflow)
    }

    /// Convert internal price -> srm price
    pub fn convert_price_out(val: u64, pc_lot_size: u64) -> Result<u64, AmmError> {
        val.checked_div(pc_lot_size).ok_or(AmmError::CheckedDivByZero)
    }

    /// Convert srm coin size -> internal coin size
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn convert_in_vol(
        val: u64,
        coin_decimal: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let coin_pow = U128::from(10).checked_pow(coin_decimal.into()).ok_or(AmmError::CheckedPowOverflow)?;

        let volume: U128 = U128::from(val)
            .checked_mul(coin_lot_size.into()).ok_or(AmmError::CheckedMulOverflow)?
            .checked_mul(sys_decimal_value.into()).ok_or(AmmError::CheckedMulOverflow)?
            .checked_div(coin_pow).ok_or(AmmError::CheckedDivByZero)?;
        
        Self::to_u64(volume.as_u128())
    }

    /// Convert internal coin size -> srm coin size
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn convert_vol_out(
        val: u64,
        coin_decimal: u64,
        coin_lot_size: u64,
        sys_decimal_value: u64,
    ) -> Result<u64, AmmError> {
        let coin_pow = U128::from(10).checked_pow(coin_decimal.into()).ok_or(AmmError::CheckedPowOverflow)?;
        let denom_mult = U128::from(coin_lot_size)
            .checked_mul(sys_decimal_value.into()).ok_or(AmmError::CheckedMulOverflow)?;

        let volume: U128 = U128::from(val)
            .checked_mul(coin_pow).ok_or(AmmError::CheckedMulOverflow)?
            .checked_div(denom_mult).ok_or(AmmError::CheckedDivByZero)?;
            
        Self::to_u64(volume.as_u128())
    }

    /// Calculates the precise amount of PC and Coin tokens currently held in the Serum OpenOrders vault 
    /// by scanning the EventQueue.
    pub fn calc_exact_vault_in_serum<'a>(
        open_orders: &'a OpenOrders,
        market_state: &'a Box<MarketState>,
        event_q_account: &'a AccountInfo,
        amm_open_account: &'a AccountInfo,
    ) -> Result<(u64, u64), AmmError> {
        // CRITICAL: .unwrap() on market_state load must be handled.
        let event_q = market_state.load_event_queue_mut(event_q_account).map_err(|_| AmmError::LoadEventQueueFailure)?;
        
        let mut native_pc_total = open_orders.native_pc_total;
        let mut native_coin_total = open_orders.native_coin_total;
        
        msg!("calc_exact len:{}", event_q.len());
        sol_log_compute_units();
        
        for event in event_q.iter() {
            // Check if the event belongs to this AMM's OpenOrders account
            if event.owner.to_aligned_bytes() != (*amm_open_account.key).to_aligned_bytes() {
                continue;
            }
            
            let event_view = event.as_view().ok_or(AmmError::EventViewFailure)?;

            match event_view {
                EventView::Fill {
                    side,
                    maker,
                    native_qty_paid,
                    native_qty_received,
                    native_fee_or_rebate: _,
                    fee_tier: _,
                    order_id: _,
                    owner: _,
                    owner_slot: _,
                    client_order_id: _,
                } => {
                    match side {
                        // Maker Bid: AMM is selling Coin (receiving PC)
                        Side::Bid if maker => {
                            native_pc_total = native_pc_total.checked_sub(native_qty_paid).ok_or(AmmError::CheckedSubOverflow)?;
                            native_coin_total = native_coin_total.checked_add(native_qty_received).ok_or(AmmError::CheckedAddOverflow)?;
                        }
                        // Maker Ask: AMM is selling PC (receiving Coin)
                        Side::Ask if maker => {
                            native_coin_total = native_coin_total.checked_sub(native_qty_paid).ok_or(AmmError::CheckedSubOverflow)?;
                            native_pc_total = native_pc_total.checked_add(native_qty_received).ok_or(AmmError::CheckedAddOverflow)?;
                        }
                        // Taker fills are handled by Serum
                        _ => (),
                    };
                }
                _ => {
                    continue;
                }
            }
        }
        sol_log_compute_units();
        Ok((native_pc_total, native_coin_total))
    }

    /// Calculates the total PC and Coin balance without the unrealized Profit & Loss (PNL).
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

    /// Simplified PNL calculation when orderbook state (Serum) is not needed.
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
    
    // --- AMM Price/Size Calculation Functions ---

    /// Gets the maximum possible buy size based on the invariant curve and price.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn get_max_buy_size_at_price(price: u64, x: u128, y: u128, amm: &AmmInfo) -> Result<u64, AmmError> {
        let fee_numerator_u128 = U128::from(amm.fees.trade_fee_numerator);
        let fee_denominator_u128 = U128::from(amm.fees.trade_fee_denominator);

        let price_with_fee = U128::from(price)
            .checked_mul(fee_denominator_u128.checked_add(fee_numerator_u128).ok_or(AmmError::CheckedAddOverflow)?).ok_or(AmmError::CheckedMulOverflow)?
            .checked_div(fee_denominator_u128).ok_or(AmmError::CheckedDivByZero)?;

        let mut max_size = U128::from(x)
            .checked_mul(amm.sys_decimal_value.into()).ok_or(AmmError::CheckedMulOverflow)?
            .checked_div(price_with_fee).ok_or(AmmError::CheckedDivByZero)?;

        // max_size = x / (price * (1 + fee)) - y
        max_size = max_size.saturating_sub(y.into());
        Self::to_u64(max_size.as_u128())
    }

    /// Gets the maximum possible sell size based on the invariant curve and price.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn get_max_sell_size_at_price(price: u64, x: u128, y: u128, amm: &AmmInfo) -> Result<u64, AmmError> {
        let fee_numerator_u128 = U128::from(amm.fees.trade_fee_numerator);
        let fee_denominator_u128 = U128::from(amm.fees.trade_fee_denominator);
        
        let price_with_fee = U128::from(price)
            .checked_mul(fee_denominator_u128).ok_or(AmmError::CheckedMulOverflow)?
            .checked_div(fee_denominator_u128.checked_add(fee_numerator_u128).ok_or(AmmError::CheckedAddOverflow)?).ok_or(AmmError::CheckedDivByZero)?;

        let second_part = U128::from(x)
            .checked_mul(amm.sys_decimal_value.into()).ok_or(AmmError::CheckedMulOverflow)?
            .checked_div(price_with_fee.into()).ok_or(AmmError::CheckedDivByZero)?;

        // max_size = y - x / (price / (1 + fee))
        let max_size = U128::from(y).saturating_sub(second_part);
        Self::to_u64(max_size.as_u128())
    }

    // --- Constant Product Swap Functions ---

    /// Calculates the output amount (amount_out) given the input amount (amount_in) 
    /// using the constant product formula (x + dx)(y - dy) = k.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn swap_token_amount_base_in(
        amount_in: U128,
        total_pc_without_take_pnl: U128,
        total_coin_without_take_pnl: U128,
        swap_direction: SwapDirection,
    ) -> Result<U128, AmmError> {
        let amount_out;
        match swap_direction {
            SwapDirection::Coin2PC => {
                // (coin + amount_in) * (pc - amount_out) = coin * pc
                // => amount_out = pc * amount_in / (coin + amount_in)
                let denominator = total_coin_without_take_pnl.checked_add(amount_in).ok_or(AmmError::CheckedAddOverflow)?;
                amount_out = total_pc_without_take_pnl
                    .checked_mul(amount_in).ok_or(AmmError::CheckedMulOverflow)?
                    .checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?;
            }
            SwapDirection::PC2Coin => {
                // (pc + amount_in) * (coin - amount_out) = coin * pc
                // => amount_out = coin * amount_in / (pc + amount_in)
                let denominator = total_pc_without_take_pnl.checked_add(amount_in).ok_or(AmmError::CheckedAddOverflow)?;
                amount_out = total_coin_without_take_pnl
                    .checked_mul(amount_in).ok_or(AmmError::CheckedMulOverflow)?
                    .checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?;
            }
        }
        Ok(amount_out)
    }

    /// Calculates the required input amount (amount_in) to achieve a desired output amount (amount_out) 
    /// using the constant product formula. Uses checked_ceil_div to ensure sufficient input.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn swap_token_amount_base_out(
        amount_out: U128,
        total_pc_without_take_pnl: U128,
        total_coin_without_take_pnl: U128,
        swap_direction: SwapDirection,
    ) -> Result<U128, AmmError> {
        let amount_in;
        match swap_direction {
            SwapDirection::Coin2PC => {
                // amount_in = (amount_out * coin) / (pc - amount_out)
                let denominator = total_pc_without_take_pnl.checked_sub(amount_out).ok_or(AmmError::CheckedSubOverflow)?;
                amount_in = total_coin_without_take_pnl
                    .checked_mul(amount_out).ok_or(AmmError::CheckedMulOverflow)?
                    .checked_ceil_div(denominator).ok_or(AmmError::CheckedDivByZero)?;
            }
            SwapDirection::PC2Coin => {
                // amount_in = (pc * amount_out) / (coin - amount_out)
                let denominator = total_coin_without_take_pnl.checked_sub(amount_out).ok_or(AmmError::CheckedSubOverflow)?;
                amount_in = total_pc_without_take_pnl
                    .checked_mul(amount_out).ok_or(AmmError::CheckedMulOverflow)?
                    .checked_ceil_div(denominator).ok_or(AmmError::CheckedDivByZero)?;
            }
        }
        Ok(amount_in)
    }
}

// --- Auxiliary Structs and Traits ---

/// The invariant calculator.
pub struct InvariantToken {
    /// Token coin balance
    pub token_coin: u64,
    /// Token pc balance
    pub token_pc: u64,
}

impl InvariantToken {
    /// Exchange rate: converts Coin amount to PC amount.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn exchange_coin_to_pc(
        &self,
        token_coin: u64,
        round_direction: RoundDirection,
    ) -> Result<u64, AmmError> {
        let numerator = U128::from(token_coin).checked_mul(self.token_pc.into()).ok_or(AmmError::CheckedMulOverflow)?;
        let denominator = self.token_coin.into();

        let amount_out_u128 = if round_direction == RoundDirection::Floor {
            numerator.checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        } else {
            numerator.checked_ceil_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        };
        
        Self::to_u64(amount_out_u128.as_u128())
    }

    /// Exchange rate: converts PC amount to Coin amount.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn exchange_pc_to_coin(
        &self,
        token_pc: u64,
        round_direction: RoundDirection,
    ) -> Result<u64, AmmError> {
        let numerator = U128::from(token_pc).checked_mul(self.token_coin.into()).ok_or(AmmError::CheckedMulOverflow)?;
        let denominator = self.token_pc.into();

        let amount_out_u128 = if round_direction == RoundDirection::Floor {
            numerator.checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        } else {
            numerator.checked_ceil_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        };
        
        Self::to_u64(amount_out_u128.as_u128())
    }
}

/// The invariant calculator for pool token conversions.
pub struct InvariantPool {
    /// Token input amount
    pub token_input: u64,
    /// Token total supply/amount
    pub token_total: u64,
}

impl InvariantPool {
    /// Exchange rate: converts pool total amount to token input amount.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn exchange_pool_to_token(
        &self,
        token_total_amount: u64,
        round_direction: RoundDirection,
    ) -> Result<u64, AmmError> {
        let numerator = U128::from(token_total_amount).checked_mul(self.token_input.into()).ok_or(AmmError::CheckedMulOverflow)?;
        let denominator = self.token_total.into();

        let amount_out_u128 = if round_direction == RoundDirection::Floor {
            numerator.checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        } else {
            numerator.checked_ceil_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        };

        Self::to_u64(amount_out_u128.as_u128())
    }
    
    /// Exchange rate: converts token amount to pool token amount.
    /// CRITICAL: Replaced unwraps with safe error handling.
    pub fn exchange_token_to_pool(
        &self,
        pool_total_amount: u64,
        round_direction: RoundDirection,
    ) -> Result<u64, AmmError> {
        let numerator = U128::from(pool_total_amount).checked_mul(self.token_input.into()).ok_or(AmmError::CheckedMulOverflow)?;
        let denominator = self.token_total.into();
        
        let amount_out_u128 = if round_direction == RoundDirection::Floor {
            numerator.checked_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        } else {
            numerator.checked_ceil_div(denominator).ok_or(AmmError::CheckedDivByZero)?
        };
        
        Self::to_u64(amount_out_u128.as_u128())
    }
}

pub trait CheckedCeilDiv: Sized {
    /// Perform ceiling division
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self>;
}

impl CheckedCeilDiv for u128 {
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self> {
        let mut quotient = self.checked_div(rhs)?;
        let remainder = self.checked_rem(rhs)?;
        if remainder != 0 {
            quotient = quotient.checked_add(1)?;
        }
        Some(quotient)
    }
}

impl CheckedCeilDiv for U128 {
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self> {
        let mut quotient = self.checked_div(rhs)?;
        let remainder = self.checked_rem(rhs)?;
        if remainder != U128::zero() {
            quotient = quotient.checked_add(U128::one())?;
        }
        Some(quotient)
    }
}
