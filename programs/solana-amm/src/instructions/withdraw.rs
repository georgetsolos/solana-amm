use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Burn, Mint, Token, TokenAccount, Transfer,
};

use crate::{constants::*, error::AmmError, state::Pool};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [
            POOL_SEED,
            pool.mint_a.as_ref(),
            pool.mint_b.as_ref()
        ],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: PDA validated by seeds.
    #[account(
        seeds = [AUTHORITY_SEED, pool.key().as_ref()],
        bump = pool.authority_bump
    )]
    pub authority: UncheckedAccount<'info>,

    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,

    #[account(
        mut,
        address = pool.vault_a,
        token::mint = mint_a,
        token::authority = authority
    )]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = pool.vault_b,
        token::mint = mint_b,
        token::authority = authority
    )]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = mint_a,
        token::authority = user
    )]
    pub user_token_a: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = mint_b,
        token::authority = user
    )]
    pub user_token_b: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = pool.lp_mint,
        mint::authority = authority
    )]
    pub lp_mint: Account<'info, Mint>,

    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = user
    )]
    pub user_lp_token: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_withdraw(
    ctx: Context<Withdraw>,
    lp_amount: u64,
    min_a: u64,
    min_b: u64,
) -> Result<()> {
    require!(lp_amount > 0, AmmError::InvalidAmount);

    let supply = ctx.accounts.lp_mint.supply;

    require!(
        supply > 0 && lp_amount <= supply,
        AmmError::InsufficientLpTokens
    );

    let amount_a = (lp_amount as u128)
        .checked_mul(ctx.accounts.vault_a.amount as u128)
        .ok_or(AmmError::MathOverflow)?
        / supply as u128;

    let amount_b = (lp_amount as u128)
        .checked_mul(ctx.accounts.vault_b.amount as u128)
        .ok_or(AmmError::MathOverflow)?
        / supply as u128;

    let amount_a =
        u64::try_from(amount_a).map_err(|_| AmmError::MathOverflow)?;

    let amount_b =
        u64::try_from(amount_b).map_err(|_| AmmError::MathOverflow)?;

    require!(amount_a >= min_a, AmmError::SlippageExceeded);
    require!(amount_b >= min_b, AmmError::SlippageExceeded);

    require!(
        amount_a > 0 && amount_b > 0,
        AmmError::InsufficientLiquidity
    );

    let burn_accounts = Burn {
        mint: ctx.accounts.lp_mint.to_account_info(),
        from: ctx.accounts.user_lp_token.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            burn_accounts,
        ),
        lp_amount,
    )?;

    let pool_key = ctx.accounts.pool.key();
    let bump = [ctx.accounts.pool.authority_bump];

    let authority_seeds: &[&[u8]] = &[
        AUTHORITY_SEED,
        pool_key.as_ref(),
        &bump,
    ];

    let signer_seeds = &[authority_seeds];

    let transfer_a = Transfer {
        from: ctx.accounts.vault_a.to_account_info(),
        to: ctx.accounts.user_token_a.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            transfer_a,
            signer_seeds,
        ),
        amount_a,
    )?;

    let transfer_b = Transfer {
        from: ctx.accounts.vault_b.to_account_info(),
        to: ctx.accounts.user_token_b.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            transfer_b,
            signer_seeds,
        ),
        amount_b,
    )?;

    ctx.accounts.pool.total_lp_supply = supply
        .checked_sub(lp_amount)
        .ok_or(AmmError::MathOverflow)?;

    msg!("Liquidity withdrawn");
    msg!("LP burned: {}", lp_amount);
    msg!("Token A returned: {}", amount_a);
    msg!("Token B returned: {}", amount_b);

    Ok(())
}
