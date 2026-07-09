use crate::commands::{RangeArgs, StopArgs, TrailingArgs};
use crate::common::{submit_actions, SubmitOptions};
use bulk_client::msgs::conditional::{Range, StopOrTP, Trailing};
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;
use std::sync::Arc;
// ---------------------------------------------------------------------------
// Stop
// ---------------------------------------------------------------------------

pub async fn handle_stop(
    api: &mut BulkHttpClient,
    args: StopArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    println!(
        "Placing Stop on {} | size={} threshold={} above={} limit={:?}",
        args.symbol, args.size, args.threshold, args.above, args.limit
    );

    let action = Action::Stop(StopOrTP {
        symbol: Arc::from(args.symbol.as_str()),
        is_above: args.above,
        size: args.size,
        threshold: args.threshold,
        limit: args.limit,
        meta: Default::default(),
    });

    submit_actions(api, submit, vec![action]).await
}

// ---------------------------------------------------------------------------
// TakeProfit
// ---------------------------------------------------------------------------

pub async fn handle_take_profit(
    api: &mut BulkHttpClient,
    args: StopArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    println!(
        "Placing TakeProfit on {} | size={} threshold={} above={} limit={:?}",
        args.symbol, args.size, args.threshold, args.above, args.limit
    );

    let action = Action::TakeProfit(StopOrTP {
        symbol: Arc::from(args.symbol.as_str()),
        is_above: args.above,
        size: args.size,
        threshold: args.threshold,
        limit: args.limit,
        meta: Default::default(),
    });

    submit_actions(api, submit, vec![action]).await
}

// ---------------------------------------------------------------------------
// Range (collar)
// ---------------------------------------------------------------------------

pub async fn handle_range(
    api: &mut BulkHttpClient,
    args: RangeArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    println!(
        "Placing Range on {} | size={} [{}, {}] buy={} limit_min={:?} limit_max={:?}",
        args.symbol, args.size, args.min, args.max, args.buy, args.limit_min, args.limit_max
    );

    let action = Action::Range(Range {
        symbol: Arc::from(args.symbol.as_str()),
        is_buy: args.buy,
        size: args.size,
        collar_min: args.min,
        collar_max: args.max,
        limit_min: args.limit_min,
        limit_max: args.limit_max,
        meta: Default::default(),
    });

    submit_actions(api, submit, vec![action]).await
}

// ---------------------------------------------------------------------------
// Trailing stop
// ---------------------------------------------------------------------------

pub async fn handle_trailing(
    api: &mut BulkHttpClient,
    args: TrailingArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    println!(
        "Placing TrailingStop on {} | size={} buy={} trail_bps={} step_bps={} limit={:?}",
        args.symbol, args.size, args.buy, args.trail_bps, args.step_bps, args.limit
    );

    let action = Action::Trailing(Trailing {
        symbol: Arc::from(args.symbol.as_str()),
        is_buy: args.buy,
        size: args.size,
        trail_bps: args.trail_bps,
        step_bps: args.step_bps,
        limit: args.limit,
        meta: Default::default(),
    });

    submit_actions(api, submit, vec![action]).await
}
