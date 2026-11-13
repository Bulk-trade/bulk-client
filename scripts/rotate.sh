#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <pythOracle|oracle|liquidator> <replace|append> [pubkey[,pubkey...]] [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet|local>]

Proposes a controller-key update through the protocol admin multisig.
Separate multiple public keys with commas or spaces.
An empty replace clears the selected set; an empty append leaves it unchanged.

Example: $0 oracle append <pubkey1>,<pubkey2> -ledger -net testnet
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# < 2 )); then
  echo "error: rotate requires a category and mode" >&2
  usage
  exit 2
fi

case "$1" in
  pythOracle|oracle|liquidator) ;;
  *)
    echo "error: category must be one of: pythOracle, oracle, liquidator" >&2
    exit 2
    ;;
esac

case "$2" in
  replace|append) ;;
  *)
    echo "error: mode must be one of: replace, append" >&2
    exit 2
    ;;
esac

for keys in "${@:3}"; do
  if [[ ! "${keys}" =~ ^[1-9A-HJ-NP-Za-km-z]{32,44}(,[1-9A-HJ-NP-Za-km-z]{32,44})*$ ]]; then
    echo "error: pubkeys must be base58-encoded Solana pubkeys" >&2
    exit 2
  fi
done

bulk_admin_run rotate "$@"
