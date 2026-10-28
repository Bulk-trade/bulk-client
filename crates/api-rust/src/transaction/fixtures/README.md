# SDK V12 signing fixtures

These vectors were regenerated with `bulk-transaction` at SDK revision
`eb161fdf18e113eb075dd985d4d0287e66363a52`, using its public
`signable_bytes_into(TransactionVersion::V12, ...)` API. They contain the numeric
little-endian `12u32` prefix and fixed V12 action fields, including ISO transfer
symbols in nested actions. Historical vector names identify the action combinations;
all vectors now use V12. Expected bytes come from the SDK, independently of this client.

The fixtures cover orders, nested multisigs, Trigger and OnFill, risk and pricing
administration, provider fee harvesting, optional subaccount margin, and ISO transfers.
Rust checks every vector and order IDs. Python checks its supported action families.
To regenerate, deserialize each recorded actions array into SDK actions and call
that revision's V12 signing API with the recorded account, nonce, and network.
