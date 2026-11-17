use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Invalid token mint")]
    InvalidMint,

    #[msg("Invalid pool authority")]
    InvalidAuthority,

    #[msg("Amount must be greater than zero")]
    InvalidAmount,

    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Arithmetic overflow")]
    MathOverflow,

    #[msg("Invalid fee configuration")]
    InvalidFee,

    #[msg("Insufficient LP tokens")]
    InsufficientLpTokens,

    #[msg("Pool is already initialized")]
    AlreadyInitialized,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("Insufficient output amount")]
    InsufficientOutput,

    #[msg("Invalid pool reserves")]
    InvalidReserves,
}
