#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <json-or-file> [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Updates the global or per-market trading fee policy from inline JSON/JSON5 or a file.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 1 )); then
  echo "error: config-fees requires exactly one JSON value or file path" >&2
  usage
  exit 2
fi

bulk_admin_run config-fees "$1"
