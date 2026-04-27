/// Stop or take-profit order.
///
/// `--above` means trigger fires when price rises above `threshold` (short stop / long TP).
/// Omit for trigger-below (long stop / short TP).
#[derive(clap::Args, Debug)]
pub struct StopArgs {
    /// Instrument symbol, e.g. BTC-USD.
    pub symbol: String,

    /// Size to execute when triggered.
    pub size: f64,

    /// Price level that arms the trigger.
    pub threshold: f64,

    /// Trigger when price is *above* the threshold (default: below).
    #[arg(long)]
    pub above: bool,

    /// Optional limit price; omit for a market trigger.
    #[arg(long)]
    pub limit: Option<f64>,
}

/// `TakeProfitArgs` is structurally identical to `StopArgs`; re-exported under
/// its own name so the CLI subcommand text differs.
pub type TakeProfitArgs = StopArgs;

// ---------------------------------------------------------------------------

/// Collar order: one stop + one take-profit in a single action.
#[derive(clap::Args, Debug)]
pub struct RangeArgs {
    /// Instrument symbol, e.g. BTC-USD.
    pub symbol: String,

    /// Size to execute when either leg triggers.
    pub size: f64,

    /// Lower collar boundary (stop for longs, TP for shorts).
    pub min: f64,

    /// Upper collar boundary (TP for longs, stop for shorts).
    pub max: f64,

    /// Set if the underlying position is long / buy-side.
    #[arg(long)]
    pub buy: bool,

    /// Optional limit price for the lower-boundary trigger.
    #[arg(long)]
    pub limit_min: Option<f64>,

    /// Optional limit price for the upper-boundary trigger.
    #[arg(long)]
    pub limit_max: Option<f64>,
}

// ---------------------------------------------------------------------------

/// Trailing stop: ratchets as price moves favorably.
#[derive(clap::Args, Debug)]
pub struct TrailingArgs {
    /// Instrument symbol, e.g. BTC-USD.
    pub symbol: String,

    /// Size to execute when the trailing stop fires.
    pub size: f64,

    /// Set if the protected position is long / buy-side.
    #[arg(long)]
    pub buy: bool,

    /// Trailing distance in basis points (e.g. 50 = 0.5%).
    #[arg(long)]
    pub trail_bps: u32,

    /// Favorable reset step in basis points before ratcheting the stop.
    #[arg(long)]
    pub step_bps: u32,

    /// Optional limit price if the stop should place a limit order on trigger.
    #[arg(long)]
    pub limit: Option<f64>,
}