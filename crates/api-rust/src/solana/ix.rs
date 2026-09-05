//! Instruction builders for deposit and withdraw intent.

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::pda::{vault, vault_token_account, TOKEN_PROGRAM_ID};

/// Deposit (opcode 2): SPL transfer `amount` from `user_token_account` into the vault ATA.
///
/// Accounts: `[payer, user_token_account, vault, mint, vault_token_account, token_program]`
pub fn deposit(
    program_id: &Pubkey,
    payer: &Pubkey,
    user_token_account: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Instruction {
    let (vault_pda, _) = vault(program_id, mint);
    let (vault_ata, _) = vault_token_account(program_id, mint);
    let mut data = Vec::with_capacity(9);
    data.push(2);
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*user_token_account, false),
            AccountMeta::new_readonly(vault_pda, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data,
    }
}

/// Withdraw intent / `request_withdraw` (opcode 4): signal only — no token transfer.
///
/// Accounts: `[payer, user_token_account, vault, mint, vault_token_account]`
pub fn request_withdraw(
    program_id: &Pubkey,
    payer: &Pubkey,
    user_token_account: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Instruction {
    let (vault_pda, _) = vault(program_id, mint);
    let (vault_ata, _) = vault_token_account(program_id, mint);
    let mut data = Vec::with_capacity(9);
    data.push(4);
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*user_token_account, false),
            AccountMeta::new_readonly(vault_pda, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(vault_ata, false),
        ],
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn fixtures() -> (Pubkey, Pubkey, Pubkey, Pubkey) {
        let program_id =
            Pubkey::from_str("BULK2CNYn3mbgfYXEXiBBFxmmDChznpjQ4oRfce8w6R4").unwrap();
        let mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let payer = Pubkey::from_str("11111111111111111111111111111112").unwrap();
        let user_ta = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();
        (program_id, mint, payer, user_ta)
    }

    #[test]
    fn deposit_layout() {
        let (program_id, mint, payer, user_ta) = fixtures();
        let ix = deposit(&program_id, &payer, &user_ta, &mint, 1_000_000);

        assert_eq!(ix.program_id, program_id);
        assert_eq!(ix.data, {
            let mut d = vec![2u8];
            d.extend_from_slice(&1_000_000u64.to_le_bytes());
            d
        });
        assert_eq!(ix.accounts.len(), 6);
        assert!(ix.accounts[0].is_signer && ix.accounts[0].is_writable);
        assert!(ix.accounts[1].is_writable);
        assert!(!ix.accounts[2].is_writable);
        assert!(ix.accounts[4].is_writable);
        assert_eq!(ix.accounts[5].pubkey, TOKEN_PROGRAM_ID);
    }

    #[test]
    fn request_withdraw_layout() {
        let (program_id, mint, payer, user_ta) = fixtures();
        let ix = request_withdraw(&program_id, &payer, &user_ta, &mint, 500);

        assert_eq!(ix.data[0], 4);
        assert_eq!(&ix.data[1..], &500u64.to_le_bytes());
        assert_eq!(ix.accounts.len(), 5);
        assert!(!ix.accounts[1].is_writable);
    }
}
