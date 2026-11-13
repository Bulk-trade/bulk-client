use bulk_client::msgs::{Rotate, RotateCategory, RotateMode};
use bulk_client::transaction::Action;
use solana_pubkey::Pubkey;
#[test]
fn rotate_matches_sdk_wire_and_json() {
    for (category, name, ordinal) in [
        (RotateCategory::PythOracle, "pythOracle", 0u32),
        (RotateCategory::Oracle, "oracle", 1),
        (RotateCategory::Liquidator, "liquidator", 2),
    ] {
        for (mode, mode_name, mode_ordinal) in [
            (RotateMode::Replace, "replace", 0u32),
            (RotateMode::Append, "append", 1),
        ] {
            for keys in [vec![], vec![Pubkey::from([7; 32])]] {
                let action = Action::from(Rotate {
                    category,
                    mode,
                    keys: keys.clone(),
                    meta: Default::default(),
                });
                assert!(action.is_admin_multisig_action());
                let mut expected = Vec::new();
                expected.extend(70u32.to_le_bytes());
                expected.extend(ordinal.to_le_bytes());
                expected.extend(mode_ordinal.to_le_bytes());
                expected.extend((keys.len() as u64).to_le_bytes());
                for key in &keys {
                    expected.extend(key.as_ref());
                }
                let bytes = bincode::serialize(&action).unwrap();
                assert_eq!(bytes, expected);
                let decoded: Action = bincode::deserialize(&bytes).unwrap();
                let json = serde_json::json!({"rotate": {"category": name, "mode": mode_name, "keys": keys.iter().map(ToString::to_string).collect::<Vec<_>>()}});
                assert_eq!(serde_json::to_value(decoded).unwrap(), json);
                let restored: Action = serde_json::from_value(json).unwrap();
                assert_eq!(bincode::serialize(&restored).unwrap(), bytes);
            }
        }
    }
}
