# SDK V13 release signing fixtures (V13 transaction wire)

The action bodies in these vectors were generated with `bulk-transaction` at SDK revision
`a67b19f542fa32fd4bfe7843072d38b848fe11fa`, using its public
`signable_bytes_into(TransactionVersion::V12, ...)` API. The Rust V13 migration changes their numeric
little-endian prefix from `12u32` to `13u32` and retains the fixed non-price action fields, including ISO transfer
symbols in nested actions. Historical vector names identify the action combinations;
all vectors now use V13. Recorded clear-sign version labels and SHA-256 hashes were updated for the new prefix.

The fixtures cover orders, nested multisigs, Trigger and OnFill, risk and pricing
administration, provider fee harvesting, optional subaccount margin, and ISO transfers.
Rust checks every vector and order IDs. Python checks its supported action families.
To regenerate, deserialize each recorded actions array into SDK actions and call
the SDK V13 signing API with the recorded account, nonce, and network.

`sdk-v13-vectors.json` adds risk policy, account enablement, router rotation, startup maintenance,
rebate reserves, both external reports, and nested administrative payloads. It also records canonical
SDK clear-sign text, independently of client implementations.

Oracle interval volumes use transaction V13 (`bv` and `sv`). Nonzero price volumes and signatures are also checked against the SDK in bulk-data's `price_signature_parity` test.
