#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <symbol> <open|suspend|close> [--price <price>] [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Changes an existing market's administrative state. --price is valid only with close.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 2 && $# != 4 )); then
  echo "error: market-admin requires a symbol, action, and optional --price value" >&2
  usage
  exit 2
fi

case "$2" in
  open|suspend)
    if (( $# != 2 )); then
      echo "error: --price is valid only with close" >&2
      exit 2
    fi
    ;;
  close)
    if (( $# == 4 )) && [[ "$3" != "--price" ]]; then
      echo "error: expected --price before the close price" >&2
      exit 2
    fi
    ;;
  *)
    echo "error: action must be one of: open, suspend, close" >&2
    exit 2
    ;;
esac

bulk_admin_run market-admin "$@"
