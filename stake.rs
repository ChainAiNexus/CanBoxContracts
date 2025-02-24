use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod staking {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool>, reward_rate: u64, lock_duration: i64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.reward_rate = reward_rate;
        pool.lock_duration = lock_duration;
        pool.pool_token_mint = ctx.accounts.mint.key();
        pool.admin = *ctx.accounts.admin.key;
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.pool_token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        
        token::transfer(transfer_ctx, amount)?;

        let stake_account = &mut ctx.accounts.stake_account;
        stake_account.amount += amount;
        stake_account.stake_time = Clock::get()?.unix_timestamp;
        stake_account.user = *ctx.accounts.user.key;
        
        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        let stake_account = &ctx.accounts.stake_account;
        let pool = &ctx.accounts.pool;
        
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            current_time >= stake_account.stake_time + pool.lock_duration,
            StakingError::LockPeriodNotEnded
        );

        let duration = (current_time - stake_account.stake_time) as u64;
        let reward = stake_account.amount
            .checked_mul(duration)
            .and_then(|v| v.checked_mul(pool.reward_rate))
            .ok_or(StakingError::CalculationOverflow)?;

        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.pool_token_account.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
        );
        token::transfer(transfer_ctx, stake_account.amount)?;

        Ok(())
    }
}

#[account]
pub struct Pool {
    pub reward_rate: u64,
    pub lock_duration: i64,
    pub pool_token_mint: Pubkey,
    pub admin: Pubkey,
}

#[account]
pub struct StakeAccount {
    pub amount: u64,
    pub stake_time: i64,
    pub user: Pubkey,
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(init, payer = admin, space = 8 + 8 + 8 + 32 + 32)]
    pub pool: Account<'info, Pool>,
    pub mint: Account<'info, token::Mint>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 8 + 8 + 32,
        seeds = [b"stake", pool.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, token::TokenAccount>,
    #[account(mut)]
    pub pool_token_account: Account<'info, token::TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut, has_one = user)]
    pub stake_account: Account<'info, StakeAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, token::TokenAccount>,
    #[account(mut)]
    pub pool_token_account: Account<'info, token::TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[error_code]
pub enum StakingError {
    #[msg("Lock period not ended")]
    LockPeriodNotEnded,
    #[msg("Calculation overflow")]
    CalculationOverflow,
}