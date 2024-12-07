use anchor_lang::prelude::*;
use crate::errors::ErrorCode;

pub const TOTAL_SUPPLY: u64 = 1_000_000_000_000_000; // 1 billion tokens (with 6 decimals)
pub const CURVE_SUPPLY: u64 = 800_000_000_000_000;   // 800 million tokens (with 6 decimals)
pub const TOKENS_PER_STEP: u64 = 10_000_000_000_000; // 10M tokens (with 6 decimals)

pub struct BondingCurve;

impl BondingCurve {
    pub fn calculate_price(market_cap: u64) -> u64 {
        // y = 0.6015 * e^(0.00003606x)
        
        // Base price: 0.6015 SOL
        let base_price: u64 = 601_500_000; // 0.6015 SOL in lamports
        
        let exp_factor = Self::calculate_exp_factor(market_cap);
        
        base_price
            .checked_mul(exp_factor)
            .unwrap_or(u64::MAX)
            .checked_div(1_000_000)
            .unwrap_or(u64::MAX)
    }

    fn calculate_exp_factor(market_cap: u64) -> u64 {
        let x: u64 = market_cap
            .checked_mul(36060)
            .unwrap_or(u64::MAX)
            .checked_div(1_000_000)
            .unwrap_or(u64::MAX);

        let x_squared = x
            .checked_mul(x)
            .unwrap_or(u64::MAX)
            .checked_div(1_000_000)
            .unwrap_or(u64::MAX);

        let x_cubed = x_squared
            .checked_mul(x)
            .unwrap_or(u64::MAX)
            .checked_div(1_000_000)
            .unwrap_or(u64::MAX);

        let base: u64 = 1_000_000;
        base
            .checked_add(x)
            .unwrap_or(u64::MAX)
            .checked_add(x_squared.checked_div(2).unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX)
            .checked_add(x_cubed.checked_div(6).unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX)
    }

    pub fn calculate_tokens_out(sol_amount: u64, current_market_cap: u64) -> Result<u64> {
        let price = Self::calculate_price(current_market_cap);
        sol_amount
            .checked_mul(TOKENS_PER_STEP)
            .unwrap_or(u64::MAX)
            .checked_div(price)
            .ok_or(error!(ErrorCode::CalculationError))
    }

    pub fn calculate_sol_out(token_amount: u64, current_market_cap: u64) -> Result<u64> {
        let price = Self::calculate_price(current_market_cap);
        token_amount
            .checked_mul(price)
            .unwrap_or(u64::MAX)
            .checked_div(TOKENS_PER_STEP)
            .ok_or(error!(ErrorCode::CalculationError))
    }

    pub fn should_transition_to_raydium(tokens_sold: u64) -> bool {
        tokens_sold >= CURVE_SUPPLY
    }
} 