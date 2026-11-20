# SDK V13 release signing fixtures (V12 transaction wire)

These vectors were regenerated with `bulk-transaction` at SDK revision
`a67b19f542fa32fd4bfe7843072d38b848fe11fa`, using its public
`signable_bytes_into(TransactionVersion::V12, ...)` API. They contain the numeric
little-endian `12u32` prefix and fixed V12 action fields, including ISO transfer
symbols in nested actions. Historical vector names identify the action combinations;
all vectors now use V12. Expected bytes come from the SDK, independently of this client.

The fixtures cover orders, nested multisigs, Trigger and OnFill, risk and pricing
administration, provider fee harvesting, optional subaccount margin, and ISO transfers.
Rust checks every vector and order IDs. Python checks its supported action families.
To regenerate, deserialize each recorded actions array into SDK actions and call
that revision's V12 signing API with the recorded account, nonce, and network.

`sdk-v13-vectors.json` adds risk policy, account enablement, router rotation, startup maintenance,
rebate reserves, both external reports, and nested administrative payloads. It also records canonical
SDK clear-sign text, independently of client implementations.
