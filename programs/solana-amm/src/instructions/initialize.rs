use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::{constants::*, state::Pool};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub treasury_authority: Signer<'info>,

    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = 8 + Pool::INIT_SPACE,
        seeds = [
            POOL_SEED,
            mint_a.key().as_ref(),
            mint_b.key().as_ref()
        ],
        bump
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: PDA used as token vault authority.
    #[account(
        seeds = [AUTHORITY_SEED, pool.key().as_ref()],
        bump
    )]
    pub authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = authority
    )]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = authority
    )]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        seeds = [LP_MINT_SEED, pool.key().as_ref()],
        bump,
        mint::decimals = LP_TOKEN_DECIMALS,
        mint::authority = authority
    )]
    pub lp_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = treasury_authority
    )]
    pub treasury_a: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = treasury_authority
    )]
    pub treasury_b: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(ctx: Context<Initialize>) -> Result<()> {
    require!(
        ctx.accounts.mint_a.key() != ctx.accounts.mint_b.key(),
        crate::error::AmmError::InvalidMint
    );

    let pool = &mut ctx.accounts.pool;

    pool.authority = ctx.accounts.payer.key();
    pool.treasury_authority = ctx.accounts.treasury_authority.key();

    pool.mint_a = ctx.accounts.mint_a.key();
    pool.mint_b = ctx.accounts.mint_b.key();

    pool.vault_a = ctx.accounts.vault_a.key();
    pool.vault_b = ctx.accounts.vault_b.key();

    pool.lp_mint = ctx.accounts.lp_mint.key();

    pool.treasury_a = ctx.accounts.treasury_a.key();
    pool.treasury_b = ctx.accounts.treasury_b.key();

    pool.bump = ctx.bumps.pool;
    pool.authority_bump = ctx.bumps.authority;
    pool.lp_mint_bump = ctx.bumps.lp_mint;

    pool.total_lp_supply = 0;

    msg!("AMM pool initialized");
    msg!("Mint A: {}", pool.mint_a);
    msg!("Mint B: {}", pool.mint_b);

    Ok(())
}
