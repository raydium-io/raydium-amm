//! State transition types

use crate::{error::AmmError, math::Calculator};
use serum_dex::state::ToAlignedBytes;
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use bytemuck::{from_bytes, from_bytes_mut, Pod, Zeroable};
use safe_transmute::{self, trivial::TriviallyTransmutable};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Ref, RefMut},
    convert::identity,
    convert::TryInto,
    mem::size_of,
};

pub const TEN_THOUSAND: u64 = 10000;
pub const MAX_ORDER_LIMIT: usize = 10;

#[cfg(not(test))]
pub fn get_recent_epoch() -> Result<u64, ProgramError> {
    use solana_program::{clock::Clock, sysvar::Sysvar};
    Ok(Clock::get()?.epoch)
}

#[cfg(test)]
pub fn get_recent_epoch() -> Result<u64, ProgramError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / (2 * 24 * 3600))
}

pub trait Loadable: Pod {
    fn load_mut<'a>(account: &'a AccountInfo) -> Result<RefMut<'a, Self>, ProgramError> {
        // TODO verify if this checks for size
        Ok(RefMut::map(account.try_borrow_mut_data()?, |data| {
            from_bytes_mut(data)
        }))
    }
    fn load<'a>(account: &'a AccountInfo) -> Result<Ref<'a, Self>, ProgramError> {
        Ok(Ref::map(account.try_borrow_data()?, |data| {
            from_bytes(data)
        }))
    }

    fn load_from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        Ok(from_bytes(data))
    }
}

macro_rules! impl_loadable {
    ($type_name:ident) => {
        unsafe impl Zeroable for $type_name {}
        unsafe impl Pod for $type_name {}
        unsafe impl TriviallyTransmutable for $type_name {}
        impl Loadable for $type_name {}
    };
}
#[cfg_attr(feature = "client", derive(Debug))]
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct TargetOrder {
    pub price: u64,
    pub vol: u64,
}
#[cfg(target_endian = "little")]
unsafe impl Zeroable for TargetOrder {}
#[cfg(target_endian = "little")]
unsafe impl Pod for TargetOrder {}
#[cfg(target_endian = "little")]
unsafe impl TriviallyTransmutable for TargetOrder {}

#[cfg_attr(feature = "client", derive(Debug))]
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct TargetOrders {
    pub owner: [u64; 4],
    pub buy_orders: [TargetOrder; 50],
    pub padding1: [u64; 8],
    pub target_x: u128,
    pub target_y: u128,
    pub plan_x_buy: u128,
    pub plan_y_buy: u128,
    pub plan_x_sell: u128,
    pub plan_y_sell: u128,
    pub placed_x: u128,
    pub placed_y: u128,
    pub calc_pnl_x: u128,
    pub calc_pnl_y: u128,
    pub sell_orders: [TargetOrder; 50],
    pub padding2: [u64; 6],
    pub replace_buy_client_id: [u64; MAX_ORDER_LIMIT],
    pub replace_sell_client_id: [u64; MAX_ORDER_LIMIT],
    pub last_order_numerator: u64,
    pub last_order_denominator: u64,

    pub plan_orders_cur: u64,
    pub place_orders_cur: u64,

    pub valid_buy_order_num: u64,
    pub valid_sell_order_num: u64,

    pub padding3: [u64; 10],

    pub free_slot_bits: u128,
}
impl_loadable!(TargetOrders);

#[cfg(test)]
impl Default for TargetOrders {
    #[inline]
    fn default() -> TargetOrders {
        TargetOrders {
            owner: [0; 4],
            buy_orders: [TargetOrder::default(); 50],
            padding1: [0; 8],
            target_x: 0,
            target_y: 0,
            plan_x_buy: 0,
            plan_y_buy: 0,
            plan_x_sell: 0,
            plan_y_sell: 0,
            placed_x: 0,
            placed_y: 0,
            calc_pnl_x: 0,
            calc_pnl_y: 0,
            sell_orders: [TargetOrder::default(); 50],
            padding2: [0; 6],
            replace_buy_client_id: [0; MAX_ORDER_LIMIT],
            replace_sell_client_id: [0; MAX_ORDER_LIMIT],
            last_order_denominator: 0,
            last_order_numerator: 0,
            plan_orders_cur: 0,
            place_orders_cur: 0,
            valid_buy_order_num: 0,
            valid_sell_order_num: 0,
            padding3: [0; 10],
            free_slot_bits: std::u128::MAX,
        }
    }
}

impl TargetOrders {
    /// init
    #[inline]
    pub fn check_init(&mut self, x: u128, y: u128, owner: &Pubkey) -> Result<(), ProgramError> {
        if identity(self.owner) != Pubkey::default().to_aligned_bytes() {
            return Err(AmmError::AlreadyInUse.into());
        }
        self.owner = owner.to_aligned_bytes();
        self.last_order_numerator = 0; // 3
        self.last_order_denominator = 0; // 1

        self.plan_orders_cur = 0;
        self.place_orders_cur = 0;

        self.valid_buy_order_num = 0;
        self.valid_sell_order_num = 0;

        self.target_x = 0;
        self.target_y = 0;
        self.plan_x_buy = 0;
        self.plan_y_buy = 0;
        self.plan_x_sell = 0;
        self.plan_y_sell = 0;
        self.placed_x = 0;
        self.placed_y = 0;
        self.calc_pnl_x = x;
        self.calc_pnl_y = y;
        self.free_slot_bits = std::u128::MAX;
        Ok(())
    }

    /// load_mut_checked
    #[inline]
    pub fn load_mut_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
        owner: &Pubkey,
    ) -> Result<RefMut<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidTargetAccountOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load_mut(account)?;
        if identity(data.owner) != owner.to_aligned_bytes() {
            return Err(AmmError::InvalidTargetOwner.into());
        }
        Ok(data)
    }

    /// load_checked
    #[inline]
    pub fn load_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
        owner: &Pubkey,
    ) -> Result<Ref<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidTargetAccountOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load(account)?;
        if identity(data.owner) != owner.to_aligned_bytes() {
            return Err(AmmError::InvalidTargetOwner.into());
        }
        Ok(data)
    }
}

#[repr(u64)]
pub enum AmmStatus {
    Uninitialized = 0u64,
    Initialized = 1u64,
    Disabled = 2u64,
    WithdrawOnly = 3u64,
    // pool only can add or remove liquidity, can't swap and plan orders
    LiquidityOnly = 4u64,
    // pool only can add or remove liquidity and plan orders, can't swap
    OrderBookOnly = 5u64,
    // pool only can add or remove liquidity and swap, can't plan orders
    SwapOnly = 6u64,
    // pool status after created and will auto update to SwapOnly during swap after open_time
    WaitingTrade = 7u64,
}
impl AmmStatus {
    pub fn from_u64(status: u64) -> Self {
        match status {
            0u64 => AmmStatus::Uninitialized,
            1u64 => AmmStatus::Initialized,
            2u64 => AmmStatus::Disabled,
            3u64 => AmmStatus::WithdrawOnly,
            4u64 => AmmStatus::LiquidityOnly,
            5u64 => AmmStatus::OrderBookOnly,
            6u64 => AmmStatus::SwapOnly,
            7u64 => AmmStatus::WaitingTrade,
            _ => unreachable!(),
        }
    }

