#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$repo_root" ]; then
  echo "install-git-hooks: not inside a git repository; skipping"
  exit 0
fi

git -C "$repo_root" config core.hooksPath .githooks
chmod +x "$repo_root/.githooks/commit-msg" "$repo_root/.githooks/pre-push"

echo "Git hooks installed: core.hooksPath=.githooks"
