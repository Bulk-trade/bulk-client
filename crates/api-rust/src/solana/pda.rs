//! PDA helpers for the bulk Solana program.

use solana_pubkey::Pubkey;

/// Seed for the vault PDA: ASCII `vault` + three NUL bytes (`b"vault\0\0\0"`).
pub const VAULT_SEED: [u8; 8] = *b"vault\0\0\0";

/// Seed for the vault token-account PDA.
pub const VAULT_ATA_SEED: &[u8] = b"vault_ata";

/// Associated Token Account program.
pub const ATA_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// SPL Token program (classic).
pub const TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Vault PDA for `mint`: seeds `[VAULT_SEED, mint]`.
pub fn vault(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_SEED.as_ref(), mint.as_ref()], program_id)
}

/// Vault token-account PDA for `mint`: seeds `[b"vault_ata", mint]`.
pub fn vault_token_account(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_ATA_SEED, mint.as_ref()], program_id)
}

/// Owner's associated token account for `mint` (Tokenkeg + ATA program).
pub fn associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ATA_PROGRAM_ID,
    )
    .0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Mainnet program / USDC / known vault PDAs.
    #[test]
    fn mainnet_usdc_vault_pdas() {
        let program_id =
            Pubkey::from_str("BULK2CNYn3mbgfYXEXiBBFxmmDChznpjQ4oRfce8w6R4").unwrap();
        let mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();

        let (vault_pda, _) = vault(&program_id, &mint);
        let (ata, _) = vault_token_account(&program_id, &mint);

        assert_eq!(
            vault_pda.to_string(),
            "7Wpp33Dn5KKUFjaij4zKYy1XZ9kdBtHjUatAT6NcjjGt"
        );
        assert_eq!(
            ata.to_string(),
            "HwdwwKH1tMXo7ggTKcA5cdQrpcgqSoVib2eQh3BiyEQL"
        );
    }
}