    pub fn into_u64(&self) -> u64 {
        match self {
            AmmStatus::Uninitialized => 0u64,
            AmmStatus::Initialized => 1u64,
            AmmStatus::Disabled => 2u64,
            AmmStatus::WithdrawOnly => 3u64,
            AmmStatus::LiquidityOnly => 4u64,
            AmmStatus::OrderBookOnly => 5u64,
            AmmStatus::SwapOnly => 6u64,
            AmmStatus::WaitingTrade => 7u64,
        }
    }
    pub fn valid_status(status: u64) -> bool {
        match status {
            1u64 | 2u64 | 3u64 | 4u64 | 5u64 | 6u64 | 7u64 => return true,
            _ => return false,
        }
    }

    pub fn deposit_permission(&self) -> bool {
        match self {
            AmmStatus::Uninitialized => false,
            AmmStatus::Initialized => true,
            AmmStatus::Disabled => false,
            AmmStatus::WithdrawOnly => false,
            AmmStatus::LiquidityOnly => true,
            AmmStatus::OrderBookOnly => true,
            AmmStatus::SwapOnly => true,
            AmmStatus::WaitingTrade => true,
        }
    }

    pub fn withdraw_permission(&self) -> bool {
        match self {
            AmmStatus::Uninitialized => false,
            AmmStatus::Initialized => true,
            AmmStatus::Disabled => false,
            AmmStatus::WithdrawOnly => true,
            AmmStatus::LiquidityOnly => true,
            AmmStatus::OrderBookOnly => true,
            AmmStatus::SwapOnly => true,
            AmmStatus::WaitingTrade => true,
        }
    }

    pub fn swap_permission(&self) -> bool {
        match self {
            AmmStatus::Uninitialized => false,
            AmmStatus::Initialized => true,
            AmmStatus::Disabled => false,
            AmmStatus::WithdrawOnly => false,
            AmmStatus::LiquidityOnly => false,
            AmmStatus::OrderBookOnly => false,
            AmmStatus::SwapOnly => true,
            AmmStatus::WaitingTrade => true,
        }
    }

    pub fn orderbook_permission(&self) -> bool {
        match self {
            AmmStatus::Uninitialized => false,
            AmmStatus::Initialized => true,
            AmmStatus::Disabled => false,
            AmmStatus::WithdrawOnly => false,
            AmmStatus::LiquidityOnly => false,
            AmmStatus::OrderBookOnly => true,
            AmmStatus::SwapOnly => false,
            AmmStatus::WaitingTrade => false,
        }
    }
}

#[repr(u64)]
pub enum AmmState {
    InvlidState = 0u64,
    IdleState = 1u64,
    CancelAllOrdersState = 2u64,
    PlanOrdersState = 3u64,
    CancelOrderState = 4u64,
    PlaceOrdersState = 5u64,
    PurgeOrderState = 6u64,
}
impl AmmState {
    pub fn from_u64(state: u64) -> Self {
        match state {
            0u64 => AmmState::InvlidState,
            1u64 => AmmState::IdleState,
            2u64 => AmmState::CancelAllOrdersState,
            3u64 => AmmState::PlanOrdersState,
            4u64 => AmmState::CancelOrderState,
            5u64 => AmmState::PlaceOrdersState,
            6u64 => AmmState::PurgeOrderState,
            _ => unreachable!(),
        }
    }

    pub fn into_u64(&self) -> u64 {
        match self {
            AmmState::InvlidState => 0u64,
            AmmState::IdleState => 1u64,
            AmmState::CancelAllOrdersState => 2u64,
            AmmState::PlanOrdersState => 3u64,
            AmmState::CancelOrderState => 4u64,
            AmmState::PlaceOrdersState => 5u64,
            AmmState::PurgeOrderState => 6u64,
        }
    }
    pub fn valid_state(state: u64) -> bool {
        match state {
            0u64 | 1u64 | 2u64 | 3u64 | 4u64 | 5u64 | 6u64 => return true,
            _ => return false,
        }
    }
}

#[cfg_attr(feature = "client", derive(Debug))]
#[derive(Copy, Clone)]
#[repr(u64)]
pub enum AmmParams {
    Status = 0u64,
    State = 1u64,
    OrderNum = 2u64,
    Depth = 3u64,
    AmountWave = 4u64,
    MinPriceMultiplier = 5u64,
    MaxPriceMultiplier = 6u64,
    MinSize = 7u64,
    VolMaxCutRatio = 8u64,
    Fees = 9u64,
    AmmOwner = 10u64,
    SetOpenTime = 11u64,
    LastOrderDistance = 12u64,
    InitOrderDepth = 13u64,
    SetSwitchTime = 14u64,
    ClearOpenTime = 15u64,
    Seperate = 16u64,
    UpdateOpenOrder = 17u64,
}
impl AmmParams {
    pub fn from_u64(state: u64) -> Self {
        match state {
            0u64 => AmmParams::Status,
            1u64 => AmmParams::State,
            2u64 => AmmParams::OrderNum,
            3u64 => AmmParams::Depth,
            4u64 => AmmParams::AmountWave,
            5u64 => AmmParams::MinPriceMultiplier,
            6u64 => AmmParams::MaxPriceMultiplier,
            7u64 => AmmParams::MinSize,
            8u64 => AmmParams::VolMaxCutRatio,
            9u64 => AmmParams::Fees,
            10u64 => AmmParams::AmmOwner,
            11u64 => AmmParams::SetOpenTime,
            12u64 => AmmParams::LastOrderDistance,
            13u64 => AmmParams::InitOrderDepth,
            14u64 => AmmParams::SetSwitchTime,
            15u64 => AmmParams::ClearOpenTime,
            16u64 => AmmParams::Seperate,
            17u64 => AmmParams::UpdateOpenOrder,
            _ => unreachable!(),
        }
    }

    pub fn into_u64(&self) -> u64 {
        match self {
            AmmParams::Status => 0u64,
            AmmParams::State => 1u64,
            AmmParams::OrderNum => 2u64,
            AmmParams::Depth => 3u64,
            AmmParams::AmountWave => 4u64,
            AmmParams::MinPriceMultiplier => 5u64,
            AmmParams::MaxPriceMultiplier => 6u64,
            AmmParams::MinSize => 7u64,
            AmmParams::VolMaxCutRatio => 8u64,
            AmmParams::Fees => 9u64,
            AmmParams::AmmOwner => 10u64,
            AmmParams::SetOpenTime => 11u64,
            AmmParams::LastOrderDistance => 12u64,
            AmmParams::InitOrderDepth => 13u64,
            AmmParams::SetSwitchTime => 14u64,
            AmmParams::ClearOpenTime => 15u64,
            AmmParams::Seperate => 16u64,
            AmmParams::UpdateOpenOrder => 17u64,
        }
    }
}

