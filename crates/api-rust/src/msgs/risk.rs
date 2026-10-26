use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};

/// Risk policy used by the executor to recalibrate all installed risk matrices.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskConfigChange {
    /// Settlement currency used for collateral and risk calculations.
    pub settle_ccy: String,
    /// Maximum maintenance margin consumed, in percentage points in `[0, 100]`.
    /// For example, `20.0` permits consumption of 20% of maintenance margin.
    #[serde(deserialize_with = "deserialize_maxdeficit")]
    pub maxdeficit: f64,
    /// Positive multiplier on minimum liquidation volume per minute; defaults to 1.5.
    #[serde(
        default = "default_volume_scaler",
        deserialize_with = "deserialize_volume_scaler"
    )]
    pub volume_scaler: f64,
    /// Collateral margin buffer, where `0.05` is 5%.
    pub margin_buffer: f64,
    /// Multiplicative correlation adjustment in `[0, 1]`.
    pub corr_discount: f64,
    /// Fraction of positive incremental cascade premium included above raw sweep cost.
    #[serde(default = "default_cascade_factor")]
    pub cascade_factor: f64,
    #[serde(skip)]
    pub meta: ActionMeta,
}

fn default_volume_scaler() -> f64 {
    1.5
}
fn default_cascade_factor() -> f64 {
    0.25
}

/// Reject invalid MM-consumption ceilings when parsing JSON, JSON5, or bincode.
fn deserialize_maxdeficit<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err(serde::de::Error::custom(
            "maxdeficit must be finite and in [0, 100]",
        ));
    }
    Ok(value)
}

/// Reject invalid execution-volume multipliers before constructing an action.
fn deserialize_volume_scaler<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if !value.is_finite() || value <= 0.0 {
        return Err(serde::de::Error::custom(
            "volume_scaler must be finite and positive",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{Action, ClearSignMessage, SignatureDomain, Transaction};
    use solana_pubkey::Pubkey;

    fn policy() -> serde_json::Value {
        serde_json::json!({"settle_ccy":"USD", "maxdeficit":20.0,
            "margin_buffer":0.05, "corr_discount":0.5})
    }

    #[test]
    fn risk_policy_defaults_roundtrips_and_displays_every_field() {
        let mut value = policy();
        let default: RiskConfigChange = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(default.volume_scaler, 1.5);
        assert_eq!(default.cascade_factor, 0.25);
        value["volume_scaler"] = serde_json::json!(2.0);
        let explicit: RiskConfigChange = serde_json::from_value(value).unwrap();
        let bytes = bincode::serialize(&explicit).unwrap();
        let restored: RiskConfigChange = bincode::deserialize(&bytes).unwrap();
        assert_eq!(restored.volume_scaler, 2.0);
        let json = serde_json::to_value(&restored).unwrap();
        for old in ["max_loss", "eloss_floor", "max_pliq", "eloss_timescale"] {
            assert!(json.get(old).is_none());
        }
        let message = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            Pubkey::default(),
            42,
            &[Action::UpdateRiskConfig(restored)],
        )
        .unwrap();
        assert!(message.contains("UpdateRiskConfig settle_ccy=USD maxdeficit=20.00000000 volume_scaler=2.00000000 margin_buffer=0.05000000 corr_discount=0.50000000 cascade_factor=0.25000000"));
    }

    #[test]
    fn risk_policy_rejects_invalid_values_and_requires_new_ceiling() {
        for ceiling in [0.0, 20.0, 100.0] {
            let mut value = policy();
            value["maxdeficit"] = serde_json::json!(ceiling);
            assert!(serde_json::from_value::<RiskConfigChange>(value).is_ok());
        }
        for (field, invalid) in [
            ("maxdeficit", -1.0),
            ("maxdeficit", 101.0),
            ("volume_scaler", 0.0),
            ("volume_scaler", -1.0),
        ] {
            let mut value = policy();
            value[field] = serde_json::json!(invalid);
            assert!(serde_json::from_value::<RiskConfigChange>(value).is_err());
        }
        let mut legacy = policy();
        legacy.as_object_mut().unwrap().remove("maxdeficit");
        legacy["max_loss"] = serde_json::json!(15000.0);
        assert!(serde_json::from_value::<RiskConfigChange>(legacy).is_err());
        for field in ["maxdeficit", "volume_scaler"] {
            for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut risk: RiskConfigChange = serde_json::from_value(policy()).unwrap();
                if field == "maxdeficit" {
                    risk.maxdeficit = invalid;
                } else {
                    risk.volume_scaler = invalid;
                }
                assert!(bincode::deserialize::<RiskConfigChange>(
                    &bincode::serialize(&risk).unwrap()
                )
                .is_err());
            }
        }
    }

    #[test]
    fn risk_policy_signing_commits_to_every_field() {
        let base: RiskConfigChange = serde_json::from_value(policy()).unwrap();
        let bytes = |risk| {
            Transaction::raw_signable_bytes(
                SignatureDomain::Devnet,
                Pubkey::default(),
                42,
                &[Action::UpdateRiskConfig(risk)],
            )
            .unwrap()
        };
        let original = bytes(base.clone());
        for changed in [
            RiskConfigChange {
                settle_ccy: "USDC".into(),
                ..base.clone()
            },
            RiskConfigChange {
                maxdeficit: 30.0,
                ..base.clone()
            },
            RiskConfigChange {
                volume_scaler: 2.0,
                ..base.clone()
            },
            RiskConfigChange {
                margin_buffer: 0.1,
                ..base.clone()
            },
            RiskConfigChange {
                corr_discount: 0.8,
                ..base.clone()
            },
            RiskConfigChange {
                cascade_factor: 0.5,
                ..base.clone()
            },
        ] {
            assert_ne!(original, bytes(changed));
        }
    }
}
