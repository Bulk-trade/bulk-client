//! Binary serialization primitives.
//!
//! Two contexts use slightly different wire formats:
//!
//! | Context            | String-length prefix | Used by                    |
//! |--------------------|----------------------|----------------------------|
//! | Order-ID hashing   | `u32` (4 bytes LE)   | `LimitOrder::order_id()`   |
//! | Transaction signing| `u64` (8 bytes LE)   | `Transaction::serialize()`  |
//!
//! Both are little-endian throughout.

use eyre::bail;

/// Write a `u8` (1 byte).
#[inline]
pub fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

/// Write a `u32` in little-endian.
#[inline]
pub fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a `u64` in little-endian.
#[inline]
pub fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write an `f64` in little-endian.
#[inline]
pub fn write_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a boolean as a single byte (0 or 1).
#[inline]
pub fn write_bool(buf: &mut Vec<u8>, v: bool) {
    buf.push(if v { 1 } else { 0 });
}

/// Write a length-prefixed UTF-8 string with a **u32** length prefix.
/// Used for order-ID hash generation.
pub fn write_string_u32(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_u32(buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
}

/// Write a length-prefixed UTF-8 string with a **u64** length prefix.
/// Used for transaction serialization (signing).
pub fn write_string_u64(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_u64(buf, bytes.len() as u64);
    buf.extend_from_slice(bytes);
}

/// Decode a base58-encoded public key and write the raw 32 bytes.
/// Returns an error if the decoded key is not exactly 32 bytes.
pub fn write_pubkey(buf: &mut Vec<u8>, key_b58: &str) -> eyre::Result<()> {
    let bytes = bs58::decode(key_b58)
        .into_vec()?;
    if bytes.len() != 32 {
        bail!("invalid pubkey length: {}", bytes.len());
    }
    buf.extend_from_slice(&bytes);
    Ok(())
}

/// Decode a base58 string into exactly 32 bytes.
pub fn decode_pubkey(key_b58: &str) -> eyre::Result<[u8; 32]> {
    let bytes = bs58::decode(key_b58)
        .into_vec()?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| eyre::eyre!("invalid pubkey length"))?;
    Ok(arr)
}