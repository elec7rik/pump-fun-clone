mod state;
mod errors;
mod bonding_curve;

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint};
use state::{TokenMetadata, BondingCurveParams};
use errors::ErrorCode;
use bonding_curve::BondingCurve;

declare_id!("9e7FCcemFyvPUrXgUfxKCZvNVpLiiYMo34t77Kwa241u");

#[program]
pub mod pump_fun {
    use super::*;

    pub fn create_token(
        ctx: Context<CreateTokenContext>,
        name: String,
        symbol: String,
        description: String,
        image_url: String,
        _initial_supply: u64,
    ) -> Result<()> {
        // Validate inputs
        require!(name.len() <= 32, ErrorCode::NameTooLong);
        require!(symbol.len() <= 10, ErrorCode::SymbolTooLong);
        
        // Create metadata for the token
        let token_metadata = TokenMetadata {
            name,
            symbol,
            description,
            image_url,
            creator: ctx.accounts.authority.key(),
            creation_time: Clock::get()?.unix_timestamp,
        };

        // Store metadata on-chain
        ctx.accounts.token_metadata.set_inner(token_metadata);

        // Initialize bonding curve parameters
        let curve_params = BondingCurveParams {
            initial_price: 1_000_000,    // 0.001 SOL
            slope: 100,                  // Price increase rate
            liquidity_target: 17_000_000_000, // $17k in lamports
            current_supply: 0,           // Start with 0 tokens sold
            total_liquidity: 0,          // Start with 0 SOL in liquidity
            bump: ctx.bumps.bonding_curve,  // Store the bump
        };
        
        ctx.accounts.bonding_curve.set_inner(curve_params);
        
        Ok(())
    }

    pub fn create_trading_pool(
        ctx: Context<CreatePoolContext>, 
        _token_mint: Pubkey
    ) -> Result<()> {
        // Initialize the pool token account
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::InitializeAccount {
                account: ctx.accounts.pool_token_account.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        );
        token::initialize_account(cpi_ctx)?;

        Ok(())
    }

