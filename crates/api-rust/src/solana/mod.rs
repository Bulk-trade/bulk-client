//! Solana program instructions for vault deposit and withdraw intent.
//!
//! Builders match the on-chain bulk program layouts (opcodes 2 and 4).

pub mod ix;
pub mod pda;

pub use ix::{deposit, request_withdraw};
pub use pda::{
    associated_token_address, vault, vault_token_account, ATA_PROGRAM_ID, TOKEN_PROGRAM_ID,
    VAULT_ATA_SEED, VAULT_SEED,
};
