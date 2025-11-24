use arrform::{arrform, ArrForm};
use serde::{Deserialize, Serialize};
use solana_program::{
    msg,
    // entrypoint::ProgramResult, // ProgramResult is not used here, so it is commented out
    pubkey::Pubkey,
    program_error::ProgramError, // Used for cleaner return types (if macro were to be used externally)
};

// Define a safe, fixed buffer size for structured logging (Solana has limits on message length)
pub const LOG_SIZE: usize = 256;

/**
 * @macro check_assert_eq
 * @brief Checks if input and expected Pubkeys are equal. If not, logs the mismatch 
 * using base64 and returns the specified ProgramError.
 */
#[macro_export]
macro_rules! check_assert_eq {
    ($input:expr, $expected:expr, $msg:expr, $err:expr) => {
        if $input != $expected {
            // Log the mismatch of the two Pubkeys for easy debugging
            log_keys_mismatch(concat!($msg, " mismatch:"), $input, $expected);
            return Err($err.into());
        }
    };
}

/// Logs the mismatch details (Pubkey inputs and expected values) during an assertion failure.
pub fn log_keys_mismatch(msg: &str, input: Pubkey, expected: Pubkey) {
    msg!(arrform!(
        LOG_SIZE,
        "ray_log: {} input:{}, expected:{}",
        msg,
        input,
        expected
    )
    .as_str());
}

/// LogType enum defines the type of event being logged (e.g., Init, Deposit, Swap).
#[derive(Debug, Clone, Copy)]
pub enum LogType {
    Init,
    Deposit,
    Withdraw,
    SwapBaseIn,
    SwapBaseOut,
}

impl LogType {
    /// Converts a u8 discriminant into a LogType. Panics if the discriminant is invalid.
    pub fn from_u8(log_type: u8) -> Self {
        match log_type {
            0 => LogType::Init,
            1 => LogType::Deposit,
            2 => LogType::Withdraw,
            3 => LogType::SwapBaseIn,
            4 => LogType::SwapBaseOut,
            // Changed unreachable!() to panic!() for safer handling of unexpected external data
            _ => panic!("Invalid LogType discriminant: {}", log_type),
        }
    }

    /// Converts a LogType into its u8 discriminant for serialization.
    pub fn into_u8(&self) -> u8 {
        match self {
            LogType::Init => 0u8,
            LogType::Deposit => 1u8,
            LogType::Withdraw => 2u8,
            LogType::SwapBaseIn => 3u8,
            LogType::SwapBaseOut => 4u8,
        }
    }
}

// --- Specific Log Structure Definitions (for AMM events) ---

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InitLog {
    pub log_type: u8,
    pub time: u64,
    pub pc_decimals: u8,
    pub coin_decimals: u8,
    pub pc_lot_size: u64,
    pub coin_lot_size: u64,
    pub pc_amount: u64,
    pub coin_amount: u64,
    pub market: Pubkey,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DepositLog {
    pub log_type: u8,
    // input
    pub max_coin: u64,
    pub max_pc: u64,
    pub base: u64,
    // pool info
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub pool_lp: u64,
    pub calc_pnl_x: u128,
    pub calc_pnl_y: u128,
    // calc result
    pub deduct_coin: u64,
    pub deduct_pc: u64,
    pub mint_lp: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WithdrawLog {
    pub log_type: u8,
    // input
    pub withdraw_lp: u64,
    // user info
    pub user_lp: u64,
    // pool info
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub pool_lp: u64,
    pub calc_pnl_x: u128,
    pub calc_pnl_y: u128,
    // calc result
    pub out_coin: u64,
    pub out_pc: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SwapBaseInLog {
    pub log_type: u8,
    // input
    pub amount_in: u64,
    pub minimum_out: u64,
    pub direction: u64,
    // user info
    pub user_source: u64,
    // pool info
    pub pool_coin: u64,
    pub pool_pc: u64,
    // calc result
    pub out_amount: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SwapBaseOutLog {
    pub log_type: u8,
    // input
    pub max_in: u64,
    pub amount_out: u64,
    pub direction: u64,
    // user info
    pub user_source: u64,
    // pool info
    pub pool_coin: u64,
    pub pool_pc: u64,
    // calc result
    pub deduct_in: u64,
}

/**
 * @function encode_ray_log
 * @brief Serializes a log struct (T) using bincode, encodes it to base64, 
 * and emits it on-chain using solana_program::msg!
 * @param log The serializable log struct.
 */
pub fn encode_ray_log<T: Serialize>(log: T) {
    // 1. Serialize struct using bincode
    let bytes = bincode::serialize(&log).unwrap();
    
    // 2. Allocate buffer for base64 encoding (4/3 multiplier + padding tolerance)
    let mut out_buf = Vec::new();
    out_buf.resize(bytes.len() * 4 / 3 + 4, 0);
    
    // 3. Encode binary data to base64 string slice
    let bytes_written = base64::encode_config_slice(bytes, base64::STANDARD, &mut out_buf);
    out_buf.resize(bytes_written, 0);
    
    // 4. Convert slice to string (unsafe is fine here since it comes from base64 encoding)
    let msg_str = unsafe { std::str::from_utf8_unchecked(&out_buf) };
    
    // 5. Emit the final message on-chain
    msg!(arrform!(LOG_SIZE, "ray_log: {}", msg_str).as_str());
}

/**
 * @function decode_ray_log
 * @brief Decodes a base64 log string into the appropriate structured log struct.
 * @param log The base64 encoded log string (usually read from transaction metadata).
 */
pub fn decode_ray_log(log: &str) {
    // 1. Decode base64 string back to binary
    let bytes = base64::decode_config(log, base64::STANDARD).unwrap();
    
    // 2. Use the first byte as the discriminant to determine the struct type
    match LogType::from_u8(bytes[0]) {
        LogType::Init => {
            let log: InitLog = bincode::deserialize(&bytes).unwrap();
            println!("{:?}", log);
        }
        LogType::Deposit => {
            let log: DepositLog = bincode::deserialize(&bytes).unwrap();
            println!("{:?}", log);
        }
        LogType::Withdraw => {
            let log: WithdrawLog = bincode::deserialize(&bytes).unwrap();
            println!("{:?}", log);
        }
        LogType::SwapBaseIn => {
            let log: SwapBaseInLog = bincode::deserialize(&bytes).unwrap();
            println!("{:?}", log);
        }
        LogType::SwapBaseOut => {
            let log: SwapBaseOutLog = bincode::deserialize(&bytes).unwrap();
            println!("{:?}", log);
        }
    }
}
