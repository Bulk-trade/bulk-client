#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <coin> <csv-file> [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Replaces one coin's complete risk matrix from a risk-surface CSV file.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 2 )); then
  echo "error: config-risk-matrix requires exactly one coin and one CSV file path" >&2
  usage
  exit 2
fi

bulk_admin_run config-risk-matrix "$1" "$2"
