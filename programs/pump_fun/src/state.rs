use anchor_lang::prelude::*;
use super::errors::ErrorCode;

#[account]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image_url: String,
    pub creator: Pubkey,
    pub creation_time: i64,
}

#[account]
pub struct BondingCurveParams {
    pub initial_price: u64,
    pub slope: u64,
    pub liquidity_target: u64,
    pub current_supply: u64,
    pub total_liquidity: u64,
    pub bump: u8,
}

impl TokenMetadata {
    pub const SIZE: usize = 32 + // name
                           10 + // symbol
                           200 + // description
                           200 + // image_url
                           32 + // creator
                           8;   // creation_time
}

impl BondingCurveParams {
    pub const SIZE: usize = 8 + // initial_price
                           8 + // slope
                           8 + // liquidity_target
                           8 + // current_supply
                           8 + // total_liquidity
                           1;  // bump

    pub fn calculate_buy_return(&self, sol_amount: u64) -> Result<u64> {
        // Calculate the price for the current supply
        let current_price = self.calculate_price(self.current_supply)?;

        // Calculate the number of tokens to be received
        let tokens_out = sol_amount
            .checked_mul(1_000_000) // Scale up for precision
            .ok_or(ErrorCode::Overflow)?
            .checked_div(current_price)
            .ok_or(ErrorCode::Overflow)?;

        // Ensure liquidity target is not exceeded
        require!(
            self.total_liquidity.checked_add(sol_amount).ok_or(ErrorCode::Overflow)? <= self.liquidity_target,
            ErrorCode::LiquidityTargetExceeded
        );

        Ok(tokens_out)
    }

    pub fn calculate_sell_return(&self, token_amount: u64) -> Result<u64> {
        // Calculate the price for the current supply
        let current_price = self.calculate_price(self.current_supply)?;

        // Calculate the amount of SOL to be received
        let sol_out = token_amount
            .checked_mul(current_price)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(1_000_000) // Scale down from precision
            .ok_or(ErrorCode::Overflow)?;

        // Ensure there is enough liquidity
        require!(
            sol_out <= self.total_liquidity,
            ErrorCode::InsufficientLiquidity
        );

        Ok(sol_out)
    }

    pub fn calculate_price(&self, amount: u64) -> Result<u64> {
        // Calculate the price based on the current supply and the amount to buy
        let new_supply = self.current_supply.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        let price = self.initial_price.checked_add(
            self.slope.checked_mul(new_supply).ok_or(ErrorCode::Overflow)?
        ).ok_or(ErrorCode::Overflow)?;

        Ok(price)
    }
} 