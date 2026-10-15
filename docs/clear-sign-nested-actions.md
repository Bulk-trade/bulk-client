# Nested actions in transaction clear-sign messages

`ClearSignMessage` renders each trigger basket and on-fill consequent using the
same recursive action paths already used for multisig proposals. Children appear
in execution order, immediately after their parent. The `Actions:` header still
counts top-level actions.

For example, a trigger with two child actions includes:

```text
[0] Trigger BTC-USD Above thresh=100000.00000000 nested=2
[0.0] CancelAll *
[0.1] Market BTC-USD Sell sz=0.10000000 ro=true iso=false
```

This makes the operations visible before approval; the signable hash alone does
not communicate their meaning to a user. On-fill parent descriptions remain
unchanged, and their consequent actions now receive recursive action lines.

## Compatibility and rollout

This changes canonical clear-sign text for transactions containing nonempty
Trigger or OnFill action lists, including those nested inside multisig proposals.
It does not change raw binary action serialization or the signable hash.

Offchain signatures cover the clear-sign text. A verifier reconstructing the old
text will reject signatures created with this renderer. Coordinate server-side
canonical rendering and client releases before deploying this change. Maintainers
must decide whether explicit format versioning or a migration window is required;
this patch does not implement a server-side transition.

Do not silently fall back to raw signing on a verification error: that would
remove the readable approval flow. Existing payload-size limits continue to apply
and must be checked with realistic nested transactions during rollout testing.
