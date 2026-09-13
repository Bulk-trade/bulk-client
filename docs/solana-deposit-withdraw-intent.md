# Solana deposit and withdrawal intent

These are Solana transactions signed by the token-account owner. They are separate
from Bulk HTTP/WebSocket trading transactions and do not use a Bulk signature domain.

| Operation | Instruction | Result |
| --- | --- | --- |
| `deposit` | Opcode 2 + amount as 8-byte little-endian `u64` | Transfers SPL tokens from the user's token account into the vault token account. Bulk credit follows bridge processing. |
| `request_withdraw` / CLI `withdraw-intent` | Opcode 4 + the same amount encoding | Signals a withdrawal request; transfers no tokens. Bulk processes the rooted event to lock margin. Settlement is a later bridge operation. |

A confirmed withdrawal-intent signature is not proof of settlement. Check Bulk's
withdrawal state and the eventual Solana token transfer separately.

Amounts are integer mint base units: `1000000` is 1 USDC for the six-decimal mainnet
USDC mint. Both operations require an **existing** classic SPL Token account owned
by the signer and matching the mint. The CLI derives the owner's associated token
account (ATA) by default; it does not create it. A supplied non-ATA token account
can also be used if it meets those ownership and mint checks. The signer needs SOL
for transaction fees, and deposits need sufficient tokens. The vault must already
be initialized for that mint. These builders use the classic Tokenkeg program,
not Token-2022.

## CLI

Load `BULK_PRIVATE_KEY` from your existing secret-management workflow, or supply
`--private-key`. It must encode a Solana keypair as base58 (64 decoded bytes), not a
keypair-file path or a Bulk agent authorization. Both commands require it even for
`--dry-run`. The current CLI rejects `--ledger` for these on-chain operations.

```bash
# Assemble and print account metas, derived addresses and instruction bytes.
bulk deposit --mint EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v --amount 1000000 --dry-run
bulk withdraw-intent --mint EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v --amount 1000000 --dry-run
```

Remove `--dry-run` to sign and submit after the default confirmation prompt.
`--yes` skips that prompt; `--preview false` also disables it. Dry-run does no RPC
simulation, account validation, signing, or submission. Submission fetches a
recent blockhash, sends with preflight enabled, and waits for `confirmed` or
`finalized`; this does not wait for Bulk credit or withdrawal settlement.

| Option | Default / meaning |
| --- | --- |
| `--mint` | Required mint public key. |
| `--amount` | Required integer base units. |
| `--user-token-account` | Optional source for deposit / destination for intent; otherwise the signer's ATA. |
| `--program-id` / `BULK_PROGRAM_ID` | `BULK2CNYn3mbgfYXEXiBBFxmmDChznpjQ4oRfce8w6R4` |
| `--rpc-url` / `BULK_RPC_URL` | `https://api.mainnet-beta.solana.com` |

Those are mainnet **defaults**, not enforced pins. The client still supports
program, mint, token-account and RPC overrides. `--api-url` and
`--signature-domain` do not select the Solana cluster for these commands. These
client builders do not inherit restrictions from a separate keychain interface.

## Rust: build and sign

The Rust crate exports `bulk_client::solana::{deposit, request_withdraw,
associated_token_address, vault, vault_token_account}`. Builders return a standard
`solana_instruction::Instruction`; they do not fetch accounts, sign or submit.

For a standalone example located at the repository root, use these dependencies
(adjust the local path when placing your application elsewhere):

```toml
[dependencies]
bulk-client = { path = "crates/api-rust" }
solana-pubkey = "3.0"
solana-keypair = "=3.1.2"
solana-signer = "3.0"
solana-hash = "3.0"
solana-transaction = { version = "3.0", features = ["bincode", "serde"] }
bincode = "1.3"
```

