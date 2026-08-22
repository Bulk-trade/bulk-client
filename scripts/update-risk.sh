#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <json-or-file> [-url <api-url>] [-net <mainnet|testnet|devnet>]

Updates the exchange risk configuration from inline JSON or a JSON file.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 1 )); then
  echo "error: risk-config requires exactly one JSON value or file path" >&2
  usage
  exit 2
fi

bulk_admin_run risk-config "$1"
