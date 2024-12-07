use anchor_lang::prelude::error_code;

#[error_code]
pub enum ErrorCode {
    #[msg("Name too long")]
    NameTooLong,
    #[msg("Symbol too long")]
    SymbolTooLong,
    #[msg("Unauthorized action")]
    Unauthorized,
    #[msg("Numerical overflow occurred")]
    Overflow,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Liquidity target exceeded")]
    LiquidityTargetExceeded,
    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,
    #[msg("Error in price calculation")]
    CalculationError,
    #[msg("Trading should now transition to Raydium")]
    TransitionToRaydium,
    #[msg("Trading is currently paused")]
    TradingPaused,
    #[msg("Invalid fee percentage")]
    InvalidFeePercentage,
    #[msg("Invalid admin address")]
    InvalidAdminAddress,
} 
