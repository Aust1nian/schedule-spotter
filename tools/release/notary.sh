#!/usr/bin/env bash
# Apple notarization, split so no single job waits on Apple for long
# (DECISIONS.md, Phase 6). Uses NOTARY_KEY_PATH, NOTARY_KEY_ID and
# NOTARY_ISSUER (mac-signing.sh setup).
#   notary.sh submit <file>     uploads; prints the submission id
#   notary.sh wait <id>         checks every minute for up to 5 h 30 min,
#                               riding out network blips; fails unless Accepted
#                               (and prints Apple's log when it isn't)
#   notary.sh staple <path>     staples the ticket, retrying: Apple's ticket
#                               server sometimes drops the connection
set -euo pipefail
auth=(--key "$NOTARY_KEY_PATH" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER")
case "${1:-}" in
  submit)
    for attempt in 1 2 3; do
      start=$(date +%s)
      if id="$(xcrun notarytool submit "$2" "${auth[@]}" --output-format json \
          | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"; then
        echo "notary: uploaded $(basename "$2") in $(( $(date +%s) - start )) s; submission $id" >&2
        echo "$id"
        exit 0
      fi
      echo "notary: upload failed (attempt $attempt); retrying in 60 s" >&2
      sleep 60
    done
    exit 1
    ;;
  wait)
    # Polls once a minute rather than one long `notarytool wait`: a runner can
    # lose its network for a moment during hours of waiting, and one failed
    # check must not end the job (release run 38658533940, B-E2B).
    start=$(date +%s)
    deadline=$(( start + 330 * 60 ))
    status=""
    while [ "$(date +%s)" -lt "$deadline" ]; do
      status="$(xcrun notarytool info "$2" "${auth[@]}" --output-format json 2>/dev/null \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["status"])' 2>/dev/null || echo "unreachable")"
      case "$status" in
        Accepted|Invalid|Rejected) break ;;
      esac
      sleep 60
    done
    echo "notary: submission $2 is $status after $(( $(date +%s) - start )) s of waiting" >&2
    if [ "$status" != Accepted ]; then
      xcrun notarytool log "$2" "${auth[@]}" >&2 || true
      exit 1
    fi
    ;;
  staple)
    for attempt in 1 2 3 4 5 6; do
      xcrun stapler staple "$2" && exit 0
      echo "notary: stapling failed (attempt $attempt); retrying in 60 s" >&2
      sleep 60
    done
    exit 1
    ;;
  *) echo "usage: notary.sh submit <file> | wait <id> | staple <path>" >&2; exit 2 ;;
esac
