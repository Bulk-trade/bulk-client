#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <json-or-file> [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Updates one instrument's funding configuration through the admin multisig.

Example:
  $0 '{symbol:"BTC-USD",rate:0.125,deviationCap:0.0005,fundingCap:0.004,premiumHorizon:3600,notional:100000,samplePeriod:1,meanWindow:3600}' -net testnet
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 1 )); then
  echo "error: update-funding requires exactly one JSON value or file path" >&2
  usage
  exit 2
fi

bulk_admin_run update-funding "$1"
