#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
TEST_ROOT=$(mktemp -d)

cleanup()
{
    if [ -n "${FIRST_PID:-}" ]; then
        kill -KILL "$FIRST_PID" >/dev/null 2>&1 || true
    fi
    rm -rf -- "$TEST_ROOT"
}
trap cleanup EXIT INT TERM

install -Dm755 "$SCRIPT_DIR/steam-deck/payload-run.sh" "$TEST_ROOT/run.sh"
mkdir -p "$TEST_ROOT/home"

cat >"$TEST_ROOT/mclone-native-client" <<'EOF'
#!/bin/sh
exec sleep "${MCLONE_TEST_HOLD_SECONDS:-30}"
EOF
chmod +x "$TEST_ROOT/mclone-native-client"

HOME="$TEST_ROOT/home" \
    MCLONE_TEST_HOLD_SECONDS=30 \
    "$TEST_ROOT/run.sh" play \
    >"$TEST_ROOT/first.stdout" \
    2>"$TEST_ROOT/first.stderr" &
FIRST_PID=$!

OWNER_FILE="$TEST_ROOT/home/.local/state/mclone-deck/instances/interactive.owner"
attempts=0
while [ ! -s "$OWNER_FILE" ] && [ "$attempts" -lt 50 ]; do
    sleep 0.1
    attempts=$((attempts + 1))
done
[ -s "$OWNER_FILE" ] || {
    echo "interactive owner record was not created" >&2
    exit 1
}

set +e
HOME="$TEST_ROOT/home" \
    "$TEST_ROOT/run.sh" play \
    >"$TEST_ROOT/duplicate.stdout" \
    2>"$TEST_ROOT/duplicate.stderr"
duplicate_status=$?
set -e
[ "$duplicate_status" -eq 75 ] || {
    echo "duplicate play exited $duplicate_status instead of 75" >&2
    exit 1
}

HOME="$TEST_ROOT/home" "$TEST_ROOT/run.sh" stop >"$TEST_ROOT/stop.stdout"
set +e
wait "$FIRST_PID"
set -e
FIRST_PID=
[ ! -e "$OWNER_FILE" ] || {
    echo "interactive owner record survived stop" >&2
    exit 1
}

mkdir -p "$(dirname -- "$OWNER_FILE")"
printf '%s\n' "999999 1" >"$OWNER_FILE"
HOME="$TEST_ROOT/home" "$TEST_ROOT/run.sh" stop >"$TEST_ROOT/stale.stdout"
[ ! -e "$OWNER_FILE" ] || {
    echo "stale interactive owner record was not removed" >&2
    exit 1
}

echo "Steam Deck payload process ownership passed"
