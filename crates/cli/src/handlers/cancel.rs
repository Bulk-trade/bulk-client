use bulk_client::BulkHttpClient;
use bulk_client::msgs::{CancelAll, CancelOrder};
use bulk_client::transaction::Action;
use crate::commands::{CancelAllArgs, CancelArgs};
use crate::common::{submit_actions, SubmitOptions};

pub async fn handle_cancel(
    api: &mut BulkHttpClient,
    args: CancelArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    println!("Cancelling order {}", args.order_id);

    let action = Action::Cancel(CancelOrder {
        symbol: args.symbol,
        oid: args.order_id,
        meta: Default::default(),
    });

    submit_actions(api, submit, vec![action]).await
}

pub async fn handle_cancel_all(
    api: &mut BulkHttpClient,
    args: CancelAllArgs,
    submit: &SubmitOptions,
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

    submit_actions(api, submit, vec![action]).await
}
