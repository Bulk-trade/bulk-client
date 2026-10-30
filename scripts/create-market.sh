#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <symbol> --config-dir <dir> --executor-config <file>
          [--skip-currency] [--dry-run] [-ledger] [-url <api-url>]
          [-net <mainnet|testnet|devnet>]

Uses models/{volatility,fairprice,regime}/<symbol>.json5 and risk/<symbol>.csv.
Submits all market configuration actions in one admin multisig proposal.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# < 1 )) || [[ "$1" == -* ]]; then
  echo "error: create-market requires <symbol>" >&2
  usage
  exit 2
fi

bulk_admin_run create-market "$@"
