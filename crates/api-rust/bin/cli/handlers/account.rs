use bulk_api::BulkHttpClient;
use bulk_api::msgs::{CancelOrder, Faucet};
use bulk_api::transaction::Action;
use crate::commands::{CancelArgs, FaucetArgs};

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