#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

DEFAULT_MULTISIG="ADM1N11111111111111111111111111111111111113D"

usage() {
  cat >&2 <<EOF
Usage: $0 <proposal-id> [-msig <multisig-address>] [-ledger] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Approves a proposal. The multisig address defaults to:
  ${DEFAULT_MULTISIG}
EOF
}

bulk_admin_parse_common_args "$@"
set -- ${BULK_ADMIN_ARGS[@]+"${BULK_ADMIN_ARGS[@]}"}

multisig="${DEFAULT_MULTISIG}"
proposal_id=""

while (( $# > 0 )); do
  case "$1" in
    -msig|-multisig)
      if (( $# < 2 )) || [[ -z "$2" ]]; then
        echo "error: $1 requires a multisig address" >&2
        exit 2
      fi
      multisig="$2"
      shift 2
      ;;
    *)
      if [[ -n "${proposal_id}" ]]; then
        echo "error: multisig-approve accepts exactly one proposal ID" >&2
        usage
        exit 2
      fi
      proposal_id="$1"
      shift
      ;;
  esac
done

if [[ -z "${proposal_id}" ]]; then
  echo "error: multisig-approve requires a proposal ID" >&2
  usage
  exit 2
fi

if [[ ! "${multisig}" =~ ^[1-9A-HJ-NP-Za-km-z]{32,44}$ ]]; then
  echo "error: multisig address must be a base58-encoded Solana pubkey" >&2
  exit 2
fi

if [[ ! "${proposal_id}" =~ ^[0-9]+$ ]]; then
  echo "error: proposal ID must be a non-negative integer" >&2
  exit 2
fi

bulk_admin_run multisig-approve "${multisig}" "${proposal_id}"
