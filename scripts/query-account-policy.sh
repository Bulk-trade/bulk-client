#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

usage() {
  cat >&2 <<EOF
Usage: $0 [-url <api-url>] [-net <mainnet|testnet|devnet>]

Queries the current account funding policy.
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

if (( $# != 0 )); then
  echo "error: query-account-policy does not accept positional arguments" >&2
  usage
  exit 2
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl was not found in PATH" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq was not found in PATH" >&2
  exit 1
fi

echo "API URL: ${BULK_ADMIN_API_URL}" >&2

curl --fail --silent --show-error --get \
  --url "${BULK_ADMIN_API_URL}/config" \
  --data-urlencode "which=account-policy" \
| jq '{withdrawFeeUsd, minWithdrawUsd, minExternalTransferUsd}'
