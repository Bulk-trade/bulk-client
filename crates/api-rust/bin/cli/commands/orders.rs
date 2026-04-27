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
