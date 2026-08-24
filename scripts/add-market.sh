#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <symbol> [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Creates a market book after its configuration has been deployed.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 1 )); then
  echo "error: add-market requires exactly one market symbol" >&2
  usage
  exit 2
fi

bulk_admin_run add-market "$1"
