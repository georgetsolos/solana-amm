use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Mint, MintTo, Token, TokenAccount, Transfer,
};

use crate::{constants::*, error::AmmError, state::Pool};

#[derive(Accounts)]
pub struct Deposit<'info> {
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

pub fn handle_deposit(
    ctx: Context<Deposit>,
    amount_a: u64,
    amount_b: u64,
) -> Result<()> {
    require!(amount_a > 0 && amount_b > 0, AmmError::InvalidAmount);

    let reserve_a = ctx.accounts.vault_a.amount as u128;
    let reserve_b = ctx.accounts.vault_b.amount as u128;

    let supply = ctx.accounts.lp_mint.supply as u128;

    let liquidity = if supply == 0 {
        integer_sqrt(
            (amount_a as u128)
                .checked_mul(amount_b as u128)
                .ok_or(AmmError::MathOverflow)?
        )
    } else {
        require!(
            reserve_a > 0 && reserve_b > 0,
            AmmError::InvalidReserves
        );

        let lp_a = (amount_a as u128)
            .checked_mul(supply)
            .ok_or(AmmError::MathOverflow)?
            / reserve_a;

        let lp_b = (amount_b as u128)
            .checked_mul(supply)
            .ok_or(AmmError::MathOverflow)?
            / reserve_b;

        lp_a.min(lp_b)
    };

    let liquidity =
        u64::try_from(liquidity).map_err(|_| AmmError::MathOverflow)?;

    require!(liquidity > 0, AmmError::InvalidAmount);

    let cpi_a = Transfer {
        from: ctx.accounts.user_token_a.to_account_info(),
        to: ctx.accounts.vault_a.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    token::transfer(
        CpiContext::new(ctx.accounts.token_program.key(), cpi_a),
        amount_a,
    )?;

    let cpi_b = Transfer {
        from: ctx.accounts.user_token_b.to_account_info(),
        to: ctx.accounts.vault_b.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    token::transfer(
        CpiContext::new(ctx.accounts.token_program.key(), cpi_b),
        amount_b,
    )?;

    let pool_key = ctx.accounts.pool.key();
    let bump = [ctx.accounts.pool.authority_bump];

    let authority_seeds: &[&[u8]] = &[
        AUTHORITY_SEED,
        pool_key.as_ref(),
        &bump,
    ];

    let signer_seeds = &[authority_seeds];

    let mint_accounts = MintTo {
        mint: ctx.accounts.lp_mint.to_account_info(),
        to: ctx.accounts.user_lp_token.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            mint_accounts,
            signer_seeds,
        ),
        liquidity,
    )?;

    ctx.accounts.pool.total_lp_supply = ctx.accounts.lp_mint.supply
        .checked_add(liquidity)
        .ok_or(AmmError::MathOverflow)?;

    msg!("Liquidity deposited");
    msg!("LP tokens minted: {}", liquidity);

    Ok(())
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }

    let mut x = value;
    let mut y = (x + 1) / 2;

    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }

    x
}
