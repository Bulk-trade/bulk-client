use bulk_client::common::WireMktId;
use bulk_client::msgs::OpaqueAction;
use bulk_client::transaction::Action;
use bulk_sdk_core::markets::MktId;
use bulk_sdk_core::models::margin::RiskMatrix;
use bulk_sdk_core::securities::Security;
use bulk_sdk_core::time_epoch_ns;

/// Converts a native SDK market identifier into its public wire representation.
///
/// # Arguments
/// * `market_id` - Native SDK identifier to convert.
pub fn wire_mktid(market_id: MktId) -> WireMktId {
    WireMktId(market_id.uuid)
}

/// Builds a risk-matrix action using native SDK parsing and validation.
///
/// # Arguments
/// * `coin` - Registered SDK security name.
/// * `csv` - Path to the risk-surface CSV.
///
/// # Returns
/// An error when the security is unknown, CSV is invalid, or serialization fails.
pub fn config_risk_action(coin: &str, csv: &str) -> eyre::Result<Action> {
    Security::initialize();
    let instrument =
        MktId::from_str(coin).ok_or_else(|| eyre::eyre!("invalid risk-surface coin '{coin}'"))?;
    let matrix = RiskMatrix::from_csv(time_epoch_ns(), instrument, csv)
        .map_err(|error| eyre::eyre!("invalid risk-surface CSV '{csv}': {error}"))?;
    opaque_action(matrix, Action::ConfigRisk, "risk surface")
}

/// Builds a security-configuration action using the native SDK security type.
///
/// # Arguments
/// * `json` - Inline JSON/JSON5 containing one complete security definition.
///
/// # Returns
/// An error when parsing or serialization fails.
pub fn config_security_action(json: &str) -> eyre::Result<Action> {
    let security: Security =
        json5::from_str(json).map_err(|error| eyre::eyre!("invalid security config: {error}"))?;
    opaque_action(security, Action::ConfigSecurity, "security config")
}

fn opaque_action<T>(
    value: T,
    wrap: impl FnOnce(OpaqueAction) -> Action,
    label: &str,
) -> eyre::Result<Action>
where
    T: serde::Serialize,
{
    let payload = bincode::serialize(&value)
        .map_err(|error| eyre::eyre!("failed to serialize {label}: {error}"))?;
    Ok(wrap(OpaqueAction {
        payload,
        meta: Default::default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_proxy_matches_native_sdk_market_id_encoding() {
        let sdk_id = MktId {
            uuid: 0x0102_0304_0506_0708,
        };

        assert_eq!(
            bincode::serialize(&sdk_id).expect("serialize SDK market ID"),
            bincode::serialize(&wire_mktid(sdk_id)).expect("serialize wire market ID")
        );
    }
}
