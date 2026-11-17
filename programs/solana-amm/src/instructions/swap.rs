
use anchor_lang::prelude::*;
use anchor_spl::token::{
    self, Mint, Token, TokenAccount, Transfer,
};

use crate::{constants::*, error::AmmError, state::Pool};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [
            POOL_SEED,
            pool.mint_a.as_ref(),
            pool.mint_b.as_ref()
        ],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,

    /// CHECK: PDA validated by seeds.
    #[account(
        seeds = [AUTHORITY_SEED, pool.key().as_ref()],
        bump = pool.authority_bump
    )]
    pub authority: UncheckedAccount<'info>,

    pub mint_a: Box<Account<'info, Mint>>,
    pub mint_b: Box<Account<'info, Mint>>,

    #[account(
        mut,
        address = pool.vault_a,
        token::mint = mint_a,
        token::authority = authority
    )]
    pub vault_a: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        address = pool.vault_b,
        token::mint = mint_b,
        token::authority = authority
    )]
    pub vault_b: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        address = pool.treasury_a,
        token::mint = mint_a
    )]
    pub treasury_a: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        address = pool.treasury_b,
        token::mint = mint_b
    )]
    pub treasury_b: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user_token_a: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user_token_b: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_swap(
    ctx: Context<Swap>,
    amount_in: u64,
    min_amount_out: u64,
    a_to_b: bool,
) -> Result<()> {
    require!(amount_in > 0, AmmError::InvalidAmount);

    let (reserve_in, reserve_out) = if a_to_b {
        (
            ctx.accounts.vault_a.amount as u128,
            ctx.accounts.vault_b.amount as u128,
        )
    } else {
        (
            ctx.accounts.vault_b.amount as u128,
            ctx.accounts.vault_a.amount as u128,
        )
    };

    require!(
        reserve_in > 0 && reserve_out > 0,
        AmmError::InsufficientLiquidity
    );

    // Calculate swap output after the 0.30% fee.
    let amount_with_fee = (amount_in as u128)
        .checked_mul(
            (FEE_DENOMINATOR - TOTAL_FEE_BPS) as u128
        )
        .ok_or(AmmError::MathOverflow)?;

    let numerator = amount_with_fee
        .checked_mul(reserve_out)
        .ok_or(AmmError::MathOverflow)?;

    let denominator = reserve_in
        .checked_mul(FEE_DENOMINATOR as u128)
        .ok_or(AmmError::MathOverflow)?
        .checked_add(amount_with_fee)
        .ok_or(AmmError::MathOverflow)?;

    let amount_out_u128 = numerator / denominator;

    let amount_out =
        u64::try_from(amount_out_u128)
            .map_err(|_| AmmError::MathOverflow)?;

    // Calculate protocol fee: 0.05%.
    let protocol_fee_u128 =
        (amount_in as u128)
            .checked_mul(PROTOCOL_FEE_BPS as u128)
            .ok_or(AmmError::MathOverflow)?
            / FEE_DENOMINATOR as u128;

    let protocol_fee =
        u64::try_from(protocol_fee_u128)
            .map_err(|_| AmmError::MathOverflow)?;

    require!(
        amount_out >= min_amount_out && amount_out > 0,
        AmmError::SlippageExceeded
    );

    require!(
        (amount_out as u128) < reserve_out,
        AmmError::InsufficientLiquidity
    );

    // Validate user token accounts.
    if a_to_b {
        require_keys_eq!(
            ctx.accounts.user_token_a.owner,
            ctx.accounts.user.key(),
            AmmError::InvalidTokenAccount
        );

        require_keys_eq!(
            ctx.accounts.user_token_a.mint,
            ctx.accounts.mint_a.key(),
            AmmError::InvalidMint
        );

        require_keys_eq!(
            ctx.accounts.user_token_b.owner,
            ctx.accounts.user.key(),
            AmmError::InvalidTokenAccount
        );

        require_keys_eq!(
            ctx.accounts.user_token_b.mint,
            ctx.accounts.mint_b.key(),
            AmmError::InvalidMint
        );
    } else {
        require_keys_eq!(
            ctx.accounts.user_token_b.owner,
            ctx.accounts.user.key(),
            AmmError::InvalidTokenAccount
        );

        require_keys_eq!(
            ctx.accounts.user_token_b.mint,
            ctx.accounts.mint_b.key(),
            AmmError::InvalidMint
        );

        require_keys_eq!(
            ctx.accounts.user_token_a.owner,
            ctx.accounts.user.key(),
            AmmError::InvalidTokenAccount
        );

        require_keys_eq!(
            ctx.accounts.user_token_a.mint,
            ctx.accounts.mint_a.key(),
            AmmError::InvalidMint
        );
    }

    // PDA signing seeds.
    let pool_key = ctx.accounts.pool.key();
    let bump = [ctx.accounts.pool.authority_bump];

    let authority_seeds: &[&[u8]] = &[
        AUTHORITY_SEED,
        pool_key.as_ref(),
        &bump,
    ];

    let signer_seeds = &[authority_seeds];

    // Transfer input tokens into the pool.
    let (source, destination) = if a_to_b {
        (
            ctx.accounts.user_token_a.to_account_info(),
            ctx.accounts.vault_a.to_account_info(),
        )
    } else {
        (
            ctx.accounts.user_token_b.to_account_info(),
            ctx.accounts.vault_b.to_account_info(),
        )
    };

    let input_transfer = Transfer {
        from: source,
        to: destination,
        authority: ctx.accounts.user.to_account_info(),
    };

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            input_transfer,
        ),
        amount_in,
    )?;

    // Transfer protocol fee to treasury.
    if protocol_fee > 0 {
        let (source, destination) = if a_to_b {
            (
                ctx.accounts.vault_a.to_account_info(),
                ctx.accounts.treasury_a.to_account_info(),
            )
        } else {
            (
                ctx.accounts.vault_b.to_account_info(),
                ctx.accounts.treasury_b.to_account_info(),
            )
        };

        let fee_transfer = Transfer {
            from: source,
            to: destination,
            authority: ctx.accounts.authority.to_account_info(),
        };

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                fee_transfer,
                signer_seeds,
            ),
            protocol_fee,
        )?;
    }

    // Transfer output tokens to user.
    let (source, destination) = if a_to_b {
        (
            ctx.accounts.vault_b.to_account_info(),
            ctx.accounts.user_token_b.to_account_info(),
        )
    } else {
        (
            ctx.accounts.vault_a.to_account_info(),
            ctx.accounts.user_token_a.to_account_info(),
        )
    };

    let output_transfer = Transfer {
        from: source,
        to: destination,
        authority: ctx.accounts.authority.to_account_info(),
    };

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            output_transfer,
            signer_seeds,
        ),
        amount_out,
    )?;

    msg!("Swap completed");
    msg!("Input: {}", amount_in);
    msg!("Output: {}", amount_out);
    msg!("Protocol fee: {}", protocol_fee);

    Ok(())
}
