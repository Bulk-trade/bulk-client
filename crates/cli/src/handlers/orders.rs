use crate::commands::{ModifyArgs, PlaceArgs};
use crate::common::{submit_actions, SubmitOptions};
use bulk_client::common::side::Side;
use bulk_client::msgs::{LimitOrder, MarketOrder, ModifyOrder};
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;
use std::sync::Arc;

pub async fn handle_place(
    api: &mut BulkHttpClient,
    args: PlaceArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    if args.qty_price.price.is_some() && args.slippage.is_some() {
        eyre::bail!("--slippage is only valid for market orders");
    }

    let order_type = if args.qty_price.price.is_some() {
        "Limit"
    } else {
        "Market"
    };
    let market_slippage = args.qty_price.price.is_none().then(|| {
        args.slippage
            .unwrap_or(bulk_client::msgs::DEFAULT_MARKET_SLIPPAGE_BPS)
    });

    println!(
        "Placing {} {} {} {:?} tif={:?}{}{}{}",
        order_type,
        args.side,
        args.instrument,
        args.qty_price,
        args.tif,
        if args.iso { " iso" } else { "" },
        if args.reduce_only { " reduce-only" } else { "" },
        market_slippage
            .map(|bps| format!(" slippage={bps}bps"))
            .unwrap_or_default(),
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
            builder_code: None,
            meta: Default::default(),
        })
    } else {
        Action::MarketOrder(MarketOrder {
            symbol: Arc::from(args.instrument),
            is_buy: args.side == Side::Buy,
            size: args.qty_price.qty,
            reduce_only: args.reduce_only,
            iso: args.iso,
            builder_code: None,
            slippage: market_slippage,
            meta: Default::default(),
        })
    };

    submit_actions(api, submit, vec![action]).await
}

pub async fn handle_modify(
    api: &mut BulkHttpClient,
    args: ModifyArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    println!(
        "Modifying order {} on {} → size {}",
        args.order_id, args.symbol, args.size
    );

    let action = Action::ModifyOrder(ModifyOrder {
        order_id: args.order_id,
        symbol: args.symbol,
        amount: args.size,
        meta: Default::default(),
    });

    submit_actions(api, submit, vec![action]).await
}
