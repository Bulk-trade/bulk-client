pub mod md;
pub mod subscription;
pub mod account;
pub mod responses;
pub mod meta;

pub use md::*;
pub use subscription::*;
pub use account::*;
pub use responses::*;
pub use meta::*;

/// 8-decimal fixed-point multiplier used for order-ID hashing.
#[allow(unused)]
const DECIMALS_MULTIPLIER: f64 = 100_000_000.0;


