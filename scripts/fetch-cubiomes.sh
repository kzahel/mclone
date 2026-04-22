#!/usr/bin/env bash
# Clone Cubitect/cubiomes into the reference dir.
#
# Usage: ./fetch-cubiomes.sh
# Env:   REF_DIR (default: ~/code/reference)

set -euo pipefail

REF_DIR="${REF_DIR:-$HOME/code/reference}"
DEST="$REF_DIR/cubiomes"

if [ -d "$DEST/.git" ]; then
    echo "cubiomes already present at $DEST (not modifying)."
    exit 0
fi

mkdir -p "$REF_DIR"
git clone --depth 1 https://github.com/Cubitect/cubiomes.git "$DEST"
echo ""
echo "Done: $DEST"