    pub fn trade_token(
        ctx: Context<TradeContext>, 
        amount_in: u64,
        min_amount_out: u64,
        is_buy: bool,
    ) -> Result<()> {
        const TRADING_FEE_BPS: u64 = 100; // 1% = 100 basis points
        
        // Get current market cap (total_liquidity)
        let current_market_cap = ctx.accounts.bonding_curve.total_liquidity;
        
        if is_buy {
            // Calculate fee
            let fee_amount = amount_in
                .checked_mul(TRADING_FEE_BPS)
                .unwrap()
                .checked_div(10000)
                .unwrap();
            let amount_after_fee = amount_in.checked_sub(fee_amount).unwrap();
            
            // Calculate tokens to receive using bonding curve
            let tokens_out = BondingCurve::calculate_tokens_out(
                amount_after_fee, 
                current_market_cap
            )?;
            require!(tokens_out >= min_amount_out, ErrorCode::SlippageExceeded);

            // Check if we should transition to Raydium
            if BondingCurve::should_transition_to_raydium(ctx.accounts.bonding_curve.current_supply) {
                return Err(ErrorCode::TransitionToRaydium.into());
            }

            // Transfer SOL fee to fee collector
            let fee_transfer_ctx = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.fee_collector.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(fee_transfer_ctx, fee_amount)?;

            // Transfer SOL from user to bonding curve
            let cpi_ctx = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.bonding_curve.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_ctx, amount_after_fee)?;

            // Mint tokens to user
            let mint_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.bonding_curve.to_account_info(),
                },
            );
            token::mint_to(mint_ctx, tokens_out)?;

            // Update bonding curve state
            ctx.accounts.bonding_curve.current_supply = ctx.accounts.bonding_curve.current_supply
                .checked_add(tokens_out)
                .unwrap();
            ctx.accounts.bonding_curve.total_liquidity = ctx.accounts.bonding_curve.total_liquidity
                .checked_add(amount_after_fee)
                .unwrap();
            
        } else {
            // Calculate SOL to receive using bonding curve
            let sol_out = BondingCurve::calculate_sol_out(
                amount_in, 
                current_market_cap
            )?;
            require!(sol_out >= min_amount_out, ErrorCode::SlippageExceeded);

            // Calculate fee
            let fee_amount = sol_out
                .checked_mul(TRADING_FEE_BPS)
                .unwrap()
                .checked_div(10000)
                .unwrap();
            let amount_after_fee = sol_out.checked_sub(fee_amount).unwrap();

            // Get PDA signer seeds
            let token_mint_key = ctx.accounts.token_mint.key();
            let bump = ctx.accounts.bonding_curve.bump;
            let seeds = &[
                b"curve".as_ref(),
                token_mint_key.as_ref(),
                &[bump],
            ];
            let signer_seeds = &[&seeds[..]];

            // Transfer SOL fee to fee collector
            let fee_transfer_ctx = CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.bonding_curve.to_account_info(),
                    to: ctx.accounts.fee_collector.to_account_info(),
                },
                signer_seeds,
            );
            anchor_lang::system_program::transfer(fee_transfer_ctx, fee_amount)?;

            // Burn tokens from user
            let burn_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            );
            token::burn(burn_ctx, amount_in)?;

            // Transfer SOL to user
            let transfer_ctx = CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.bonding_curve.to_account_info(),
                    to: ctx.accounts.user.to_account_info(),
                },
                signer_seeds,
            );
            anchor_lang::system_program::transfer(transfer_ctx, amount_after_fee)?;

            // Update bonding curve state
            ctx.accounts.bonding_curve.current_supply = ctx.accounts.bonding_curve.current_supply
                .checked_sub(amount_in)
                .unwrap();
            ctx.accounts.bonding_curve.total_liquidity = ctx.accounts.bonding_curve.total_liquidity
                .checked_sub(sol_out)
                .unwrap();
        }
        
        Ok(())
    }

    pub fn initialize_treasury(
        ctx: Context<InitializeTreasury>,
        token_mint: Pubkey,
    ) -> Result<()> {
        // Initialize treasury PDA for this specific token
        let treasury_bump = ctx.bumps.treasury_authority;
        let treasury_seeds = &[
            b"treasury",
            token_mint.as_ref(),
            &[treasury_bump]
        ];
        
        // Create the treasury token account owned by the PDA
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::InitializeAccount {
                account: ctx.accounts.treasury_token_account.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                authority: ctx.accounts.treasury_authority.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        );
        token::initialize_account(cpi_ctx)?;

        Ok(())
    }

    pub fn withdraw_fees(
        ctx: Context<WithdrawFees>,
        amount: u64,
    ) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.program_config.admin,
            ErrorCode::Unauthorized
        );

        let transfer_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.fee_collector.to_account_info(),
                to: ctx.accounts.authority.to_account_info(),
            },
        );
        
        anchor_lang::system_program::transfer(transfer_ctx, amount)?;
        
        Ok(())
    }

    pub fn initialize_program_config(
        ctx: Context<InitializeProgramConfig>,
        fee_collector: Pubkey,
        admin: Pubkey,
    ) -> Result<()> {
        ctx.accounts.program_config.fee_collector = fee_collector;
        ctx.accounts.program_config.admin = admin;
        Ok(())
    }

    pub fn update_program_config(
        ctx: Context<UpdateProgramConfig>,
        new_fee_collector: Option<Pubkey>,
        new_admin: Option<Pubkey>,
    ) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.program_config.admin,
            ErrorCode::Unauthorized
        );

        if let Some(fee_collector) = new_fee_collector {
            ctx.accounts.program_config.fee_collector = fee_collector;
        }
        if let Some(admin) = new_admin {
            ctx.accounts.program_config.admin = admin;
        }
        Ok(())
    }

    pub fn pause_trading(ctx: Context<PauseTrading>) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.program_config.admin,
            ErrorCode::Unauthorized
        );
        
        ctx.accounts.program_config.trading_paused = true;
        Ok(())
    }

    pub fn resume_trading(ctx: Context<ResumeTrading>) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.program_config.admin,
            ErrorCode::Unauthorized
        );
        
        ctx.accounts.program_config.trading_paused = false;
        Ok(())
    }

    pub fn update_trading_fee(
        ctx: Context<UpdateTradingFee>,
        new_fee_bps: u16,
    ) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.program_config.admin,
            ErrorCode::Unauthorized
        );
        require!(new_fee_bps <= 1000, ErrorCode::InvalidFeePercentage); // Max 10%
        
        ctx.accounts.program_config.trading_fee_bps = new_fee_bps;
        
        emit!(ConfigUpdateEvent {
            admin: ctx.accounts.program_config.admin,
            fee_collector: ctx.accounts.program_config.fee_collector,
            trading_fee_bps: new_fee_bps,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String, symbol: String)]
