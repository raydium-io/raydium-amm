use arrform::{arrform, ArrForm};
use serde::{Deserialize, Serialize};
use solana_program::{
    msg,
    pubkey::Pubkey,
};

pub const LOG_SIZE: usize = 256;

#[macro_export]
macro_rules! check_assert_eq {
    ($input:expr, $expected:expr, $msg:expr, $err:expr) => {
        if $input != $expected {
            log_keys_mismatch(concat!($msg, " mismatch:"), $input, $expected);
            return Err($err.into());
        }
    };
}

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

#[derive(Debug)]
pub enum LogType {
    Init,
    Deposit,
    Withdraw,
    SwapBaseIn,
    SwapBaseOut,
}

impl LogType {
    pub fn from_u8(log_type: u8) -> Self {
        match log_type {
            0 => LogType::Init,
            1 => LogType::Deposit,
            2 => LogType::Withdraw,
            3 => LogType::SwapBaseIn,
            4 => LogType::SwapBaseOut,
            _ => {
                msg!("ray_log: Unknown log type: {}");
                LogType::Init // Default to Init for safety
            }
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            LogType::Init => 0u8,
            LogType::Deposit => 1u8,
            LogType::Withdraw => 2u8,
            LogType::SwapBaseIn => 3u8,
            LogType::SwapBaseOut => 4u8,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InitLog {
    pub log_type: u8,
    pub timestamp: u64,
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
    pub max_coin: u64,
    pub max_pc: u64,
    pub base: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub pool_lp: u64,
    pub calc_pnl_x: u128,
    pub calc_pnl_y: u128,
    pub deduct_coin: u64,
    pub deduct_pc: u64,
    pub mint_lp: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WithdrawLog {
    pub log_type: u8,
    pub withdraw_lp: u64,
    pub user_lp: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub pool_lp: u64,
    pub calc_pnl_x: u128,
    pub calc_pnl_y: u128,
    pub out_coin: u64,
    pub out_pc: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SwapBaseInLog {
    pub log_type: u8,
    pub amount_in: u64,
    pub minimum_out: u64,
    pub direction: u64,
    pub user_source: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub out_amount: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SwapBaseOutLog {
    pub log_type: u8,
    pub max_in: u64,
    pub amount_out: u64,
    pub direction: u64,
    pub user_source: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub deduct_in: u64,
}

pub fn encode_ray_log<T: Serialize>(log: &T) -> Result<String, &'static str> {
    bincode::serialize(log)
        .map(|bytes| base64::encode(bytes))
        .map_err(|_| "Serialization failed")
}

pub fn log_ray_log<T: Serialize>(log: &T) {
    match encode_ray_log(log) {
        Ok(encoded_log) => {
            msg!(arrform!(LOG_SIZE, "ray_log: {}", encoded_log).as_str());
        }
        Err(err) => {
            msg!("ray_log: Encoding failed: {}", err);
        }
    }
}

pub fn decode_ray_log(log: &str) {
    if let Ok(bytes) = base64::decode(log) {
        match LogType::from_u8(bytes[0]) {
            LogType::Init => {
                if let Ok(log) = bincode::deserialize::<InitLog>(&bytes) {
                    println!("{:?}", log);
                }
            }
            LogType::Deposit => {
                if let Ok(log) = bincode::deserialize::<DepositLog>(&bytes) {
                    println!("{:?}", log);
                }
            }
            LogType::Withdraw => {
                if let Ok(log) = bincode::deserialize::<WithdrawLog>(&bytes) {
                    println!("{:?}", log);
                }
            }
            LogType::SwapBaseIn => {
                if let Ok(log) = bincode::deserialize::<SwapBaseInLog>(&bytes) {
                    println!("{:?}", log);
                }
            }
            LogType::SwapBaseOut => {
                if let Ok(log) = bincode::deserialize::<SwapBaseOutLog>(&bytes) {
                    println!("{:?}", log);
                }
            }
        }
    } else {
        msg!("ray_log: Decoding failed");
    }
}
