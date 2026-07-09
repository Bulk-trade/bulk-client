use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A map between instrument and value
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InstrumentConfig<T> {
    pub default: T,
    pub instruments: HashMap<String, T>,
}

impl<T> InstrumentConfig<T> {
    pub fn new(default: T, instruments: HashMap<String, T>) -> Self {
        Self {
            default,
            instruments,
        }
    }

    /// Get appropriate value for instrument
    ///
    /// # Arguments
    /// - `id`: instrument ID
    ///
    /// # Returns
    /// - returns value associated with instrument or default
    pub fn value_for(&self, id: &str) -> &T {
        if let Some(instrument) = self.instruments.get(id) {
            instrument
        } else {
            &self.default
        }
    }
}
