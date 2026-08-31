#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <pubkey> <k> [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Sets an account-specific open-order limit to <k> without changing the global limit.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 2 )); then
  echo "error: user-max-orders requires a pubkey and numeric limit" >&2
  usage
  exit 2
fi

if [[ ! "$1" =~ ^[1-9A-HJ-NP-Za-km-z]{32,44}$ ]]; then
  echo "error: pubkey must be a base58-encoded Solana pubkey" >&2
  exit 2
fi

if [[ ! "$2" =~ ^[0-9]+$ ]]; then
  echo "error: maxorders must be a non-negative integer" >&2
  exit 2
fi

bulk_admin_run user-admin "$1" --maxorders "$2"
