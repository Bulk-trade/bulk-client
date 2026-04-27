use std::str::FromStr;
use eyre::{bail, Context};

/// `qty` for market orders, `qty@price` for limit orders.
#[derive(Debug, Clone)]
pub struct QtyPrice {
    pub qty: f64,
    /// `None` → market order.
    pub price: Option<f64>,
}

impl FromStr for QtyPrice {
    type Err = eyre::Error;

    fn from_str(s: &str) -> eyre::Result<Self> {
        match s.split_once('@') {
            Some((qty_part, price_part)) => {
                let qty = f64::from_str(qty_part).with_context(|| {
                    format!("invalid quantity '{qty_part}' in '{s}'")
                })?;
                let price = f64::from_str(price_part).with_context(|| {
                    format!("invalid price '{price_part}' in '{s}'")
                })?;
                if qty <= 0.0 {
                    bail!("quantity must be positive, got {qty}");
                }
                if price <= 0.0 {
                    bail!("price must be positive, got {price}");
                }
                Ok(QtyPrice { qty, price: Some(price) })
            }
            None => {
                let qty = f64::from_str(s).with_context(|| {
                    format!(
                        "invalid qty@price format '{s}'\n  \
                         → expected <qty>@<price> (e.g. 1.3@70000) or <qty> for a market order"
                    )
                })?;
                if qty <= 0.0 {
                    bail!("quantity must be positive, got {qty}");
                }
                Ok(QtyPrice { qty, price: None })
            }
        }
    }
}
