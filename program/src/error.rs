//! Error types

use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    msg,
    program_error::{PrintProgramError, ProgramError},
};
use thiserror::Error;

/// Errors that may be returned by the TokenAmm program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum AmmError {
    // General errors
    #[error("Already in use.")]
    AlreadyInUse,
    #[error("Invalid program address.")]
    InvalidProgramAddress,
    #[error("Expected mint.")]
    ExpectedMint,
    #[error("Expected account.")]
    ExpectedAccount,
    #[error("Invalid coin vault.")]
    InvalidCoinVault,

    // Vault-related errors
    #[error("Invalid PC vault.")]
    InvalidPCVault,
    #[error("Invalid token LP.")]
    InvalidTokenLP,
    #[error("Invalid destination token coin.")]
    InvalidDestTokenCoin,
    #[error("Invalid destination token PC.")]
    InvalidDestTokenPC,
    #[error("Invalid pool mint.")]
    InvalidPoolMint,

    // Account-related errors
    #[error("Invalid open orders.")]
    InvalidOpenOrders,
    #[error("Invalid market.")]
    InvalidMarket,
    #[error("Invalid market program.")]
    InvalidMarketProgram,
    #[error("Invalid target orders.")]
    InvalidTargetOrders,
    #[error("Account must be writable.")]
    AccountNeedWriteable,

    #[error("Account must be read-only.")]
    AccountNeedReadOnly,
    #[error("Invalid coin mint.")]
    InvalidCoinMint,
    #[error("Invalid PC mint.")]
    InvalidPCMint,
    #[error("Invalid owner.")]
    InvalidOwner,
    #[error("Invalid supply.")]
    InvalidSupply,

    // Specific validation errors
    #[error("Invalid delegate.")]
    InvalidDelegate,
    #[error("Invalid sign account.")]
    InvalidSignAccount,
    #[error("Invalid status.")]
    InvalidStatus,
    #[error("Invalid instruction.")]
    InvalidInstruction,
    #[error("Wrong accounts number.")]
    WrongAccountsNumber,

    // Configuration errors
    #[error("Invalid target account owner.")]
    InvalidTargetAccountOwner,
    #[error("Invalid target owner.")]
    InvalidTargetOwner,
    #[error("Invalid AMM account owner.")]
    InvalidAmmAccountOwner,
    #[error("Invalid parameter set.")]
    InvalidParamsSet,
    #[error("Invalid input.")]
    InvalidInput,

    // Computation errors
    #[error("Exceeded desired slippage limit.")]
    ExceededSlippage,
    #[error("Calculation exchange rate failed.")]
    CalculationExRateFailure,
    #[error("Checked subtraction overflow.")]
    CheckedSubOverflow,
    #[error("Checked addition overflow.")]
    CheckedAddOverflow,
    #[error("Checked multiplication overflow.")]
    CheckedMulOverflow,

    #[error("Checked division overflow.")]
    CheckedDivOverflow,
    #[error("Empty funds.")]
    CheckedEmptyFunds,
    #[error("P&L calculation error.")]
    CalcPnlError,
    #[error("Invalid SPL token program.")]
    InvalidSplTokenProgram,
    #[error("Take P&L error.")]
    TakePnlError,

    // Miscellaneous errors
    #[error("Insufficient funds.")]
    InsufficientFunds,
    #[error("Conversion to u64 failed with overflow or underflow.")]
    ConversionFailure,
    #[error("User token input does not match AMM.")]
    InvalidUserToken,
    #[error("Invalid SRM mint.")]
    InvalidSrmMint,
    #[error("Invalid SRM token.")]
    InvalidSrmToken,

    #[error("Too many open orders.")]
    TooManyOpenOrders,
    #[error("Order at slot is already placed.")]
    OrderAtSlotIsPlaced,
    #[error("Invalid system program address.")]
    InvalidSysProgramAddress,
    #[error("Invalid fee.")]
    InvalidFee,
    #[error("Repeat AMM creation for the market.")]
    RepeatCreateAmm,

    #[error("Zero LP not allowed.")]
    NotAllowZeroLP,
    #[error("Token account has a close authority.")]
    InvalidCloseAuthority,
    #[error("Pool token mint has a freeze authority.")]
    InvalidFreezeAuthority,
    #[error("Invalid referrer PC mint.")]
    InvalidReferPCMint,
    #[error("Invalid configuration account.")]
    InvalidConfigAccount,

    #[error("Repeat configuration account creation.")]
    RepeatCreateConfigAccount,
    #[error("Market lot size is too large.")]
    MarketLotSizeIsTooLarge,
    #[error("Initial LP amount is too low.")]
    InitLpAmountTooLess,
    #[error("Unknown AMM error.")]
    UnknownAmmError,
}

impl From<AmmError> for ProgramError {
    fn from(e: AmmError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for AmmError {
    fn type_of() -> &'static str {
        "Amm Error"
    }
}

impl PrintProgramError for AmmError {
    fn print<E>(&self)
    where
        E: 'static
            + std::error::Error
            + DecodeError<E>
            + PrintProgramError
            + num_traits::FromPrimitive,
    {
        msg!("Error: {:?}", self);
    }
}
