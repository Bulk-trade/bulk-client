# Create a BIP1 market proposal

`bulk-sdk create-bip1` reads the configuration tree and submits one proposal to the
existing protocol admin multisig. Config discovery, reference substitution, validation,
binary encoding, signing, and submission all run in Rust. No manifest or Python is required.

Before running, add the currency to `securities/Currencies.json` and its `FOO-USD`
perpetual to `securities/Perpetuals.json`. The complete security directory must include
USD and resolve all security dependencies. Each definition must have a unique ID.

From bulk-client, build the utility:

```sh
cargo build --manifest-path crates/cli-sdk/Cargo.toml --bin bulk-sdk
```

Validate without a signer (requires the target API to read its current liquidator config):

```sh
crates/cli-sdk/target/debug/bulk-sdk create-bip1 PAPER high \
  --config-dir ../bulk-sdk/config \
  --executor-config ../bulk-sdk/config/executor-testnet.json5 \
  --api-url http://64.130.50.69:12000/api/v1 \
  --dry-run
```

Submit to staging/devnet:

```sh
crates/cli-sdk/target/debug/bulk-sdk create-bip1 PAPER high \
  --config-dir ../bulk-sdk/config \
  --executor-config ../bulk-sdk/config/executor-testnet.json5 \
  --api-url http://64.130.50.69:12000/api/v1 \
  --signature-domain devnet
```

Set `BULK_PRIVATE_KEY` to an authorized admin signer, or add `--ledger`. The default
shows the canonical transaction and requests confirmation. `--yes` skips confirmation;
`--preview false --yes` also skips rendering the canonical preview, which can be slow
for a large risk matrix. `--dry-run` still prints every encoded action for inspection.

Action order is currency, perpetual, volatility, fair price, regime, currency risk matrix,
funding, `AddMarket`, `UpdateLiquidatorConfig`, and finally `MarketAdmin(Suspend)`.
The market starts suspended. Use `--skip-currency` if the underlier already exists on the target
exchange; there is no automatic existence detection. This yields nine actions instead
of ten. The command submits the proposal; multisig approval and execution follow the
normal protocol policy.

Choose `low`, `medium`, or `high`. Models come from
`models/{volatility,fairprice,regime}/ref/{low,medium,high}.json5` (the alternative
`BIP1_LOW.json5` naming is also supported). Every `SYMBOL` placeholder is replaced with
the currency. Risk comes from `risk/ref/{low,medium,high}.csv`. Instrument-specific
reference values, such as the regime trend volume threshold, should be reviewed.
Runtime config files are not modified.

Funding comes from the explicitly selected executor JSON5 file. An exact pair entry,
`funding["PAPER-USD"]`, takes precedence over `funding["*"]`. The selected entry must
contain every funding parameter; partial overrides are not merged. The SDK validates
funding against the configured perpetual before submission.

The former Python wrapper and `market-proposal` manifest command have been replaced by
`create-bip1`. Each invocation reads the current configs and submits a fresh proposal;
do not blindly retry after an uncertain response.

Run tests with:

```sh
cargo test --manifest-path crates/cli-sdk/Cargo.toml
```

Liquidator configuration is fetched from the selected API's `/config?which=liquidator`
endpoint on every invocation, including dry runs. The `*` instrument policy is copied
in full to the new pair, preserving the wildcard, global settings, and existing pairs.
The complete updated configuration is included in the same multisig proposal.
A missing or duplicate wildcard, existing pair policy, or failed API request aborts
proposal creation. Because this replaces the full liquidator config, review pending
config changes before approving the proposal to avoid overwriting intervening updates.
