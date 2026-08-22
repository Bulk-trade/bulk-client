#!/usr/bin/env bash

# Shared setup for the command wrappers in this directory.
# This file is intended to be sourced, not executed directly.

bulk_admin_parse_common_args() {
  BULK_ADMIN_API_URL="${BULK_API_URL:-}"
  BULK_ADMIN_NETWORK="${BULK_SIGNATURE_DOMAIN:-devnet}"
  BULK_ADMIN_ARGS=()
  local network_was_set=false
  local url_was_set=false

  while (( $# > 0 )); do
    case "$1" in
      -url)
        if (( $# < 2 )) || [[ -z "$2" ]]; then
          echo "error: -url requires a value" >&2
          return 2
        fi
        BULK_ADMIN_API_URL="$2"
        url_was_set=true
        shift 2
        ;;
      -net)
        if (( $# < 2 )) || [[ -z "$2" ]]; then
          echo "error: -net requires a value" >&2
          return 2
        fi
        BULK_ADMIN_NETWORK="$2"
        network_was_set=true
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        BULK_ADMIN_ARGS+=("$1")
        shift
        ;;
    esac
  done

  case "${BULK_ADMIN_NETWORK}" in
    mainnet|testnet|devnet) ;;
    *)
      echo "error: -net must be one of: mainnet, testnet, devnet" >&2
      return 2
      ;;
  esac

  # An explicit network selects its canonical URL unless -url was also supplied.
  if [[ "${network_was_set}" == true && "${url_was_set}" == false ]]; then
    BULK_ADMIN_API_URL=""
  fi

  if [[ -z "${BULK_ADMIN_API_URL}" ]]; then
    case "${BULK_ADMIN_NETWORK}" in
      testnet)
        BULK_ADMIN_API_URL="https://exchange-api.bulk.trade/api/v1"
        ;;
      devnet)
        BULK_ADMIN_API_URL="http://localhost:12000/api/v1"
        ;;
      mainnet)
        echo "error: mainnet requires -url or BULK_API_URL" >&2
        return 2
        ;;
    esac
  fi

  BULK_ADMIN_API_URL="${BULK_ADMIN_API_URL%/}"
}

bulk_admin_run() {
  local command="$1"
  shift

  if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo was not found in PATH" >&2
    return 1
  fi

  if [[ -z "${BULK_PRIVATE_KEY:-}" ]]; then
    read -r -s -p "Bulk private key (base58): " BULK_PRIVATE_KEY
    echo >&2
    export BULK_PRIVATE_KEY
  fi

  if [[ -z "${BULK_PRIVATE_KEY}" ]]; then
    echo "error: private key cannot be empty" >&2
    return 1
  fi

  export BULK_SIGNATURE_DOMAIN="${BULK_ADMIN_NETWORK}"
  export BULK_API_URL="${BULK_ADMIN_API_URL}"

  echo "Signature domain: ${BULK_SIGNATURE_DOMAIN}" >&2
  echo "API URL: ${BULK_API_URL}" >&2
  echo "Verify the signer, account, nonce, and action in the preview before confirming." >&2

  cargo run \
    --manifest-path "${PROJECT_ROOT}/Cargo.toml" \
    --package bulk-cli \
    --bin bulk \
    -- \
    --api-url "${BULK_API_URL}" \
    "${command}" \
    "$@"
}

bulk_admin_cleanup() {
  unset BULK_PRIVATE_KEY
}

trap bulk_admin_cleanup EXIT HUP INT TERM
