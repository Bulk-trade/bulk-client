#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat >&2 <<EOF
Usage: $0 [-url <api-url>] [-net <mainnet|testnet|devnet>]

Options:
  -url <api-url>  Exchange API base URL.
  -net <network>  Signature network domain (default: testnet).
  -h, --help      Show this help.
EOF
}

api_url="${BULK_API_URL:-https://exchange-api.bulk.trade/api/v1}"
network="${BULK_SIGNATURE_DOMAIN:-testnet}"

while (( $# > 0 )); do
  case "$1" in
    -url)
      if (( $# < 2 )) || [[ -z "$2" ]]; then
        echo "error: -url requires a value" >&2
        usage
        exit 2
      fi
      api_url="$2"
      shift 2
      ;;
    -net)
      if (( $# < 2 )) || [[ -z "$2" ]]; then
        echo "error: -net requires a value" >&2
        usage
        exit 2
      fi
      network="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

case "${network}" in
  mainnet|testnet|devnet) ;;
  *)
    echo "error: -net must be one of: mainnet, testnet, devnet" >&2
    exit 2
    ;;
esac

# Keep the private key out of shell history and remove it from this process's
# environment when the script exits.
cleanup() {
  unset BULK_PRIVATE_KEY
}
trap cleanup EXIT HUP INT TERM

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo was not found in PATH" >&2
  exit 1
fi

if [[ -z "${BULK_PRIVATE_KEY:-}" ]]; then
  read -r -s -p "Bulk private key (base58): " BULK_PRIVATE_KEY
  echo >&2
  export BULK_PRIVATE_KEY
fi

if [[ -z "${BULK_PRIVATE_KEY}" ]]; then
  echo "error: private key cannot be empty" >&2
  exit 1
fi

export BULK_SIGNATURE_DOMAIN="${network}"
export BULK_API_URL="${api_url}"

echo "Signature domain: ${BULK_SIGNATURE_DOMAIN}" >&2
echo "API URL: ${BULK_API_URL}" >&2
echo "Verify that account=<expected pubkey> in the preview before confirming." >&2

# With no instrument filter, this cancels every open order for the account
# derived from BULK_PRIVATE_KEY. Preview and interactive confirmation remain on.
cargo run \
  --manifest-path "${PROJECT_ROOT}/Cargo.toml" \
  --package bulk-cli \
  --bin bulk \
  -- \
  --api-url "${BULK_API_URL}" \
  cancel-all
