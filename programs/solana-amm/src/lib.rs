pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("uRMgTCNZMr6KURp4KVsAWXb1SU77TFdrDwuMqTr8LcA");

#[program]
pub mod solana_amm {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::handle_initialize(ctx)
    }

    pub fn deposit(
        ctx: Context<Deposit>,
        amount_a: u64,
        amount_b: u64,
    ) -> Result<()> {
        instructions::deposit::handle_deposit(ctx, amount_a, amount_b)
    }

    pub fn withdraw(
        ctx: Context<Withdraw>,
        lp_amount: u64,
        min_a: u64,
        min_b: u64,
    ) -> Result<()> {
        instructions::withdraw::handle_withdraw(
            ctx,
            lp_amount,
            min_a,
            min_b,
        )
    }

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
        a_to_b: bool,
    ) -> Result<()> {
        instructions::swap::handle_swap(
            ctx,
            amount_in,
            min_amount_out,
            a_to_b,
        )
    }
}
