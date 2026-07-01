#!/usr/bin/env bash
# Git pre-push entrypoint for scheduling a background deploy after main pushes.
set -euo pipefail

REMOTE="${1:-origin}"
TARGET_BRANCH="${MCLONE_DEPLOY_AFTER_PUSH_BRANCH:-main}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SCHEDULER="$SCRIPT_DIR/deploy-after-main-push.sh"
ZERO_OID="0000000000000000000000000000000000000000"

while read -r local_ref local_oid remote_ref remote_oid; do
  if [ "$remote_ref" != "refs/heads/$TARGET_BRANCH" ]; then
    continue
  fi

  if [ "$local_oid" = "$ZERO_OID" ]; then
    continue
  fi

  "$SCHEDULER" --schedule --remote "$REMOTE" --branch "$TARGET_BRANCH" --sha "$local_oid" || true
done