#[cfg_attr(feature = "client", derive(Debug))]
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u64)]
pub enum AmmResetFlag {
    ResetYes = 0u64,
    ResetNo = 1u64,
}
impl AmmResetFlag {
    pub fn from_u64(flag: u64) -> Self {
        match flag {
            0u64 => AmmResetFlag::ResetYes,
            1u64 => AmmResetFlag::ResetNo,
            _ => unreachable!(),
        }
    }

    pub fn into_u64(&self) -> u64 {
        match self {
            AmmResetFlag::ResetYes => 0u64,
            AmmResetFlag::ResetNo => 1u64,
        }
    }
}

fn validate_fraction(numerator: u64, denominator: u64) -> Result<(), AmmError> {
    if numerator >= denominator || denominator == 0 {
        Err(AmmError::InvalidFee)
    } else {
        Ok(())
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fees {
    /// numerator of the min_separate
    pub min_separate_numerator: u64,
    /// denominator of the min_separate
    pub min_separate_denominator: u64,

    /// numerator of the fee
    pub trade_fee_numerator: u64,
    /// denominator of the fee
    /// and 'trade_fee_denominator' must be equal to 'min_separate_denominator'
    pub trade_fee_denominator: u64,

    /// numerator of the pnl
    pub pnl_numerator: u64,
    /// denominator of the pnl
    pub pnl_denominator: u64,

    /// numerator of the swap_fee
    pub swap_fee_numerator: u64,
    /// denominator of the swap_fee
    pub swap_fee_denominator: u64,
}

impl Fees {
    /// Validate that the fees are reasonable
    pub fn validate(&self) -> Result<(), AmmError> {
        validate_fraction(self.min_separate_numerator, self.min_separate_denominator)?;
        validate_fraction(self.trade_fee_numerator, self.trade_fee_denominator)?;
        validate_fraction(self.pnl_numerator, self.pnl_denominator)?;
        validate_fraction(self.swap_fee_numerator, self.swap_fee_denominator)?;
        Ok(())
    }

    pub fn initialize(&mut self) -> Result<(), AmmError> {
        // min_separate = 5/10000
        self.min_separate_numerator = 5;
        self.min_separate_denominator = TEN_THOUSAND;
        // trade_fee = 25/10000
        self.trade_fee_numerator = 25;
        self.trade_fee_denominator = TEN_THOUSAND;
        // pnl = 12/100
        self.pnl_numerator = 12;
        self.pnl_denominator = 100;
        // swap_fee = 25 / 10000
        self.swap_fee_numerator = 25;
        self.swap_fee_denominator = TEN_THOUSAND;
        Ok(())
    }
}

/// IsInitialized is required to use `Pack::pack` and `Pack::unpack`
impl IsInitialized for Fees {
    fn is_initialized(&self) -> bool {
        true
    }
}

impl Sealed for Fees {}
impl Pack for Fees {
    const LEN: usize = 64;
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 64];
        let (
            min_separate_numerator,
            min_separate_denominator,
            trade_fee_numerator,
            trade_fee_denominator,
            pnl_numerator,
            pnl_denominator,
            swap_fee_numerator,
            swap_fee_denominator,
        ) = mut_array_refs![output, 8, 8, 8, 8, 8, 8, 8, 8];
        *min_separate_numerator = self.min_separate_numerator.to_le_bytes();
        *min_separate_denominator = self.min_separate_denominator.to_le_bytes();
        *trade_fee_numerator = self.trade_fee_numerator.to_le_bytes();
        *trade_fee_denominator = self.trade_fee_denominator.to_le_bytes();
        *pnl_numerator = self.pnl_numerator.to_le_bytes();
        *pnl_denominator = self.pnl_denominator.to_le_bytes();
        *swap_fee_numerator = self.swap_fee_numerator.to_le_bytes();
        *swap_fee_denominator = self.swap_fee_denominator.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Fees, ProgramError> {
        let input = array_ref![input, 0, 64];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            min_separate_numerator,
            min_separate_denominator,
            trade_fee_numerator,
            trade_fee_denominator,
            pnl_numerator,
            pnl_denominator,
            swap_fee_numerator,
            swap_fee_denominator,
        ) = array_refs![input, 8, 8, 8, 8, 8, 8, 8, 8];
        Ok(Self {
            min_separate_numerator: u64::from_le_bytes(*min_separate_numerator),
            min_separate_denominator: u64::from_le_bytes(*min_separate_denominator),
            trade_fee_numerator: u64::from_le_bytes(*trade_fee_numerator),
            trade_fee_denominator: u64::from_le_bytes(*trade_fee_denominator),
            pnl_numerator: u64::from_le_bytes(*pnl_numerator),
            pnl_denominator: u64::from_le_bytes(*pnl_denominator),
            swap_fee_numerator: u64::from_le_bytes(*swap_fee_numerator),
            swap_fee_denominator: u64::from_le_bytes(*swap_fee_denominator),
        })
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StateData {
    /// delay to take pnl coin
    pub need_take_pnl_coin: u64,
    /// delay to take pnl pc
    pub need_take_pnl_pc: u64,
    /// total pnl pc
    pub total_pnl_pc: u64,
    /// total pnl coin
    pub total_pnl_coin: u64,
    /// ido pool open time
    pub pool_open_time: u64,
    /// padding for future updates
    pub padding: [u64; 2],
    /// switch from orderbookonly to init
    pub orderbook_to_init_time: u64,

    /// swap coin in amount
    pub swap_coin_in_amount: u128,
    /// swap pc out amount
    pub swap_pc_out_amount: u128,
    /// charge pc as swap fee while swap pc to coin
    pub swap_acc_pc_fee: u64,

    /// swap pc in amount
    pub swap_pc_in_amount: u128,
    /// swap coin out amount
    pub swap_coin_out_amount: u128,
    /// charge coin as swap fee while swap coin to pc
    pub swap_acc_coin_fee: u64,
}

impl StateData {
    pub fn initialize(&mut self, open_time: u64) -> Result<(), AmmError> {
        self.need_take_pnl_coin = 0u64;
        self.need_take_pnl_pc = 0u64;
        self.total_pnl_pc = 0u64;
        self.total_pnl_coin = 0u64;
        self.pool_open_time = open_time;
        self.padding = Zeroable::zeroed();
        self.orderbook_to_init_time = 0u64;
        self.swap_coin_in_amount = 0u128;
        self.swap_pc_out_amount = 0u128;
        self.swap_acc_pc_fee = 0u64;
        self.swap_pc_in_amount = 0u128;
        self.swap_coin_out_amount = 0u128;
        self.swap_acc_coin_fee = 0u64;

        Ok(())
    }
}

#[cfg_attr(feature = "client", derive(Debug))]
#[repr(C, packed)]
#[derive(Clone, Copy, Default, PartialEq)]
pub struct AmmInfo {
    /// Initialized status.
    pub status: u64,
    /// Nonce used in program address.
    /// The program address is created deterministically with the nonce,
    /// amm program id, and amm account pubkey.  This program address has
    /// authority over the amm's token coin account, token pc account, and pool
    /// token mint.
    pub nonce: u64,
    /// max order count
    pub order_num: u64,
    /// within this range, 5 => 5% range
    pub depth: u64,
    /// coin decimal
    pub coin_decimals: u64,
    /// pc decimal
    pub pc_decimals: u64,
    /// amm machine state
    pub state: u64,
    /// amm reset_flag
    pub reset_flag: u64,
    /// min size 1->0.000001
    pub min_size: u64,
    /// vol_max_cut_ratio numerator, sys_decimal_value as denominator
    pub vol_max_cut_ratio: u64,
    /// amount wave numerator, sys_decimal_value as denominator
    pub amount_wave: u64,
    /// coinLotSize 1 -> 0.000001
    pub coin_lot_size: u64,
    /// pcLotSize 1 -> 0.000001
    pub pc_lot_size: u64,
    /// min_cur_price: (2 * amm.order_num * amm.pc_lot_size) * max_price_multiplier
    pub min_price_multiplier: u64,
    /// max_cur_price: (2 * amm.order_num * amm.pc_lot_size) * max_price_multiplier
    pub max_price_multiplier: u64,
    /// system decimal value, used to normalize the value of coin and pc amount
    pub sys_decimal_value: u64,
    /// All fee information
    pub fees: Fees,
    /// Statistical data
    pub state_data: StateData,
    /// Coin vault
    pub coin_vault: Pubkey,
    /// Pc vault
    pub pc_vault: Pubkey,
    /// Coin vault mint
    pub coin_vault_mint: Pubkey,
    /// Pc vault mint
    pub pc_vault_mint: Pubkey,
    /// lp mint
    pub lp_mint: Pubkey,
    /// open_orders key
    pub open_orders: Pubkey,
    /// market key
    pub market: Pubkey,
    /// market program key
    pub market_program: Pubkey,
    /// target_orders key
    pub target_orders: Pubkey,
    /// padding
    pub padding1: [u64; 8],
    /// amm owner key
    pub amm_owner: Pubkey,
    /// pool lp amount
    pub lp_amount: u64,
    /// client order id
    pub client_order_id: u64,
    /// recent epoch
    pub recent_epoch: u64,
    /// padding
    pub padding2: u64,
}
impl_loadable!(AmmInfo);

impl AmmInfo {
    /// Helper function to get the more efficient packed size of the struct
    /// load_mut_checked
    #[inline]
    pub fn load_mut_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<RefMut<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidAmmAccountOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load_mut(account)?;
        if data.status == AmmStatus::Uninitialized as u64 {
            return Err(AmmError::InvalidStatus.into());
        }
        Ok(data)
    }

    /// load_checked
    #[inline]
    pub fn load_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<Ref<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidAmmAccountOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load(account)?;
        if data.status == AmmStatus::Uninitialized as u64 {
            return Err(AmmError::InvalidStatus.into());
        }
        Ok(data)
    }

    pub fn initialize(
        &mut self,
        nonce: u8,
        open_time: u64,
        coin_decimals: u8,
        pc_decimals: u8,
        coin_lot_size: u64,
        pc_lot_size: u64,
    ) -> Result<(), AmmError> {
        self.fees.initialize()?;
        self.state_data.initialize(open_time)?;

        self.status = AmmStatus::Uninitialized.into_u64();
        self.nonce = nonce as u64;
        self.order_num = 7;
        self.depth = 3;
        self.coin_decimals = coin_decimals as u64;
        self.pc_decimals = pc_decimals as u64;
        self.state = AmmState::IdleState.into_u64();
        self.reset_flag = AmmResetFlag::ResetNo.into_u64();
        if pc_decimals > coin_decimals {
            self.sys_decimal_value = (10 as u64)
                .checked_pow(pc_decimals.try_into().unwrap())
                .unwrap();
        } else {
            self.sys_decimal_value = (10 as u64)
                .checked_pow(coin_decimals.try_into().unwrap())
                .unwrap();
        }
        let temp_value_numerator = (coin_lot_size as u128)
            .checked_mul(
                (10 as u128)
                    .checked_pow(pc_decimals.try_into().unwrap())
                    .unwrap(),
            )
            .unwrap();
        let temp_value_denominator = (pc_lot_size as u128)
            .checked_mul(
                (10 as u128)
                    .checked_pow(coin_decimals.try_into().unwrap())
                    .unwrap(),
            )
            .unwrap();
        if (self.sys_decimal_value as u128)
            <= temp_value_numerator
                .checked_div(temp_value_denominator)
                .unwrap()
        {
            self.sys_decimal_value = Calculator::to_u64(
                temp_value_numerator
                    .checked_div(temp_value_denominator)
                    .unwrap(),
            )
            .unwrap();
        }
        let min_size = (coin_lot_size as u128)
            .checked_mul(self.sys_decimal_value as u128)
            .unwrap()
            .checked_div(
                (10u128)
                    .checked_pow(coin_decimals.try_into().unwrap())
                    .unwrap(),
            )
            .unwrap();
        if min_size < u64::max_value().into() {
            self.min_size = Calculator::to_u64(min_size)?;
        } else {
            // must check not zero in process_monitor_step
            self.min_size = 0;
        }
        self.vol_max_cut_ratio = 500; // TEN_THOUSAND as denominator
        self.amount_wave = self
            .sys_decimal_value
            .checked_mul(5)
            .unwrap()
            .checked_div(1000)
            .unwrap();
        self.coin_lot_size = coin_lot_size;
        self.pc_lot_size = Calculator::convert_in_pc_lot_size(
            pc_decimals,
            coin_decimals,
            pc_lot_size,
            coin_lot_size,
            self.sys_decimal_value,
        );
        self.min_price_multiplier = 1;
        self.max_price_multiplier = 1000000000;
        self.client_order_id = 0;
        self.padding1 = Zeroable::zeroed();
        self.recent_epoch = get_recent_epoch().unwrap();
        self.padding2 = Zeroable::zeroed();

        Ok(())
    }

    pub fn incr_client_order_id(&mut self) -> u64 {
        self.client_order_id = self.client_order_id.wrapping_add(1);
        if self.client_order_id == 0 {
            self.client_order_id += 1;
        }
        self.client_order_id
    }
}

