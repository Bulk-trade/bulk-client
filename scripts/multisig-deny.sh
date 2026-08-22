#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

ADMIN_MULTISIG="ADM1N11111111111111111111111111111111111113D"

usage() {
  cat >&2 <<EOF
Usage: $0 <proposal-id> [-url <api-url>] [-net <mainnet|testnet|devnet>]

Rejects a proposal on the protocol administrative multisig:
  ${ADMIN_MULTISIG}
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 1 )); then
  echo "error: multisig-deny requires exactly one proposal ID" >&2
  usage
  exit 2
fi

if [[ ! "$1" =~ ^[0-9]+$ ]]; then
  echo "error: proposal ID must be a non-negative integer" >&2
  exit 2
fi

bulk_admin_run multisig-reject "${ADMIN_MULTISIG}" "$1"
