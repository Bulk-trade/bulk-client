//! Protocol-admin actions for allocating market rebate reserves.

use crate::transaction::ActionMeta;

// ───── Rebate Pool Allocation ────────────────────────────────────────────────────────────────────

/// Replaces selected markets' rebate reserves; omitted markets are unchanged.
///
/// Allocations are settlement-currency amounts, rounded to six decimal places.
/// Only the configured protocol admin multisig may execute this action.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RebatePoolAllocation {
    /// Market symbol to absolute reserve amount. Zero releases the entire reserve.
    #[serde(with = "rebate_allocation_map")]
    pub allocations: std::collections::BTreeMap<String, f64>,
    #[serde(skip)]
    pub meta: ActionMeta,
}

impl RebatePoolAllocation {
    /// Converts an allocation to six-decimal unsigned fixed-point units.
    ///
    /// # Arguments
    /// * `amount` - Nonnegative, finite settlement-currency allocation.
    ///
    /// # Returns
    /// Rounded micro-units, or an error for invalid or overflowing amounts.
    pub fn allocation_units(amount: f64) -> eyre::Result<u64> {
        let scaled = amount * 1_000_000.0;
        eyre::ensure!(
            amount.is_finite() && amount >= 0.0 && scaled.round() < u64::MAX as f64,
            "rebate allocation must be finite, nonnegative, and fit in u64 micro-units"
        );
        Ok(scaled.round() as u64)
    }
}

mod rebate_allocation_map {
    use super::RebatePoolAllocation;
    use serde::ser::SerializeMap;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::collections::BTreeMap;

    pub(super) fn serialize<S: Serializer>(
        values: &BTreeMap<String, f64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let human = serializer.is_human_readable();
        let mut map = serializer.serialize_map(Some(values.len()))?;
        for (symbol, amount) in values {
            let units = RebatePoolAllocation::allocation_units(*amount)
                .map_err(serde::ser::Error::custom)?;
            let canonical = from_units(units).map_err(serde::ser::Error::custom)?;
            if human {
                map.serialize_entry(symbol, &canonical)?;
            } else {
                map.serialize_entry(symbol, &units)?;
            }
        }
        map.end()
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<String, f64>, D::Error> {
        if deserializer.is_human_readable() {
            BTreeMap::<String, f64>::deserialize(deserializer)?
                .into_iter()
                .map(|(symbol, amount)| {
                    let units = RebatePoolAllocation::allocation_units(amount)
                        .map_err(serde::de::Error::custom)?;
                    Ok((symbol, from_units(units).map_err(serde::de::Error::custom)?))
                })
                .collect()
        } else {
            BTreeMap::<String, u64>::deserialize(deserializer)?
                .into_iter()
                .map(|(symbol, units)| {
                    Ok((symbol, from_units(units).map_err(serde::de::Error::custom)?))
                })
                .collect()
        }
    }

    // Find a stable f64 so binary decoding and subsequent signing retain the exact micro-units.
    pub(super) fn from_units(units: u64) -> Result<f64, &'static str> {
        let bits = (units as f64 / 1_000_000.0).to_bits();
        for candidate in [bits, bits.saturating_sub(1), bits.saturating_add(1)] {
            let amount = f64::from_bits(candidate);
            if RebatePoolAllocation::allocation_units(amount).ok() == Some(units) {
                return Ok(amount);
            }
        }
        Err("rebate allocation has no stable f64 representation")
    }
}

/// Returns the canonical in-memory allocation for six-decimal wire units.
///
/// # Arguments
/// * `units` - Unsigned settlement-currency micro-units.
///
/// # Returns
/// A stable floating-point amount, or an error when the units cannot round-trip.
pub(crate) fn rebate_allocation_amount(units: u64) -> Result<f64, &'static str> {
    rebate_allocation_map::from_units(units)
}