/// State of amm config account
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AmmConfig {
    /// withdraw pnl owner
    pub pnl_owner: Pubkey,
    /// admin amm order owner
    pub cancel_owner: Pubkey,
    /// pending
    pub pending_1: [u64; 28],
    /// pending
    pub pending_2: [u64; 31],
    /// init amm pool fee amount
    pub create_pool_fee: u64,
}
impl_loadable!(AmmConfig);

impl AmmConfig {
    /// Helper function to get the more efficient packed size of the struct
    /// load_mut_checked
    #[inline]
    pub fn load_mut_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<RefMut<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load_mut(account)?;
        Ok(data)
    }

    /// load_checked
    #[inline]
    pub fn load_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<Ref<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load(account)?;
        Ok(data)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LastOrderDistance {
    pub last_order_numerator: u64,
    pub last_order_denominator: u64,
}

/// For simulateTransaction to get instruction data
#[cfg_attr(feature = "client", derive(Debug))]
#[derive(Copy, Clone)]
#[repr(u64)]
pub enum SimulateParams {
    PoolInfo = 0u64,
    SwapBaseInInfo = 1u64,
    SwapBaseOutInfo = 2u64,
    RunCrankInfo = 3u64,
}
impl SimulateParams {
    pub fn from_u64(flag: u64) -> Self {
        match flag {
            0u64 => SimulateParams::PoolInfo,
            1u64 => SimulateParams::SwapBaseInInfo,
            2u64 => SimulateParams::SwapBaseOutInfo,
            3u64 => SimulateParams::RunCrankInfo,
            _ => unreachable!(),
        }
    }

