use crate::commands::{DepositArgs, WithdrawIntentArgs};
use crate::common::SubmitOptions;
use base64::{engine::general_purpose::STANDARD, Engine};
use bulk_client::solana::{
    associated_token_address, deposit, request_withdraw, vault, vault_token_account,
};
use serde::Deserialize;
use serde_json::json;
use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::io::{self, Write};
use std::str::FromStr;
use std::time::Duration;

pub async fn handle_deposit(
    payer: &Keypair,
    args: DepositArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let payer_pk = payer.pubkey();
    let user_ta = args
        .user_token_account
        .unwrap_or_else(|| associated_token_address(&payer_pk, &args.mint));
    let ix = deposit(
        &args.program_id,
        &payer_pk,
        &user_ta,
        &args.mint,
        args.amount,
    );
    run_solana_ix(
        payer,
        "deposit",
        &args.rpc_url,
        args.dry_run,
        submit,
        &args.program_id,
        &args.mint,
        ix,
    )
    .await
}

pub async fn handle_withdraw_intent(
    payer: &Keypair,
    args: WithdrawIntentArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let payer_pk = payer.pubkey();
    let user_ta = args
        .user_token_account
        .unwrap_or_else(|| associated_token_address(&payer_pk, &args.mint));
    let ix = request_withdraw(
        &args.program_id,
        &payer_pk,
        &user_ta,
        &args.mint,
        args.amount,
    );
    run_solana_ix(
        payer,
        "withdraw-intent",
        &args.rpc_url,
        args.dry_run,
        submit,
        &args.program_id,
        &args.mint,
        ix,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_solana_ix(
    payer: &Keypair,
    name: &str,
    rpc_url: &str,
    dry_run: bool,
    submit: &SubmitOptions,
    program_id: &Pubkey,
    mint: &Pubkey,
    ix: Instruction,
) -> eyre::Result<()> {
    let payer_pk = payer.pubkey();
    let (vault_pda, _) = vault(program_id, mint);
    let (vault_ata, _) = vault_token_account(program_id, mint);

    eprintln!("--- {name} ---");
    eprintln!("program:   {program_id}");
    eprintln!("payer:     {payer_pk}");
    eprintln!("mint:      {mint}");
    eprintln!("vault:     {vault_pda}");
    eprintln!("vault_ata: {vault_ata}");
    eprintln!("data:      {}", bytes_to_hex(&ix.data));
    for (i, meta) in ix.accounts.iter().enumerate() {
        eprintln!(
            "  [{i}] {}  signer={} writable={}",
            meta.pubkey, meta.is_signer, meta.is_writable
        );
    }

    if dry_run {
        eprintln!("dry-run: not sending");
        return Ok(());
    }

    if submit.preview && !submit.auto_yes {
        eprint!("Submit on-chain? [y/N]: ");
        io::stderr().flush()?;
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        if !matches!(buf.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            return Err(eyre::eyre!("transaction rejected by user"));
        }
    }

    let signature = send_and_confirm(rpc_url, &[ix], payer).await?;
    println!("{signature}");
    Ok(())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = std::fmt::Write::write_fmt(&mut out, format_args!("{b:02x}"));
    }
    out
}

async fn send_and_confirm(
    rpc_url: &str,
    instructions: &[Instruction],
    payer: &Keypair,
) -> eyre::Result<String> {
    let client = reqwest::Client::new();
    let blockhash = latest_blockhash(&client, rpc_url).await?;
    let mut tx = Transaction::new_with_payer(instructions, Some(&payer.pubkey()));
    tx.try_sign(&[payer], blockhash)
        .map_err(|e| eyre::eyre!("sign transaction: {e}"))?;

    let wire = bincode::serialize(&tx).map_err(|e| eyre::eyre!("serialize tx: {e}"))?;
    let encoded = STANDARD.encode(wire);

    let sig: String = rpc_call(
        &client,
        rpc_url,
        "sendTransaction",
        json!([
            encoded,
            {
                "encoding": "base64",
                "skipPreflight": false,
                "preflightCommitment": "confirmed"
            }
        ]),
    )
    .await?;

    confirm_signature(&client, rpc_url, &sig).await?;
    Ok(sig)
}

async fn latest_blockhash(client: &reqwest::Client, rpc_url: &str) -> eyre::Result<Hash> {
    #[derive(Deserialize)]
    struct Value {
        blockhash: String,
    }
    #[derive(Deserialize)]
    struct Result {
        value: Value,
    }
    let result: Result = rpc_call(
        client,
        rpc_url,
        "getLatestBlockhash",
        json!([{ "commitment": "confirmed" }]),
    )
    .await?;
    Hash::from_str(&result.value.blockhash).map_err(|e| eyre::eyre!("parse blockhash: {e}"))
}

async fn confirm_signature(
    client: &reqwest::Client,
    rpc_url: &str,
    signature: &str,
) -> eyre::Result<()> {
    for _ in 0..60 {
        #[derive(Deserialize)]
        struct Status {
            #[serde(rename = "confirmationStatus")]
            confirmation_status: Option<String>,
            err: Option<serde_json::Value>,
        }
        #[derive(Deserialize)]
        struct Result {
            value: Vec<Option<Status>>,
        }
        let result: Result = rpc_call(
            client,
            rpc_url,
            "getSignatureStatuses",
            json!([[signature], { "searchTransactionHistory": true }]),
        )
        .await?;
        if let Some(Some(status)) = result.value.into_iter().next() {
            if let Some(err) = status.err {
                return Err(eyre::eyre!("transaction failed: {err}"));
            }
            if matches!(
                status.confirmation_status.as_deref(),
                Some("confirmed") | Some("finalized")
            ) {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(eyre::eyre!(
        "timed out waiting for confirmation of {signature}"
    ))
}

async fn rpc_call<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    rpc_url: &str,
    method: &str,
    params: serde_json::Value,
) -> eyre::Result<T> {
    #[derive(Deserialize)]
    struct RpcResponse<R> {
        result: Option<R>,
        error: Option<serde_json::Value>,
    }

    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let resp: RpcResponse<T> = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| eyre::eyre!("rpc {method}: {e}"))?
        .error_for_status()
        .map_err(|e| eyre::eyre!("rpc {method} status: {e}"))?
        .json()
        .await
        .map_err(|e| eyre::eyre!("rpc {method} decode: {e}"))?;

    if let Some(err) = resp.error {
        return Err(eyre::eyre!("rpc {method} error: {err}"));
    }
    resp.result
        .ok_or_else(|| eyre::eyre!("rpc {method}: missing result"))
}
