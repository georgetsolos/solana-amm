use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub authority: Pubkey,
    pub treasury_authority: Pubkey,

    pub mint_a: Pubkey,
    pub mint_b: Pubkey,

    pub vault_a: Pubkey,
    pub vault_b: Pubkey,

    pub lp_mint: Pubkey,

    pub treasury_a: Pubkey,
    pub treasury_b: Pubkey,

    pub bump: u8,
    pub authority_bump: u8,
    pub lp_mint_bump: u8,

    pub total_lp_supply: u64,
}