    pub fn into_u64(&self) -> u64 {
        match self {
            SimulateParams::PoolInfo => 0u64,
            SimulateParams::SwapBaseInInfo => 1u64,
            SimulateParams::SwapBaseOutInfo => 2u64,
            SimulateParams::RunCrankInfo => 3u64,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunCrankData {
    pub status: u64,
    pub state: u64,
    pub run_crank: bool,
}
impl RunCrankData {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
    pub fn from_json(data: &str) -> Self {
        serde_json::from_str(data).unwrap()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GetPoolData {
    pub status: u64,
    pub coin_decimals: u64,
    pub pc_decimals: u64,
    pub lp_decimals: u64,
    // pool token vault without pnl
    pub pool_pc_amount: u64,
    pub pool_coin_amount: u64,
    pub pnl_pc_amount: u64,
    pub pnl_coin_amount: u64,
    pub pool_lp_supply: u64,
    pub pool_open_time: u64,
    pub amm_id: String,
}
impl GetPoolData {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
    pub fn from_json(data: &str) -> Self {
        serde_json::from_str(data).unwrap()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GetSwapBaseInData {
    pub pool_data: GetPoolData,
    pub amount_in: u64,
    pub minimum_amount_out: u64,
    pub price_impact: u64,
}
impl GetSwapBaseInData {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
    pub fn from_json(data: &str) -> Self {
        serde_json::from_str(data).unwrap()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GetSwapBaseOutData {
    pub pool_data: GetPoolData,
    pub max_amount_in: u64,
    pub amount_out: u64,
    pub price_impact: u64,
}
impl GetSwapBaseOutData {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
    pub fn from_json(data: &str) -> Self {
        serde_json::from_str(data).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_amm_info_layout() {
        let status: u64 = 0x123456789abcdef0;
        let nonce: u64 = 0x123456789abcde0f;
        let order_num: u64 = 0x123456789abcd0ef;
        let depth: u64 = 0x123456789abc0def;
        let coin_decimals: u64 = 0x123456789ab0cdef;
        let pc_decimals: u64 = 0x123456789a0bcdef;
        let state: u64 = 0x1234567890abcdef;
        let reset_flag: u64 = 0x1234567809abcdef;
        let min_size: u64 = 0x1234567089abcdef;
        let vol_max_cut_ratio: u64 = 0x1234560789abcdef;
        let amount_wave: u64 = 0x1234506789abcdef;
        let coin_lot_size: u64 = 0x1234056789abcdef;
        let pc_lot_size: u64 = 0x1230456789abcdef;
        let min_price_multiplier: u64 = 0x1203456789abcdef;
        let max_price_multiplier: u64 = 0x1023456789abcdef;
        let sys_decimal_value: u64 = 0x123456789abcdfe0;

        let min_separate_numerator: u64 = 0x123456789abcfde0;
        let min_separate_denominator: u64 = 0x123456789abfcde0;
        let trade_fee_numerator: u64 = 0x123456789afbcde0;
        let trade_fee_denominator: u64 = 0x123456789fabcde0;
        let pnl_numerator: u64 = 0x12345678f9abcde0;
        let pnl_denominator: u64 = 0x1234567f89abcde0;
        let swap_fee_numerator: u64 = 0x123456f789abcde0;
        let swap_fee_denominator: u64 = 0x12345f6789abcde0;

        let need_take_pnl_coin: u64 = 0x1234f56789abcde0;
        let need_take_pnl_pc: u64 = 0x123f456789abcde0;
        let total_pnl_pc: u64 = 0x12f3456789abcde0;
        let total_pnl_coin: u64 = 0x1f23456789abcde0;
        let pool_open_time: u64 = 0x123456789abcedf0;
        let padding: [u64; 2] = [0x123456789abecdf0, 0x123456789aebcdf0];
        let orderbook_to_init_time: u64 = 0x123456789eabcdf0;
        let swap_coin_in_amount: u128 = 0x11002233445566778899aabbccddeeff;
        let swap_pc_out_amount: u128 = 0x11220033445566778899aabbccddeeff;
        let swap_acc_pc_fee: u64 = 0x12345678e9abcdf0;
        let swap_pc_in_amount: u128 = 0x11223300445566778899aabbccddeeff;
        let swap_coin_out_amount: u128 = 0x11223344005566778899aabbccddeeff;
        let swap_acc_coin_fee: u64 = 0x1234567e89abcdf0;

        let coin_vault = Pubkey::new_unique();
        let pc_vault = Pubkey::new_unique();
        let coin_vault_mint = Pubkey::new_unique();
        let pc_vault_mint = Pubkey::new_unique();
        let lp_mint = Pubkey::new_unique();
        let open_orders = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let market_program = Pubkey::new_unique();
        let target_orders = Pubkey::new_unique();

        let mut padding1: [u64; 8] = [0u64; 8];
        let mut padding1_data = [0u8; 8 * 8];
        let mut offset = 0;
        for i in 0..8 {
            padding1[i] = u64::MAX - i as u64;
            padding1_data[offset..offset + 8].copy_from_slice(&padding1[i].to_le_bytes());
            offset += 8;
        }
        let amm_owner = Pubkey::new_unique();
        let lp_amount: u64 = 0x123456e789abcdf0;
        let client_order_id: u64 = 0x12345e6789abcdf0;
        let recent_epoch: u64 = 0x1234e56789abcdf0;
        let padding2: u64 = 0x123e456789abcdf0;

        // serialize original data
        let mut pool_data = [0u8; 752];
        let mut offset = 0;
        pool_data[offset..offset + 8].copy_from_slice(&status.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&nonce.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&order_num.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&depth.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&coin_decimals.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&pc_decimals.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&state.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&reset_flag.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&min_size.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&vol_max_cut_ratio.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&amount_wave.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&coin_lot_size.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&pc_lot_size.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&min_price_multiplier.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&max_price_multiplier.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&sys_decimal_value.to_le_bytes());
        offset += 8;

        pool_data[offset..offset + 8].copy_from_slice(&min_separate_numerator.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&min_separate_denominator.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&trade_fee_numerator.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&trade_fee_denominator.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&pnl_numerator.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&pnl_denominator.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&swap_fee_numerator.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&swap_fee_denominator.to_le_bytes());
        offset += 8;

        pool_data[offset..offset + 8].copy_from_slice(&need_take_pnl_coin.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&need_take_pnl_pc.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&total_pnl_pc.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&total_pnl_coin.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&pool_open_time.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&padding[0].to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&padding[1].to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&orderbook_to_init_time.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 16].copy_from_slice(&swap_coin_in_amount.to_le_bytes());
        offset += 16;
        pool_data[offset..offset + 16].copy_from_slice(&swap_pc_out_amount.to_le_bytes());
        offset += 16;
        pool_data[offset..offset + 8].copy_from_slice(&swap_acc_pc_fee.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 16].copy_from_slice(&swap_pc_in_amount.to_le_bytes());
        offset += 16;
        pool_data[offset..offset + 16].copy_from_slice(&swap_coin_out_amount.to_le_bytes());
        offset += 16;
        pool_data[offset..offset + 8].copy_from_slice(&swap_acc_coin_fee.to_le_bytes());
        offset += 8;

        pool_data[offset..offset + 32].copy_from_slice(&coin_vault.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&pc_vault.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&coin_vault_mint.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&pc_vault_mint.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&lp_mint.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&open_orders.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&market.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&market_program.to_bytes());
        offset += 32;
        pool_data[offset..offset + 32].copy_from_slice(&target_orders.to_bytes());
        offset += 32;
        pool_data[offset..offset + 8 * 8].copy_from_slice(&padding1_data);
        offset += 8 * 8;
        pool_data[offset..offset + 32].copy_from_slice(&amm_owner.to_bytes());
        offset += 32;
        pool_data[offset..offset + 8].copy_from_slice(&lp_amount.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&client_order_id.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&recent_epoch.to_le_bytes());
        offset += 8;
        pool_data[offset..offset + 8].copy_from_slice(&padding2.to_le_bytes());
        offset += 8;

        // len check
        assert_eq!(offset, pool_data.len());
        assert_eq!(pool_data.len(), core::mem::size_of::<AmmInfo>());

        // deserialize original data
        let unpack_data: &AmmInfo =
            bytemuck::from_bytes(&pool_data[0..core::mem::size_of::<AmmInfo>()]);
        // data check
        let unpack_status = unpack_data.status;
        assert_eq!(status, unpack_status);
        let unpack_nonce = unpack_data.nonce;
        assert_eq!(nonce, unpack_nonce);
        let unpack_order_num = unpack_data.order_num;
        assert_eq!(order_num, unpack_order_num);
        let unpack_depth = unpack_data.depth;
        assert_eq!(depth, unpack_depth);
        let unpack_coin_decimals = unpack_data.coin_decimals;
        assert_eq!(coin_decimals, unpack_coin_decimals);
        let unpack_pc_decimals = unpack_data.pc_decimals;
        assert_eq!(pc_decimals, unpack_pc_decimals);
        let unpack_state = unpack_data.state;
        assert_eq!(state, unpack_state);
        let unpack_reset_flag = unpack_data.reset_flag;
        assert_eq!(reset_flag, unpack_reset_flag);
        let unpack_min_size = unpack_data.min_size;
        assert_eq!(min_size, unpack_min_size);
        let unpack_vol_max_cut_ratio = unpack_data.vol_max_cut_ratio;
        assert_eq!(vol_max_cut_ratio, unpack_vol_max_cut_ratio);
        let unpack_amount_wave = unpack_data.amount_wave;
        assert_eq!(amount_wave, unpack_amount_wave);
        let unpack_coin_lot_size = unpack_data.coin_lot_size;
        assert_eq!(coin_lot_size, unpack_coin_lot_size);
        let unpack_pc_lot_size = unpack_data.pc_lot_size;
        assert_eq!(pc_lot_size, unpack_pc_lot_size);
        let unpack_min_price_multiplier = unpack_data.min_price_multiplier;
        assert_eq!(min_price_multiplier, unpack_min_price_multiplier);
        let unpack_max_price_multiplier = unpack_data.max_price_multiplier;
        assert_eq!(max_price_multiplier, unpack_max_price_multiplier);
        let unpack_sys_decimal_value = unpack_data.sys_decimal_value;
        assert_eq!(sys_decimal_value, unpack_sys_decimal_value);
        let unpack_min_separate_numerator = unpack_data.fees.min_separate_numerator;
        assert_eq!(min_separate_numerator, unpack_min_separate_numerator);
        let unpack_min_separate_denominator = unpack_data.fees.min_separate_denominator;
        assert_eq!(min_separate_denominator, unpack_min_separate_denominator);
        let unpack_trade_fee_numerator = unpack_data.fees.trade_fee_numerator;
        assert_eq!(trade_fee_numerator, unpack_trade_fee_numerator);
        let unpack_trade_fee_denominator = unpack_data.fees.trade_fee_denominator;
        assert_eq!(trade_fee_denominator, unpack_trade_fee_denominator);
        let unpack_pnl_numerator = unpack_data.fees.pnl_numerator;
        assert_eq!(pnl_numerator, unpack_pnl_numerator);
        let unpack_pnl_denominator = unpack_data.fees.pnl_denominator;
        assert_eq!(pnl_denominator, unpack_pnl_denominator);
        let unpack_swap_fee_numerator = unpack_data.fees.swap_fee_numerator;
        assert_eq!(swap_fee_numerator, unpack_swap_fee_numerator);
        let unpack_swap_fee_denominator = unpack_data.fees.swap_fee_denominator;
        assert_eq!(swap_fee_denominator, unpack_swap_fee_denominator);
        let unpack_need_take_pnl_coin = unpack_data.state_data.need_take_pnl_coin;
        assert_eq!(need_take_pnl_coin, unpack_need_take_pnl_coin);
        let unpack_need_take_pnl_pc = unpack_data.state_data.need_take_pnl_pc;
        assert_eq!(need_take_pnl_pc, unpack_need_take_pnl_pc);
        let unpack_total_pnl_pc = unpack_data.state_data.total_pnl_pc;
        assert_eq!(total_pnl_pc, unpack_total_pnl_pc);
        let unpack_total_pnl_coin = unpack_data.state_data.total_pnl_coin;
        assert_eq!(total_pnl_coin, unpack_total_pnl_coin);
        let unpack_pool_open_time = unpack_data.state_data.pool_open_time;
        assert_eq!(pool_open_time, unpack_pool_open_time);
        for i in 0..2 {
            let unpack_padding = unpack_data.state_data.padding[i];
            assert_eq!(padding[i], unpack_padding);
        }
        let unpack_orderbook_to_init_time = unpack_data.state_data.orderbook_to_init_time;
        assert_eq!(orderbook_to_init_time, unpack_orderbook_to_init_time);
        let unpack_swap_coin_in_amount = unpack_data.state_data.swap_coin_in_amount;
        assert_eq!(swap_coin_in_amount, unpack_swap_coin_in_amount);
        let unpack_swap_pc_out_amount = unpack_data.state_data.swap_pc_out_amount;
        assert_eq!(swap_pc_out_amount, unpack_swap_pc_out_amount);
        let unpack_swap_acc_pc_fee = unpack_data.state_data.swap_acc_pc_fee;
        assert_eq!(swap_acc_pc_fee, unpack_swap_acc_pc_fee);
        let unpack_swap_pc_in_amount = unpack_data.state_data.swap_pc_in_amount;
        assert_eq!(swap_pc_in_amount, unpack_swap_pc_in_amount);
        let unpack_swap_coin_out_amount = unpack_data.state_data.swap_coin_out_amount;
        assert_eq!(swap_coin_out_amount, unpack_swap_coin_out_amount);
        let unpack_swap_acc_coin_fee = unpack_data.state_data.swap_acc_coin_fee;
        assert_eq!(swap_acc_coin_fee, unpack_swap_acc_coin_fee);
        let unpack_coin_vault = unpack_data.coin_vault;
        assert_eq!(coin_vault, unpack_coin_vault);
        let unpack_pc_vault = unpack_data.pc_vault;
        assert_eq!(pc_vault, unpack_pc_vault);
        let unpack_coin_vault_mint = unpack_data.coin_vault_mint;
        assert_eq!(coin_vault_mint, unpack_coin_vault_mint);
        let unpack_pc_vault_mint = unpack_data.pc_vault_mint;
        assert_eq!(pc_vault_mint, unpack_pc_vault_mint);
        let unpack_lp_mint = unpack_data.lp_mint;
        assert_eq!(lp_mint, unpack_lp_mint);
        let unpack_open_orders = unpack_data.open_orders;
        assert_eq!(open_orders, unpack_open_orders);
        let unpack_market = unpack_data.market;
        assert_eq!(market, unpack_market);
        let unpack_market_program = unpack_data.market_program;
        assert_eq!(market_program, unpack_market_program);
        let unpack_target_orders = unpack_data.target_orders;
        assert_eq!(target_orders, unpack_target_orders);
        for i in 0..8 {
            let unpack_padding1 = unpack_data.padding1[i];
            assert_eq!(padding1[i], unpack_padding1);
        }
        let unpack_amm_owner = unpack_data.amm_owner;
        assert_eq!(amm_owner, unpack_amm_owner);
        let unpack_lp_amount = unpack_data.lp_amount;
        assert_eq!(lp_amount, unpack_lp_amount);
        let unpack_client_order_id = unpack_data.client_order_id;
        assert_eq!(client_order_id, unpack_client_order_id);
        let unpack_recent_epoch = unpack_data.recent_epoch;
        assert_eq!(recent_epoch, unpack_recent_epoch);
        let unpack_padding2 = unpack_data.padding2;
        assert_eq!(padding2, unpack_padding2);
    }

    #[test]
    fn test_target_info_layout() {
        let owner: [u64; 4] = [
            0x123456789abcedf0,
            0x123456789abced0f,
            0x123456789abce0df,
            0x123456789abc0edf,
        ];
        let mut buy_orders: [TargetOrder; 50] = [TargetOrder::default(); 50];
        let mut buy_orders_data = [0u8; 8 * 2 * 50];
        let mut offset = 0;
        for i in 0..50 {
            buy_orders[i].price = u64::MAX - i as u64;
            buy_orders[i].vol = u64::MAX - 3 * i as u64;
            buy_orders_data[offset..offset + 8].copy_from_slice(&buy_orders[i].price.to_le_bytes());
            offset += 8;
            buy_orders_data[offset..offset + 8].copy_from_slice(&buy_orders[i].vol.to_le_bytes());
            offset += 8;
        }
        let mut padding1 = [0u64; 8];
        for i in 0..8 {
            padding1[i] = 1 << i;
        }
        let target_x: u128 = 0x11002233445566778899aabbccddeeff;
        let target_y: u128 = 0x11220033445566778899aabbccddeeff;
        let plan_x_buy: u128 = 0x11223300445566778899aabbccddeeff;
        let plan_y_buy: u128 = 0x11223344005566778899aabbccddeeff;
        let plan_x_sell: u128 = 0x11223344550066778899aabbccddeeff;
        let plan_y_sell: u128 = 0x11223344556600778899aabbccddeeff;
        let placed_x: u128 = 0x11223344556677008899aabbccddeeff;
        let placed_y: u128 = 0x11223344556677880099aabbccddeeff;
        let calc_pnl_x: u128 = 0x11223344556677889900aabbccddeeff;
        let calc_pnl_y: u128 = 0x112233445566778899aa00bbccddeeff;
        let mut sell_orders: [TargetOrder; 50] = [TargetOrder::default(); 50];
        let mut sell_orders_data = [0u8; 8 * 2 * 50];
        let mut offset = 0;
        for i in 0..50 {
            sell_orders[i].price = u64::MAX - (i + 50) as u64;
            sell_orders[i].vol = u64::MAX - 3 * (i + 50) as u64;
            sell_orders_data[offset..offset + 8]
                .copy_from_slice(&sell_orders[i].price.to_le_bytes());
            offset += 8;
            sell_orders_data[offset..offset + 8].copy_from_slice(&sell_orders[i].vol.to_le_bytes());
            offset += 8;
        }
        let mut padding2 = [0u64; 6];
        for i in 0..6 {
            padding2[i] = 1 << (i + 8);
        }
        let mut replace_buy_client_id = [0u64; MAX_ORDER_LIMIT];
        let mut replace_sell_client_id = [0u64; MAX_ORDER_LIMIT];
        for i in 0..MAX_ORDER_LIMIT {
            replace_buy_client_id[i] = 1 << (i + 8 + 6);
            replace_sell_client_id[i] = 1 << (i + 8 + 6 + MAX_ORDER_LIMIT);
        }
        let last_order_numerator: u64 = 0x123456789ab0cedf;
        let last_order_denominator: u64 = 0x123456789a0bcedf;
        let plan_orders_cur: u64 = 0x1234567890abcedf;
        let place_orders_cur: u64 = 0x1234567809abcedf;
        let valid_buy_order_num: u64 = 0x1234567089abcedf;
        let valid_sell_order_num: u64 = 0x1234560789abcedf;
        let mut padding3 = [0u64; 10];
        for i in 0..10 {
            padding3[i] = 1 << (i + 8 + 6 + MAX_ORDER_LIMIT + MAX_ORDER_LIMIT);
        }
        let free_slot_bits: u128 = 0x112233445566778899aabb00ccddeeff;

        // serialize original data
        let mut target_orders_data = [0u8; 2208];
        let mut offset = 0;
        for i in 0..4 {
            target_orders_data[offset..offset + 8].copy_from_slice(&owner[i].to_le_bytes());
            offset += 8;
        }
        target_orders_data[offset..offset + 8 * 2 * 50].copy_from_slice(&buy_orders_data);
        offset += 8 * 2 * 50;
        for i in 0..8 {
            target_orders_data[offset..offset + 8].copy_from_slice(&padding1[i].to_le_bytes());
            offset += 8;
        }
        target_orders_data[offset..offset + 16].copy_from_slice(&target_x.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&target_y.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&plan_x_buy.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&plan_y_buy.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&plan_x_sell.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&plan_y_sell.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&placed_x.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&placed_y.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&calc_pnl_x.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 16].copy_from_slice(&calc_pnl_y.to_le_bytes());
        offset += 16;
        target_orders_data[offset..offset + 8 * 2 * 50].copy_from_slice(&sell_orders_data);
        offset += 8 * 2 * 50;
        for i in 0..6 {
            target_orders_data[offset..offset + 8].copy_from_slice(&padding2[i].to_le_bytes());
            offset += 8;
        }
        for i in 0..MAX_ORDER_LIMIT {
            target_orders_data[offset..offset + 8]
                .copy_from_slice(&replace_buy_client_id[i].to_le_bytes());
            offset += 8;
        }
        for i in 0..MAX_ORDER_LIMIT {
            target_orders_data[offset..offset + 8]
                .copy_from_slice(&replace_sell_client_id[i].to_le_bytes());
            offset += 8;
        }
        target_orders_data[offset..offset + 8].copy_from_slice(&last_order_numerator.to_le_bytes());
        offset += 8;
        target_orders_data[offset..offset + 8]
            .copy_from_slice(&last_order_denominator.to_le_bytes());
        offset += 8;
        target_orders_data[offset..offset + 8].copy_from_slice(&plan_orders_cur.to_le_bytes());
        offset += 8;
        target_orders_data[offset..offset + 8].copy_from_slice(&place_orders_cur.to_le_bytes());
        offset += 8;
        target_orders_data[offset..offset + 8].copy_from_slice(&valid_buy_order_num.to_le_bytes());
        offset += 8;
        target_orders_data[offset..offset + 8].copy_from_slice(&valid_sell_order_num.to_le_bytes());
        offset += 8;
        for i in 0..10 {
            target_orders_data[offset..offset + 8].copy_from_slice(&padding3[i].to_le_bytes());
            offset += 8;
        }
        target_orders_data[offset..offset + 16].copy_from_slice(&free_slot_bits.to_le_bytes());
        offset += 16;

        // len check
        assert_eq!(offset, target_orders_data.len());
        assert_eq!(
            target_orders_data.len(),
            core::mem::size_of::<TargetOrders>()
        );

        // deserialize original data
        let unpack_data: &TargetOrders =
            bytemuck::from_bytes(&target_orders_data[0..core::mem::size_of::<TargetOrders>()]);
        // data check
        let unpack_owner = unpack_data.owner;
        for i in 0..4 {
            assert_eq!(owner[i], unpack_owner[i]);
        }
        let unpack_buy_orders = unpack_data.buy_orders;
        for i in 0..50 {
            let price = buy_orders[i].price;
            let unpack_price = unpack_buy_orders[i].price;
            assert_eq!(price, unpack_price);
            let vol = buy_orders[i].vol;
            let unpack_vol = unpack_buy_orders[i].vol;
            assert_eq!(vol, unpack_vol);
        }
        let unpack_padding1 = unpack_data.padding1;
        for i in 0..8 {
            assert_eq!(padding1[i], unpack_padding1[i]);
        }
        let unpack_target_x = unpack_data.target_x;
        assert_eq!(target_x, unpack_target_x);
        let unpack_target_y = unpack_data.target_y;
        assert_eq!(target_y, unpack_target_y);
        let unpack_plan_x_buy = unpack_data.plan_x_buy;
        assert_eq!(plan_x_buy, unpack_plan_x_buy);
        let unpack_plan_y_buy = unpack_data.plan_y_buy;
        assert_eq!(plan_y_buy, unpack_plan_y_buy);
        let unpack_plan_x_sell = unpack_data.plan_x_sell;
        assert_eq!(plan_x_sell, unpack_plan_x_sell);
        let unpack_plan_y_sell = unpack_data.plan_y_sell;
        assert_eq!(plan_y_sell, unpack_plan_y_sell);
        let unpack_placed_x = unpack_data.placed_x;
        assert_eq!(placed_x, unpack_placed_x);
        let unpack_placed_y = unpack_data.placed_y;
        assert_eq!(placed_y, unpack_placed_y);
        let unpack_calc_pnl_x = unpack_data.calc_pnl_x;
        assert_eq!(calc_pnl_x, unpack_calc_pnl_x);
        let unpack_calc_pnl_y = unpack_data.calc_pnl_y;
        assert_eq!(calc_pnl_y, unpack_calc_pnl_y);
        let unpack_sell_orders = unpack_data.sell_orders;
        for i in 0..50 {
            let price = sell_orders[i].price;
            let unpack_price = unpack_sell_orders[i].price;
            assert_eq!(price, unpack_price);
            let vol = sell_orders[i].vol;
            let unpack_vol = unpack_sell_orders[i].vol;
            assert_eq!(vol, unpack_vol);
        }
        let unpack_padding2 = unpack_data.padding2;
        for i in 0..6 {
            assert_eq!(padding2[i], unpack_padding2[i]);
        }
        let unpack_replace_buy_client_id = unpack_data.replace_buy_client_id;
        for i in 0..MAX_ORDER_LIMIT {
            assert_eq!(replace_buy_client_id[i], unpack_replace_buy_client_id[i]);
        }
        let unpack_replace_sell_client_id = unpack_data.replace_sell_client_id;
        for i in 0..MAX_ORDER_LIMIT {
            assert_eq!(replace_sell_client_id[i], unpack_replace_sell_client_id[i]);
        }
        let unpack_last_order_numerator = unpack_data.last_order_numerator;
        assert_eq!(last_order_numerator, unpack_last_order_numerator);
        let unpack_last_order_denominator = unpack_data.last_order_denominator;
        assert_eq!(last_order_denominator, unpack_last_order_denominator);
        let unpack_plan_orders_cur = unpack_data.plan_orders_cur;
        assert_eq!(plan_orders_cur, unpack_plan_orders_cur);
        let unpack_place_orders_cur = unpack_data.place_orders_cur;
        assert_eq!(place_orders_cur, unpack_place_orders_cur);
        let unpack_valid_buy_order_num = unpack_data.valid_buy_order_num;
        assert_eq!(valid_buy_order_num, unpack_valid_buy_order_num);
        let unpack_valid_sell_order_num = unpack_data.valid_sell_order_num;
        assert_eq!(valid_sell_order_num, unpack_valid_sell_order_num);
        let unpack_padding3 = unpack_data.padding3;
        for i in 0..10 {
            assert_eq!(padding3[i], unpack_padding3[i]);
        }
        let unpack_free_slot_bits = unpack_data.free_slot_bits;
        assert_eq!(free_slot_bits, unpack_free_slot_bits);
    }
}
