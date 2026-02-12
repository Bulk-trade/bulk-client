pub mod limit_order;
pub mod market_order;
pub mod cancel_order;
pub mod cancel_all;
pub mod oracle;
pub mod md;
pub mod subscription;
pub mod account;
pub mod responses;

/// 8-decimal fixed-point multiplier used for order-ID hashing.
#[allow(unused)]
const DECIMALS_MULTIPLIER: f64 = 100_000_000.0;

