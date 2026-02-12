use std::fmt::Write;
use std::fmt;
use crate::common::{write_string_u64, write_u32, write_u64};


/// Cancel All.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct CancelAll {
    pub symbols: Vec<String>,
}

#[allow(unused)]
impl CancelAll {
    pub fn new(
        symbols: Vec<String>,
    ) -> Self {
        Self {
            symbols,
        }
    }

    /// Produce the compact JSON payload expected by the exchange API.
    pub fn write_api(&self, buf: &mut String) {
        buf.push_str(r#"{"cancelAll":{"c":["#);
        for (i, sym) in self.symbols.iter().enumerate() {
            if i > 0 { buf.push(','); }
            write!(buf, r#""{}""#, sym).unwrap();
        }
        buf.push_str("]}}");
    }

    /// Serialize for inclusion in a **transaction** (signing context).
    ///
    /// Uses the `u64` string-length prefix convention.
    pub fn serialize_for_tx(&self, buf: &mut Vec<u8>) -> eyre::Result<()> {
        // order type tag = 2 (cancelAll)
        write_u32(buf, 2);
        write_u64(buf, self.symbols.len() as u64);
        for sym in &self.symbols {
            write_string_u64(buf, sym);
        }
        Ok(())
    }
}

impl fmt::Display for CancelAll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancelAll({:?})", self.symbols)
    }
}
