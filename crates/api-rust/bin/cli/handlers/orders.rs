use std::sync::Arc;
use bulk_api::BulkHttpClient;
use bulk_api::common::side::Side;
use bulk_api::msgs::{LimitOrder, MarketOrder};
use bulk_api::transaction::{Action};
use crate::commands::PlaceArgs;

pub async fn handle_place(
    api: &mut BulkHttpClient,
    args: PlaceArgs
) -> eyre::Result<()> {
    let order_type = if args.qty_price.price.is_some() { "Limit" } else { "Market" };

    println!(
        "Placing {} {} {} {:?} tif={:?}{}{}",
        order_type,
        args.side,
        args.instrument,
        args.qty_price,
        args.tif,
        if args.iso { " iso" } else { "" },
        if args.reduce_only { " reduce-only" } else { "" },
    );

    let action = if args.qty_price.price.is_some() {
        Action::LimitOrder(LimitOrder {
            symbol: Arc::from(args.instrument),
            is_buy: args.side == Side::Buy,
            price: args.qty_price.price.unwrap(),
            size: args.qty_price.qty,
            tif: args.tif,
            reduce_only: args.reduce_only,
            iso: args.iso,
            meta: Default::default(),
        })
    } else {
        Action::MarketOrder(MarketOrder {
            symbol: Arc::from(args.instrument),
            is_buy: args.side == Side::Buy,
            size: args.qty_price.qty,
            reduce_only: args.reduce_only,
            iso: args.iso,
            meta: Default::default(),
        })
    };

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}\n", results);
    Ok(())
}
