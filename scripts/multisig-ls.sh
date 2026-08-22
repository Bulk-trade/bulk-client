#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/include/common.sh"

DEFAULT_MULTISIG="ADM1N11111111111111111111111111111111111113D"

usage() {
  cat >&2 <<EOF
Usage: $0 [proposal-id] [-msig <multisig-address>] [-url <api-url>] [-net <mainnet|testnet|devnet>]

Lists pending and ready proposals, or one optionally selected proposal.
The multisig address defaults to:
  ${DEFAULT_MULTISIG}

Examples:
  $0
  $0 23
  $0 -msig <multisig-address>
  $0 23 -msig <multisig-address>
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
    -*)
      echo "error: unknown argument: $1" >&2
      usage
      exit 2
      ;;
    *)
      if [[ -n "${proposal_id}" ]]; then
        echo "error: multisig-ls accepts at most one proposal ID" >&2
        usage
        exit 2
      fi
      proposal_id="$1"
      shift
      ;;
  esac
done

if [[ ! "${multisig}" =~ ^[1-9A-HJ-NP-Za-km-z]{32,44}$ ]]; then
  echo "error: multisig address must be a base58-encoded Solana pubkey" >&2
  exit 2
fi

if [[ -n "${proposal_id}" && ! "${proposal_id}" =~ ^[0-9]+$ ]]; then
  echo "error: proposal ID must be a non-negative integer" >&2
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

curl --fail --silent --show-error \
  --url "${BULK_ADMIN_API_URL}/multisig/${multisig}/proposals" \
| jq --arg proposal_id "${proposal_id}" '
    if $proposal_id == "" then
      (.proposals // [] | map(select(.status == "pending" or .status == "ready"))) as $selected
      | {
          multisig,
          availableCount: ($selected | length),
          proposals: $selected
        }
    else
      (.proposals // [] | map(select(.proposalId == ($proposal_id | tonumber)))) as $selected
      | {
          multisig,
          requestedProposalId: ($proposal_id | tonumber),
          matchCount: ($selected | length),
          proposals: $selected
        }
    end
  '
