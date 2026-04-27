use std::str::FromStr;
use solana_hash::Hash;
use bulk_api::common::side::Side;
use bulk_api::common::tif::TimeInForce;
use crate::common::QtyPrice;
// ---------------------------------------------------------------------------
// Place limit order
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct PlaceArgs {
    /// Buy or Sell.
    pub side: Side,

    /// Instrument symbol, e.g. BTC-USD.
    pub instrument: String,

    /// Quantity and optional limit price as `qty@price` (limit) or bare `qty` (market).
    pub qty_price: QtyPrice,

    /// GTC, ALO, IOC
    #[arg(long, default_value = "GTC")]
    pub tif: TimeInForce,

    /// Mark the order as an Isolated-margin order.
    #[arg(long)]
    pub iso: bool,

    /// Reduce an existing position only; reject if it would open or increase.
    #[arg(long)]
    pub reduce_only: bool,

    /// Arbitrary client-supplied tag carried on the order record.
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ModifyArgs {
    /// Instrument the order is on.
    pub symbol: String,

    /// Order-id to modify. May be prefixed with `oid=` for scripting clarity.
    #[arg(value_parser = parse_oid)]
    pub order_id: Hash,

    /// New absolute size for the order.
    pub size: f64,
}

fn parse_oid(s: &str) -> Result<Hash, String> {
    let raw = s.strip_prefix("oid=").unwrap_or(s);
    Hash::from_str(raw).map_err(|e| e.to_string())
}