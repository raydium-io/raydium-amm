const TWAP_TIME_MIN: usize = 10;
pub const TICK_SECONDS: usize = 10; // Sample every 10s
const BUFFER_SIZE: usize = TWAP_TIME_MIN * 60 / TICK_SECONDS; // 60 slots

const PRECISION: u128 = 1_000_000_000_000; // 1e12

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwapBuffer {
    pub prices: [u128; BUFFER_SIZE],
    pub index: usize,
}

pub enum Volatility {
    Low,
    Medium,
    High,
}

impl Default for TwapBuffer {
    fn default() -> Self {
        TwapBuffer {
            index: 0,
            prices: [0; BUFFER_SIZE],
        }
    }
}

impl TwapBuffer {
    pub fn update_price(
        &mut self,
        total_pc_without_take_pnl: u64,
        total_coin_without_take_pnl: u64,
    ) {
        let price = (total_pc_without_take_pnl as u128)
            .checked_mul(PRECISION)
            .and_then(|p| p.checked_div(total_coin_without_take_pnl as u128))
            .unwrap_or(0); // Fallback for overflow/divide by 0

        self.prices[self.index] = price;
        self.index = (self.index + 1) % BUFFER_SIZE;
    }

    /// Returns the fee depending on the historical prices.
    ///
    /// We do not need a special case when bootstraping the prices, we just use the prices set to 0
    /// which is fine, because we can consider that a new pool is volatile during the first 10 minutes.
    pub fn fee(&self) -> Volatility {
        let twap = self.calc_twap();
        let current = self.current_price();

        let volatility = (current.abs_diff(twap) * 100_u128).checked_div(twap);

        match volatility {
            Some(v) if v < 50 => Volatility::Low,
            Some(v) if v < 150 => Volatility::Medium,
            None | Some(_) => Volatility::High,
        }
    }

    fn calc_twap(&self) -> u128 {
        // Safety: this obviously cannot overflow, because BUFFER_SIZE cannot exceed `u64::MAX`.
        let sum: u128 = self.prices.into_iter().map(u128::from).sum();

        // Safety: this cannot overflow , because even if every price is `u64::MAX`, the average is still `u64::MAX`.
        sum / BUFFER_SIZE as u128
    }

    fn current_price(&self) -> u128 {
        let last_index = if self.index == 0 {
            BUFFER_SIZE - 1
        } else {
            self.index - 1
        };

        self.prices[last_index]
    }
}
