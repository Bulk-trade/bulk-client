# Bulk Clear Sign v1

This document defines the human-readable clear-sign envelope used by `bulk-cli`.

## Goals

- Show users exactly what is being approved.
- Keep signing deterministic across implementations.
- Bind clear-sign payload to execution nonce and account.

## Canonical Message

The canonical message is newline-delimited UTF-8:

```text
domain=bulk
intent_version=1
api_url=<normalized_api_url_without_trailing_slash>
account=<base58_pubkey>
nonce=<u64>
expires_at=<unix_seconds>
actions=<count>
0:<action_line_0>
1:<action_line_1>
...
```

Action lines are emitted in transaction order.

## Intent Hash

`intent_hash = sha256("bulk-clear-sign-v1\n" + canonical_message_bytes)`

Hash is rendered as lowercase hex.

## Signature

The clear-sign signature is Ed25519 over `canonical_message_bytes`.

In the current implementation this is signed with the same software key used for
execution signing.

## Expiry

`expires_at` is `now_unix + intent_ttl_secs` where `intent_ttl_secs >= 1`.

Default in CLI: `120` seconds.

## CLI Controls

- `--clear-sign <true|false>`: enable/disable clear-sign flow.
- `--yes`: bypass interactive clear-sign confirmation prompt.
- `--intent-ttl-secs <u64>`: expiry horizon for clear-sign payload.
- `--ledger`: reserved for future Solana-app ledger signer mode.
