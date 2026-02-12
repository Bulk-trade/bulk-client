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

use solana_pubkey::Pubkey;

/// Write a `u8` (1 byte).
#[inline]
#[allow(unused)]
pub fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

/// Write a `u32` in little-endian.
#[inline]
#[allow(unused)]
pub fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a `u64` in little-endian.
#[inline]
#[allow(unused)]
pub fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write an `f64` in little-endian.
#[inline]
#[allow(unused)]
pub fn write_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a boolean as a single byte (0 or 1).
#[inline]
#[allow(unused)]
pub fn write_bool(buf: &mut Vec<u8>, v: bool) {
    buf.push(if v { 1 } else { 0 });
}

/// Write a length-prefixed UTF-8 string with a **u32** length prefix.
/// Used for order-ID hash generation.
#[allow(unused)]
pub fn write_string_u32(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_u32(buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
}

/// Write a length-prefixed UTF-8 string with a **u64** length prefix.
/// Used for transaction serialization (signing).
#[allow(unused)]
pub fn write_string_u64(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_u64(buf, bytes.len() as u64);
    buf.extend_from_slice(bytes);
}

/// Write a [`Pubkey`]'s raw 32 bytes into the buffer.
#[inline]
#[allow(unused)]
pub fn write_pubkey_bytes(buf: &mut Vec<u8>, key: &Pubkey) {
    buf.extend_from_slice(&key.to_bytes());
}
