#!/usr/bin/env bash
# Signing material for one macOS release job, from the release secrets.
#   mac-signing.sh setup [--with-certificate]   writes to $GITHUB_ENV
#   mac-signing.sh cleanup
# setup always writes the notarization key to a file and exports
# NOTARY_KEY_PATH, NOTARY_KEY_ID and NOTARY_ISSUER. With --with-certificate it
# also imports the Developer ID certificate into a temporary keychain and
# exports APPLE_SIGNING_IDENTITY. It never exports Tauri's APPLE_API_*
# variables: Tauri would then notarize and wait inside the build, and a
# multi-GB bundle can wait on Apple longer than a job may run.
# Needs: APPLE_API_KEY_P8, APPLE_API_KEY, APPLE_API_ISSUER, and with
# --with-certificate also APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD and
# APPLE_SIGNING_IDENTITY.
set -euo pipefail
kc="$RUNNER_TEMP/build.keychain-db"
key="$RUNNER_TEMP/notary-key.p8"
case "${1:-}" in
  setup)
    printf '%s' "$APPLE_API_KEY_P8" > "$key"
    {
      echo "NOTARY_KEY_PATH=$key"
      echo "NOTARY_KEY_ID=$APPLE_API_KEY"
      echo "NOTARY_ISSUER=$APPLE_API_ISSUER"
    } >> "$GITHUB_ENV"
    if [ "${2:-}" = --with-certificate ]; then
      pw="$(uuidgen)"
      security create-keychain -p "$pw" "$kc"
      security set-keychain-settings -lut 21600 "$kc"
      security unlock-keychain -p "$pw" "$kc"
      printf '%s' "$APPLE_CERTIFICATE" | base64 --decode > "$RUNNER_TEMP/cert.p12"
      security import "$RUNNER_TEMP/cert.p12" -k "$kc" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
      security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$pw" "$kc" >/dev/null
      security list-keychains -d user -s "$kc" login.keychain-db
      rm "$RUNNER_TEMP/cert.p12"
      echo "APPLE_SIGNING_IDENTITY=$APPLE_SIGNING_IDENTITY" >> "$GITHUB_ENV"
    fi
    ;;
  cleanup)
    security delete-keychain "$kc" 2>/dev/null || true
    rm -f "$key"
    ;;
  *) echo "usage: mac-signing.sh setup [--with-certificate] | cleanup" >&2; exit 2 ;;
esac
