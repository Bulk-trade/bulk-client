#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

TEST_ACCOUNT="B7DJjX4WtcimU4oUvnabHrDNu1YGnBdU1UfK78KdA2T2"

usage() {
  cat >&2 <<EOF
Usage: $0 <k> [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Sets the global open-order limit to <k>. The staging max-order test account
${TEST_ACCOUNT}
is left without an account-specific override and therefore inherits the global limit.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 1 )); then
  echo "error: max-orders requires exactly one numeric limit" >&2
  usage
  exit 2
fi

if [[ ! "$1" =~ ^[0-9]+$ ]]; then
  echo "error: maxorders must be a non-negative integer" >&2
  exit 2
fi

bulk_admin_run user-admin \
  "${TEST_ACCOUNT}" \
  --use-global \
  --global-maxorders "$1"
