//! Positive eight-decimal wire values used by SDK external-report payloads.
use serde::{Deserialize, Deserializer, Serializer};
const SCALE: f64 = 100_000_000.0;
fn units(value: f64) -> Option<u64> {
    let scaled = value * SCALE;
    if !value.is_finite() || value <= 0.0 || !scaled.is_finite() || scaled >= u64::MAX as f64 {
        return None;
    }
    let units = scaled.round() as u64;
    (units != 0 && units != u64::MAX).then_some(units)
}
fn stable(value: u64) -> Option<f64> {
    let bits = (value as f64 / SCALE).to_bits();
    [bits, bits.saturating_sub(1), bits.saturating_add(1)]
        .into_iter()
        .map(f64::from_bits)
        .find(|candidate| units(*candidate) == Some(value))
}
/// Encode a positive SDK decimal using canonical JSON or fixed-point binary units.
///
/// # Arguments
/// * `value` - Decimal value to encode.
/// * `serializer` - Destination format.
///
/// # Returns
/// Encoded output, or an error for invalid or unrepresentable values.
pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    let fixed = units(*value).ok_or_else(|| {
        serde::ser::Error::custom("SDK report value must be finite, positive, and representable")
    })?;
    if serializer.is_human_readable() {
        serializer.serialize_str(
            &stable(fixed)
                .ok_or_else(|| serde::ser::Error::custom("unstable SDK report value"))?
                .to_string(),
        )
    } else {
        serializer.serialize_u64(fixed)
    }
}
/// Decode a positive SDK decimal from JSON numbers/strings or binary units.
///
/// # Arguments
/// * `deserializer` - Source format.
///
/// # Returns
/// A stable decimal, or an error for invalid or unrepresentable values.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    if !deserializer.is_human_readable() {
        let fixed = u64::deserialize(deserializer)?;
        return stable(fixed).ok_or_else(|| serde::de::Error::custom("invalid SDK report units"));
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Numeric(f64),
        Text(String),
    }
    let value = match Number::deserialize(deserializer)? {
        Number::Numeric(value) => value,
        Number::Text(text) => text.parse::<f64>().map_err(serde::de::Error::custom)?,
    };
    units(value).and_then(stable).ok_or_else(|| {
        serde::de::Error::custom("SDK report value must be finite, positive, and representable")
    })
}
