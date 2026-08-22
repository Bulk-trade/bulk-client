#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <instrument> <both|pyth|bulk> [-url <api-url>] [-net <mainnet|testnet|devnet>]

Configures the accepted oracle publisher source for an instrument.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 2 )); then
  echo "error: pricing-admin requires an instrument and publisher source" >&2
  usage
  exit 2
fi

case "$2" in
  both|pyth|bulk) ;;
  *)
    echo "error: source must be one of: both, pyth, bulk" >&2
    exit 2
    ;;
esac

bulk_admin_run pricing-admin "$@"
