#!/usr/bin/env bash
# Install the local pre-push hook that schedules native web deploys after main pushes.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
HOOK_DIR="$(git -C "$PROJECT_DIR" rev-parse --git-path hooks)"
case "$HOOK_DIR" in
  /*) ;;
  *) HOOK_DIR="$PROJECT_DIR/$HOOK_DIR" ;;
esac
HOOK_PATH="$HOOK_DIR/pre-push"
MARKER="mclone deploy-after-main-push hook"
FORCE=0

while (($#)); do
  case "$1" in
    --force)
      FORCE=1
      shift
      ;;
    -h|--help)
      echo "Usage: $0 [--force]"
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

mkdir -p "$HOOK_DIR"
if [ -f "$HOOK_PATH" ] && ! grep -q "$MARKER" "$HOOK_PATH" && [ "$FORCE" -ne 1 ]; then
  cat >&2 <<EOF
$HOOK_PATH already exists and was not installed by this script.
Re-run with --force to replace it.
EOF
  exit 1
fi

cat > "$HOOK_PATH" <<'EOF'
#!/usr/bin/env bash
# mclone deploy-after-main-push hook
set -euo pipefail

PROJECT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$PROJECT_DIR" ]; then
  exit 0
fi

HOOK="$PROJECT_DIR/scripts/local-deploy/pre-push-hook.sh"
if [ -x "$HOOK" ]; then
  exec "$HOOK" "$@"
fi

exit 0
EOF

chmod +x "$HOOK_PATH"
echo "Installed $HOOK_PATH"
