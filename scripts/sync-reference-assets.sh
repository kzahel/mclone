#!/usr/bin/env bash
# Compatibility wrapper for the former targeted R2 reference-asset upload.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "Reference assets now deploy as part of the aggregate Workers Static Assets manifest."
exec "$PROJECT_DIR/scripts/deploy-native-web.sh" "$@"
