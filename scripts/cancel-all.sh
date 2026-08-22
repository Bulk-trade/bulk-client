#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 [-url <api-url>] [-net <mainnet|testnet|devnet>]

Options:
  -url <api-url>  Override the exchange API base URL.
  -net <network>  Signature domain and API network (default: testnet).
  -h, --help      Show this help.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 0 )); then
  echo "error: unknown argument: $1" >&2
  usage
  exit 2
fi

# With no instrument filter, this cancels every open order for the signer.
bulk_admin_run cancel-all
