use std::collections::HashMap;
use std::sync::Arc;
use bulk_api::BulkHttpClient;
use bulk_api::msgs::{AgentWalletCreation, Faucet, UpdateUserSettings};
use bulk_api::msgs::subaccounts::{CreateSubAccount, RemoveSubAccount, Transfer};
use bulk_api::transaction::Action;
use crate::commands::{AgentWalletArgs, CreateSubAccountArgs, FaucetArgs, RemoveSubAccountArgs, TransferArgs, UpdateLeverageArgs};

pub async fn handle_faucet(
    api: &mut BulkHttpClient,
    args: FaucetArgs,
) -> eyre::Result<()> {
    let account = api.public_key().unwrap();
    println!("Faucet request for account {}", account);

    let action = Action::Faucet(Faucet {
        user: account,
        amount: args.amount,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// Leverage
// ---------------------------------------------------------------------------

pub async fn handle_update_leverage(
    api: &mut BulkHttpClient,
    args: UpdateLeverageArgs,
) -> eyre::Result<()> {
    for (sym, lev) in &args.settings {
        println!("  {sym} → {lev}x");
    }

    let max_leverage: HashMap<String, f64> = args.settings.into_iter().collect();

    let action = Action::UpdateUserSettings(UpdateUserSettings {
        max_leverage,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// Agent wallet
// ---------------------------------------------------------------------------

pub async fn handle_agent_wallet(
    api: &mut BulkHttpClient,
    args: AgentWalletArgs,
) -> eyre::Result<()> {
    let verb = if args.delete { "Removing" } else { "Adding" };
    println!("{verb} agent wallet {}", args.agent);

    let action = Action::AgentWalletCreation(AgentWalletCreation {
        agent: args.agent,
        delete: args.delete,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// CreateSubAccount
// ---------------------------------------------------------------------------

pub async fn handle_create_subaccount(
    api: &mut BulkHttpClient,
    args: CreateSubAccountArgs,
) -> eyre::Result<()> {
    println!(
        "Creating sub-account '{}' margin_symbol={:?} margin_amount={:?}",
        args.name, args.margin_symbol, args.margin_amount
    );

    let action = Action::CreateSubAccount(CreateSubAccount {
        name: Arc::from(args.name.as_str()),
        margin_symbol: args.margin_symbol.map(|s| Arc::from(s.as_str())),
        margin_amount: args.margin_amount,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// RemoveSubAccount
// ---------------------------------------------------------------------------

pub async fn handle_remove_subaccount(
    api: &mut BulkHttpClient,
    args: RemoveSubAccountArgs,
) -> eyre::Result<()> {
    println!("Removing sub-account {}", args.pubkey);

    let action = Action::RemoveSubAccount(RemoveSubAccount {
        to_remove: args.pubkey,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer
// ---------------------------------------------------------------------------

pub async fn handle_transfer(
    api: &mut BulkHttpClient,
    args: TransferArgs,
) -> eyre::Result<()> {
    println!(
        "Transferring {} {} from {} → {} ({:?})",
        args.amount, args.symbol, args.from, args.to, args.kind
    );

    let action = Action::Transfer(Transfer {
        kind: args.kind,
        from: args.from,
        to: args.to,
        margin_symbol: Arc::from(args.symbol.as_str()),
        margin_amount: args.amount,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}