pub struct CreateTokenContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub program_config: Account<'info, ProgramConfig>,
    
    #[account(
        init,
        payer = authority,
        mint::decimals = 9,
        mint::authority = authority,
    )]
    pub token_mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = authority,
        space = 8 + TokenMetadata::SIZE,  // discriminator + metadata size
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub token_metadata: Account<'info, TokenMetadata>,
    
    #[account(
        init,
        payer = authority,
        space = 8 + BondingCurveParams::SIZE,  // discriminator + params size
        seeds = [b"curve", token_mint.key().as_ref()],
        bump
    )]
    pub bonding_curve: Account<'info, BondingCurveParams>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CreatePoolContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        token::mint = token_mint,
        token::authority = pool_authority,
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,

    /// CHECK: This is the PDA that will own the pool
    #[account(
        seeds = [b"pool", token_mint.key().as_ref()],
        bump
    )]
    pub pool_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct TradeContext<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        token::mint = token_mint,
        token::authority = treasury_authority
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,
    
    pub token_mint: Account<'info, Mint>,
    
    /// CHECK: PDA that owns the treasury
    #[account(
        seeds = [b"treasury", token_mint.key().as_ref()],
        bump
    )]
    pub treasury_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        seeds = [b"curve", token_mint.key().as_ref()],
        bump,
    )]
    pub bonding_curve: Account<'info, BondingCurveParams>,

    /// CHECK: Account that receives trading fees
    #[account(
        mut,
        constraint = fee_collector.key() == program_config.fee_collector
    )]
    pub fee_collector: AccountInfo<'info>,
    
    pub program_config: Account<'info, ProgramConfig>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(token_mint: Pubkey)]
pub struct InitializeTreasury<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        init,
        payer = authority,
        token::mint = token_mint,
        token::authority = treasury_authority,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,
    
    pub token_mint: Account<'info, Mint>,
    
    /// CHECK: This is the PDA that will own the treasury
    #[account(
        seeds = [b"treasury", token_mint.key().as_ref()],
        bump
    )]
    pub treasury_authority: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub program_config: Account<'info, ProgramConfig>,
    
    /// CHECK: Account that holds collected fees
    #[account(mut)]
    pub fee_collector: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeProgramConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32, // discriminator + fee_collector + admin
    )]
    pub program_config: Account<'info, ProgramConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateProgramConfig<'info> {
    pub authority: Signer<'info>,

    #[account(mut)]
    pub program_config: Account<'info, ProgramConfig>,
}

#[derive(Accounts)]
pub struct PauseTrading<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub program_config: Account<'info, ProgramConfig>,
}

#[derive(Accounts)]
pub struct ResumeTrading<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub program_config: Account<'info, ProgramConfig>,
}

#[derive(Accounts)]
pub struct UpdateTradingFee<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub program_config: Account<'info, ProgramConfig>,
}

#[account]
pub struct ProgramConfig {
    pub fee_collector: Pubkey,
    pub admin: Pubkey,
    pub trading_paused: bool,
    pub trading_fee_bps: u16,
}

#[event]
pub struct TradeEvent {
    pub user: Pubkey,
    pub token_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub is_buy: bool,
    pub timestamp: i64,
}

#[event]
pub struct ConfigUpdateEvent {
    pub admin: Pubkey,
    pub fee_collector: Pubkey,
    pub trading_fee_bps: u16,
    pub timestamp: i64,
}
