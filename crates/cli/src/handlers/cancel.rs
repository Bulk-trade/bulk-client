use bulk_client::BulkHttpClient;
use bulk_client::msgs::{CancelAll, CancelOrder};
use bulk_client::transaction::Action;
use crate::commands::{CancelAllArgs, CancelArgs};

pub async fn handle_cancel(
    api: &mut BulkHttpClient,
    args: CancelArgs
) -> eyre::Result<()> {
    println!("Cancelling order {}", args.order_id);

    let action = Action::Cancel(CancelOrder {
        symbol: args.symbol,
        oid: args.order_id,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

pub async fn handle_cancel_all(
    api: &mut BulkHttpClient,
    args: CancelAllArgs
) -> eyre::Result<()> {
    let symbols = match &args.instrument {
        Some(inst) => {
            println!("Cancelling all orders for {inst}");
            vec![inst.clone()]
        },
        None => {
            println!("Cancelling all open orders");
            vec![]
        },
    };

    let action = Action::CancelAll(CancelAll {
        symbols,
        meta: Default::default(),
    });

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}
