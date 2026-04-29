// ---------------------------------------------------------------------------
// CancelArgs
// ---------------------------------------------------------------------------

use std::str::FromStr;
use solana_hash::Hash;

#[derive(clap::Args, Debug)]
pub struct CancelArgs {
    /// Which instrument is this order under
    pub symbol: String,

    /// Order-id to cancel.  May be prefixed with `oid=` for scripting clarity.
    #[arg(value_parser = parse_oid)]
    pub order_id: Hash,
}

// ---------------------------------------------------------------------------
// CancelAllArgs
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct CancelAllArgs {
    /// If supplied, only cancel orders for this instrument.
    #[arg(long)]
    pub instrument: Option<String>,
}



/// Strip an optional `oid=` prefix so users can write either form.
fn parse_oid(s: &str) -> eyre::Result<Hash, String> {
    let raw = s.strip_prefix("oid=").unwrap_or(s);
    Hash::from_str(raw).map_err(|e| e.to_string())
}
