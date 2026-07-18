#!/usr/bin/env bash
# Download and decompile the pinned Minecraft Beta 1.7.3 research specimen.
# Generated source and jars remain under the gitignored reference/ tree.
#
# Usage:
#   ./scripts/decompile-beta-mc.sh [--out DIR] [--force]

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd -P)
exec "$SCRIPT_DIR/decompile-alpha-mc.sh" b1.7.3 "$@"
