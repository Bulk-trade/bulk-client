"""Solana deposit and withdraw-intent instruction builders.

Matches bulk-program opcodes 2 (deposit) and 4 (request_withdraw).
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import List, Tuple, Union

from solders.pubkey import Pubkey

PubkeyLike = Union[str, bytes, Pubkey]

TOKEN_PROGRAM_ID = Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
VAULT_SEED = b"vault\0\0\0"
VAULT_ATA_SEED = b"vault_ata"


def _pubkey(key: PubkeyLike) -> Pubkey:
    if isinstance(key, Pubkey):
        return key
    if isinstance(key, bytes):
        return Pubkey.from_bytes(key)
    return Pubkey.from_string(key)


def vault(program_id: PubkeyLike, mint: PubkeyLike) -> Tuple[Pubkey, int]:
    return Pubkey.find_program_address(
        [VAULT_SEED, bytes(_pubkey(mint))], _pubkey(program_id)
    )


def vault_token_account(program_id: PubkeyLike, mint: PubkeyLike) -> Tuple[Pubkey, int]:
    return Pubkey.find_program_address(
        [VAULT_ATA_SEED, bytes(_pubkey(mint))], _pubkey(program_id)
    )


@dataclass(frozen=True)
class AccountMeta:
    pubkey: Pubkey
    is_signer: bool
    is_writable: bool


@dataclass(frozen=True)
class Instruction:
    program_id: Pubkey
    accounts: List[AccountMeta]
    data: bytes


def deposit(
    program_id: PubkeyLike,
    payer: PubkeyLike,
    user_token_account: PubkeyLike,
    mint: PubkeyLike,
    amount: int,
) -> Instruction:
    """Opcode 2: SPL transfer `amount` into the vault ATA."""
    program = _pubkey(program_id)
    mint_pk = _pubkey(mint)
    vault_pda, _ = vault(program, mint_pk)
    vault_ata, _ = vault_token_account(program, mint_pk)
    data = bytes([2]) + int(amount).to_bytes(8, "little", signed=False)
    return Instruction(
        program_id=program,
        accounts=[
            AccountMeta(_pubkey(payer), True, True),
            AccountMeta(_pubkey(user_token_account), False, True),
            AccountMeta(vault_pda, False, False),
            AccountMeta(mint_pk, False, False),
            AccountMeta(vault_ata, False, True),
            AccountMeta(TOKEN_PROGRAM_ID, False, False),
        ],
        data=data,
    )


def request_withdraw(
    program_id: PubkeyLike,
    payer: PubkeyLike,
    user_token_account: PubkeyLike,
    mint: PubkeyLike,
    amount: int,
) -> Instruction:
    """Opcode 4: withdraw intent signal (no token transfer)."""
    program = _pubkey(program_id)
    mint_pk = _pubkey(mint)
    vault_pda, _ = vault(program, mint_pk)
    vault_ata, _ = vault_token_account(program, mint_pk)
    data = bytes([4]) + int(amount).to_bytes(8, "little", signed=False)
    return Instruction(
        program_id=program,
        accounts=[
            AccountMeta(_pubkey(payer), True, True),
            AccountMeta(_pubkey(user_token_account), False, False),
            AccountMeta(vault_pda, False, False),
            AccountMeta(mint_pk, False, False),
            AccountMeta(vault_ata, False, False),
        ],
        data=data,
    )