```rust
use bulk_client::solana::{associated_token_address, deposit};
use solana_hash::Hash;
use solana_keypair::read_keypair_file;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{env, str::FromStr};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let payer = read_keypair_file(env::var("SOLANA_KEYPAIR_FILE")?)?;
    let program = Pubkey::from_str("BULK2CNYn3mbgfYXEXiBBFxmmDChznpjQ4oRfce8w6R4")?;
    let mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    let ix = deposit(
        &program, &payer.pubkey(),
        &associated_token_address(&payer.pubkey(), &mint), &mint, 1_000_000,
    );
    // Obtain a fresh getLatestBlockhash result from the intended Solana RPC.
    let blockhash = Hash::from_str(&env::var("SOLANA_RECENT_BLOCKHASH")?)?;
    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer.pubkey()));
    tx.try_sign(&[&payer], blockhash)?;
    let wire = bincode::serialize(&tx)?;
    println!("Signed {} bytes; not submitted", wire.len());
    Ok(())
}
```

For an intent, replace `deposit` with `request_withdraw` in both the import and
call; its arguments are identical. To use an external Solana wallet, construct the
transaction with that wallet's public key as payer and token-account owner, then
pass it through the wallet's Solana transaction-signing API instead of `try_sign`.
The client has no browser-wallet adapter. Serialize the signed transaction and
submit through your Solana RPC integration with preflight and confirmation checks.
Do not sign these instruction bytes as an off-chain Bulk trading message.

## Python: convert the builder and sign with solders

Install the local package with `python -m pip install ./crates/api-python`.
Its dependencies include `solders>=0.26.0`. The module is
`bulk_client.solana`; its builders return the client's own `Instruction` and
`AccountMeta` dataclasses, so convert them to solders types before assembly.

```python
import json
import os
from pathlib import Path

from bulk_client.solana import deposit, TOKEN_PROGRAM_ID
from solders.hash import Hash
from solders.instruction import AccountMeta, Instruction
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.transaction import Transaction

payer = Keypair.from_bytes(bytes(json.loads(
    Path(os.environ["SOLANA_KEYPAIR_FILE"]).read_text()
)))
program = Pubkey.from_string("BULK2CNYn3mbgfYXEXiBBFxmmDChznpjQ4oRfce8w6R4")
mint = Pubkey.from_string("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
# The Python module has no associated_token_address helper.
user_ata = Pubkey.find_program_address(
    [bytes(payer.pubkey()), bytes(TOKEN_PROGRAM_ID), bytes(mint)],
    Pubkey.from_string("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
)[0]
built = deposit(program, payer.pubkey(), user_ata, mint, 1_000_000)
ix = Instruction(built.program_id, built.data, [
    AccountMeta(meta.pubkey, meta.is_signer, meta.is_writable)
    for meta in built.accounts
])
# Supply a fresh blockhash obtained from the intended RPC's getLatestBlockhash.
blockhash = Hash.from_string(os.environ["SOLANA_RECENT_BLOCKHASH"])
tx = Transaction.new_signed_with_payer([ix], payer.pubkey(), [payer], blockhash)
wire = bytes(tx)
print(f"Signed {len(wire)} bytes; not submitted")
```

Use `request_withdraw` instead of `deposit` for an intent. Pass integer base units;
do not pass human-readable decimal token amounts. For an external wallet, hand
the assembled Solana transaction to its transaction-signing integration rather
than loading a local keypair. Neither Python builder sends a transaction.

## Wire reference

The vault uses seeds `[b"vault\0\0\0", mint]`; its token account uses
`[b"vault_ata", mint]`, both under the selected Bulk program. The latter is a Bulk
PDA, not the user's conventional ATA derivation.

Deposit accounts in order: signer/writable payer, writable user token account,
read-only vault, read-only mint, writable vault token account, read-only Tokenkeg.
Intent accounts use the same first five in order, with only the payer writable;
there is no token-program account or transfer instruction.

Implementation references: [Rust builders](../crates/api-rust/src/solana/ix.rs),
[Python builders](../crates/api-python/bulk_client/solana.py),
[CLI flags](../crates/cli/src/commands/solana.rs), and
[CLI submission](../crates/cli/src/handlers/solana.rs).
