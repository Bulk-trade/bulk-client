#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 <symbol> <low|medium|high> --config-dir <dir> --executor-config <file>
          [--skip-currency] [--dry-run] [-ledger] [-url <api-url>]
          [-net <mainnet|testnet|devnet>]

Uses ref/{low,medium,high}.json5 models and the matching risk/ref/*.csv.
Submits all market configuration actions in one admin multisig proposal.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# < 2 )) || [[ "$1" == -* ]]; then
  echo "error: create-bip1 requires <symbol> <low|medium|high>" >&2
  usage
  exit 2
fi

bulk_admin_run create-bip1 "$@"
