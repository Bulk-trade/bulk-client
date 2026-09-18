# SDK signing fixtures

`sdk-signing-vectors.json` was generated from the actual `bulk-transaction` crate
at SDK commit `bc0ea23f17b42090d73177beb424ad6516d58386`, not from this client's
serializer. Each actions array was deserialized into SDK `Vec<Action>` and passed
to `bulk_transaction::transaction::signable_bytes_into` with the recorded domain,
account and nonce. `default_meta_order_ids` records SDK `Action::hash()` for
market/limit actions with their untouched default metadata; other entries are null.

Coverage includes legacy and V2 orders without builder codes, V3 LOB and COB
orders with builder codes, nested multisigs, Trigger and OnFill, and historic
embedded multisig layouts.
Rust checks all vectors and both order-ID entry points. Python checks its supported
actions and asserts that unsupported multisig proposals are rejected.

To regenerate, use that SDK crate with `serde` and `hash` features, deserialize
each recorded actions array, call those public APIs and replace only the expected
fields. Do not derive expected bytes or IDs from the client under test.
