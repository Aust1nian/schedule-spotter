#!/usr/bin/env bash
# Apple notarization, split so no single job waits on Apple for long
# (DECISIONS.md, Phase 6). Uses NOTARY_KEY_PATH, NOTARY_KEY_ID and
# NOTARY_ISSUER (mac-signing.sh setup).
#   notary.sh submit <file>     uploads; prints the submission id
#   notary.sh wait <id>         waits up to 5 h 30 min; fails unless Accepted
#                               (and prints Apple's log when it isn't)
#   notary.sh staple <path>     staples the ticket, retrying: Apple's ticket
#                               server sometimes drops the connection
set -euo pipefail
auth=(--key "$NOTARY_KEY_PATH" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER")
case "${1:-}" in
  submit)
    start=$(date +%s)
    id="$(xcrun notarytool submit "$2" "${auth[@]}" --output-format json | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
    echo "notary: uploaded $(basename "$2") in $(( $(date +%s) - start )) s; submission $id" >&2
    echo "$id"
    ;;
  wait)
    start=$(date +%s)
    xcrun notarytool wait "$2" "${auth[@]}" --timeout 5h30m >&2 || true
    status="$(xcrun notarytool info "$2" "${auth[@]}" --output-format json | python3 -c 'import json,sys; print(json.load(sys.stdin)["status"])')"
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
