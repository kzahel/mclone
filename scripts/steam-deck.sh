#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
STAGE_DIR="$REPO_ROOT/dist/steamdeck"
PAYLOAD_RUN="$REPO_ROOT/scripts/steam-deck/payload-run.sh"
ASSET_PACK="$REPO_ROOT/reference/minecraft-1.17.1/extracted.zip"
STEAMRT4_BUILD_SCRIPT="$REPO_ROOT/scripts/steam-deck-build-steamrt4.sh"
SCREEN_WAKE_HELPER="$REPO_ROOT/scripts/steam-deck/screen-wake.py"

DECK_BUILDER=${MCLONE_STEAM_DECK_BUILDER:-host}
case "$DECK_BUILDER" in
    host)
        CLIENT_BINARY=${MCLONE_STEAM_DECK_CLIENT_BINARY:-"$REPO_ROOT/native/target/release/mclone-native-client"}
        BUILD_RUNTIME=host-ubuntu
        BUILDER_RECEIPT=
        ;;
    steamrt4)
        CLIENT_BINARY=${MCLONE_STEAM_DECK_CLIENT_BINARY:-"$REPO_ROOT/native/target/steamrt4/release/mclone-native-client"}
        BUILD_RUNTIME=steamrt4-sdk-4.0.20260608.242786
        BUILDER_RECEIPT="$REPO_ROOT/native/target/steamrt4/build-receipt.json"
        ;;
    *)
        echo "steam-deck: unknown builder: $DECK_BUILDER" >&2
        exit 1
        ;;
esac

CONFIGURED_DECK_HOST=$(git -C "$REPO_ROOT" config --local --get \
    mclone.steamDeckHost 2>/dev/null || true)
DECK_HOST=${MCLONE_STEAM_DECK:-${CONFIGURED_DECK_HOST:-steamdeck.local}}
DECK_USER=${MCLONE_STEAM_DECK_USER:-deck}
DECK_KEY=${MCLONE_STEAM_DECK_KEY:-"$HOME/.config/steamos-devkit/devkit_rsa"}
DECK_TITLE=${MCLONE_STEAM_DECK_TITLE:-mclone}
DECK_RUNTIME=${MCLONE_STEAM_DECK_RUNTIME:-SteamLinuxRuntime_4}
REMOTE_RESULT_ROOT=${MCLONE_STEAM_DECK_REMOTE_RESULTS:-"/home/$DECK_USER/.local/state/mclone-deck/results"}
LOCAL_RESULT_ROOT=${MCLONE_STEAM_DECK_RESULTS_ROOT:-/tmp/mclone-steam-deck-results}

SSH_OPTIONS=(
    -o BatchMode=yes
    -o ConnectTimeout=10
    -o StrictHostKeyChecking=yes
    -i "$DECK_KEY"
)

usage()
{
    cat <<'EOF'
Usage: scripts/steam-deck.sh COMMAND

Commands:
  status        Query the paired Deck and SteamOS session.
  power-status  Report AC, idle-suspend, and internal-screen state.
  screen-off    Sleep the internal panel through the Gaming Mode compositor.
  screen-on     Wake the internal panel through the Gaming Mode compositor.
  stage         Build and assemble dist/steamdeck.
  upload        Stage, upload, and register Devkit Game: mclone.
  deploy        Stage, upload, register, and launch interactive play.
  launch        Launch the already-uploaded interactive title.
  stop          Stop the owned interactive Devkit title, if running.
  smoke         Stage/upload, run the bounded screenshot + present smoke,
                pull its results, and restore the interactive shortcut.
  perf          Stage/upload, run the timedemo + 3600-frame present bench,
                pull its results, and restore the interactive shortcut.
  gamescope-repro
                Stage/upload and run the bounded elevated RD13 window used
                for controlled Gamescope screenshot reproduction.
  pull-results  Pull the newest Deck result, or the optional RUN_ID argument.

Environment:
  MCLONE_STEAM_DECK=HOST
      Deck hostname or address. Defaults to steamdeck.local.
  MCLONE_STEAM_DECK_SKIP_BUILD=1
      Reuse the selected builder's existing binary while staging.
  MCLONE_STEAM_DECK_BUILDER=host|steamrt4
      Build on Ubuntu or in the pinned SteamRT4 SDK. Defaults to host.
  MCLONE_STEAM_DECK_SKIP_ASSET_CHECK=1
      Skip pnpm assets:pack:check while staging.
  MCLONE_STEAM_DECK_ENSURE_ASSET_PACK=1
      If the asset lock check fails, rebuild the archive once and recheck.
      The tracked asset lock is never rewritten automatically.
  MCLONE_STEAM_DECK_RUNTIME=SteamLinuxRuntime_4|none
      Devkit compatibility runtime. Defaults to Steam Linux Runtime 4.
  MCLONE_STEAM_DECK_RESULTS_ROOT=PATH
      Local pulled-result root. Defaults to /tmp/mclone-steam-deck-results.
EOF
}

die()
{
    echo "steam-deck: $*" >&2
    exit 1
}

require_command()
{
    command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

ssh_deck()
{
    ssh "${SSH_OPTIONS[@]}" "$DECK_USER@$DECK_HOST" "$@"
}

require_deck()
{
    require_command ssh
    require_command rsync
    require_command jq
    test -f "$DECK_KEY" || die "Devkit key not found: $DECK_KEY"
    ssh_deck "/usr/bin/true" >/dev/null
}

deck_power_status()
{
    require_deck
    ssh_deck "/usr/bin/bash -s" <<'REMOTE'
set -euo pipefail

ac_online=unknown
for supply in /sys/class/power_supply/*; do
    [[ -e $supply ]] || continue
    [[ $(cat "$supply/type" 2>/dev/null || true) == Mains ]] || continue
    ac_online=$(cat "$supply/online" 2>/dev/null || echo unknown)
    break
done

config="$HOME/.local/share/Steam/config/config.vdf"
ac_suspend=$(sed -n \
    's/.*"IdleSuspendACSeconds"[[:space:]]*"\([0-9]*\)".*/\1/p' \
    "$config" 2>/dev/null | tail -1)
battery_suspend=$(sed -n \
    's/.*"IdleSuspendBatterySeconds"[[:space:]]*"\([0-9]*\)".*/\1/p' \
    "$config" 2>/dev/null | tail -1)

connector=unavailable
connector_state=unknown
for path in /sys/class/drm/card*-eDP-*; do
    [[ -e $path ]] || continue
    connector=${path##*/}
    connector_state=$(cat "$path/enabled" 2>/dev/null || echo unknown)
    break
done

if pgrep -f '(^|/)gamescope( |$)' >/dev/null 2>&1; then
    gamescope_state=running
else
    gamescope_state=stopped
fi

printf 'AC online: %s\n' "$ac_online"
printf 'AC idle suspend: %s seconds%s\n' \
    "${ac_suspend:-unknown}" \
    "$([[ ${ac_suspend:-} == 0 ]] && printf ' (disabled)' || true)"
printf 'Battery idle suspend: %s seconds%s\n' \
    "${battery_suspend:-unknown}" \
    "$([[ ${battery_suspend:-} == 0 ]] && printf ' (disabled)' || true)"
printf 'Gamescope: %s\n' "$gamescope_state"
printf 'Internal connector: %s (%s)\n' "$connector" "$connector_state"
printf 'SSH: reachable\n'
REMOTE
}

set_deck_internal_screen_sleep()
{
    local requested=$1
    local expected=$2
    local label=$3
    local remote_helper=none
    require_deck
    if [[ $requested == true ]]; then
        test -f "$SCREEN_WAKE_HELPER" ||
            die "screen wake helper not found: $SCREEN_WAKE_HELPER"
        local remote_helper_dir="/home/$DECK_USER/.local/state/mclone-deck/screen-wake"
        remote_helper="$remote_helper_dir/screen-wake.py"
        ssh_deck \
            "/usr/bin/install -d -m 700 $(printf '%q' "$remote_helper_dir")"
        rsync \
            -a \
            --chmod=Fu=rwx,Fgo= \
            -e "ssh ${SSH_OPTIONS[*]}" \
            "$SCREEN_WAKE_HELPER" \
            "$DECK_USER@$DECK_HOST:$remote_helper"
    fi
    ssh_deck \
        "/usr/bin/bash -s -- $(printf '%q' "$requested") $(printf '%q' "$expected") $(printf '%q' "$label") $(printf '%q' "$remote_helper")" \
        <<'REMOTE'
set -euo pipefail
requested=$1
expected=$2
label=$3
remote_helper=$4
runtime_dir="/run/user/$(id -u)"
watcher_unit=mclone-deck-screen-wake.service
watcher_state_dir="$HOME/.local/state/mclone-deck/screen-wake"
watcher_ready="$watcher_state_dir/ready"

gamescope_display=
for path in "$runtime_dir"/gamescope-[0-9]*; do
    [[ -S $path ]] || continue
    name=${path##*/}
    [[ $name =~ ^gamescope-[0-9]+$ ]] || continue
    gamescope_display=$name
    break
done
[[ -n $gamescope_display ]] || {
    echo "steam-deck: active Gaming Mode Gamescope socket not found" >&2
    exit 1
}
command -v gamescopectl >/dev/null 2>&1 || {
    echo "steam-deck: gamescopectl is unavailable" >&2
    exit 1
}

stop_watcher()
{
    systemctl --user stop "$watcher_unit" >/dev/null 2>&1 || true
    systemctl --user reset-failed "$watcher_unit" >/dev/null 2>&1 || true
    rm -f -- "$watcher_ready"
}

if [[ $requested == true ]]; then
    [[ -x $remote_helper ]] || {
        echo "steam-deck: deployed screen wake helper is unavailable" >&2
        exit 1
    }
    stop_watcher
    install -d -m 700 "$watcher_state_dir"
    systemd-run \
        --user \
        --unit="$watcher_unit" \
        --collect \
        --quiet \
        /usr/bin/env \
        "XDG_RUNTIME_DIR=$runtime_dir" \
        "GAMESCOPE_WAYLAND_DISPLAY=$gamescope_display" \
        /usr/bin/python3 \
        "$remote_helper" \
        "$watcher_ready"
    for _attempt in {1..50}; do
        [[ -f $watcher_ready ]] && break
        systemctl --user is-active --quiet "$watcher_unit" || {
            journalctl --user -u "$watcher_unit" -n 20 --no-pager >&2 || true
            echo "steam-deck: local-input screen wake watcher failed to start" >&2
            exit 1
        }
        sleep 0.1
    done
    [[ -f $watcher_ready ]] || {
        stop_watcher
        echo "steam-deck: local-input screen wake watcher did not arm" >&2
        exit 1
    }
else
    stop_watcher
fi

cleanup_failed_sleep()
{
    if [[ $requested == true ]]; then
        XDG_RUNTIME_DIR=$runtime_dir \
        GAMESCOPE_WAYLAND_DISPLAY=$gamescope_display \
            gamescopectl drm_sleep_internal_screen false >/dev/null 2>&1 || true
        stop_watcher
    fi
}
trap cleanup_failed_sleep ERR

XDG_RUNTIME_DIR=$runtime_dir \
GAMESCOPE_WAYLAND_DISPLAY=$gamescope_display \
    gamescopectl drm_sleep_internal_screen "$requested"

connector=
for path in /sys/class/drm/card*-eDP-*; do
    [[ -e $path ]] || continue
    connector=$path
    break
done
[[ -n $connector ]] || {
    echo "steam-deck: internal eDP connector not found" >&2
    exit 1
}

state=unknown
for _attempt in {1..20}; do
    state=$(cat "$connector/enabled" 2>/dev/null || echo unknown)
    [[ $state == "$expected" ]] && break
    sleep 0.1
done
[[ $state == "$expected" ]] || {
    echo \
        "steam-deck: internal connector did not become $expected (state=$state)" \
        >&2
    exit 1
}
trap - ERR
printf \
    'Deck internal screen: %s (%s=%s); SSH remains reachable%s\n' \
    "$label" "${connector##*/}" "$state" \
    "$([[ $requested == true ]] && printf '; local Deck-button wake armed' || true)"
REMOTE
}

stage_payload()
{
    require_command cargo
    require_command jq
    require_command rsync
    require_command sha256sum

    local asset_check_skipped=false
    if [[ ${MCLONE_STEAM_DECK_SKIP_ASSET_CHECK:-0} != 1 ]]; then
        if ! (
            cd "$REPO_ROOT"
            pnpm assets:pack:check
        ); then
            if [[ ${MCLONE_STEAM_DECK_ENSURE_ASSET_PACK:-0} != 1 ]]; then
                return 1
            fi
            echo \
                "Steam Deck asset pack is stale; rebuilding the archive once"
            (
                cd "$REPO_ROOT"
                pnpm assets:pack
                pnpm assets:pack:check
            )
        fi
    else
        asset_check_skipped=true
    fi
    test -f "$ASSET_PACK" || die "asset pack not found: $ASSET_PACK"

    local build_skipped=false
    if [[ ${MCLONE_STEAM_DECK_SKIP_BUILD:-0} != 1 ]]; then
        case "$DECK_BUILDER" in
            host)
                cargo build \
                    --release \
                    --locked \
                    --manifest-path "$REPO_ROOT/native/Cargo.toml" \
                    -p mclone-native-client \
                    --bin mclone-native-client
                ;;
            steamrt4)
                "$STEAMRT4_BUILD_SCRIPT"
                ;;
        esac
    else
        build_skipped=true
    fi
    test -x "$CLIENT_BINARY" || die "release binary not found: $CLIENT_BINARY"

    local temp_stage
    temp_stage=$(mktemp -d)
    trap 'rm -rf -- "$temp_stage"' RETURN
    install -Dm755 "$CLIENT_BINARY" "$temp_stage/mclone-native-client"
    install -Dm755 "$PAYLOAD_RUN" "$temp_stage/run.sh"
    install -Dm644 "$ASSET_PACK" "$temp_stage/assets/extracted.zip"

    local commit dirty binary_sha asset_sha builder_receipt
    commit=$(git -C "$REPO_ROOT" rev-parse HEAD)
    if [[ -z $(git -C "$REPO_ROOT" status --porcelain --untracked-files=normal) ]]; then
        dirty=false
    else
        dirty=true
    fi
    binary_sha=$(sha256sum "$temp_stage/mclone-native-client" | cut -d' ' -f1)
    asset_sha=$(sha256sum "$temp_stage/assets/extracted.zip" | cut -d' ' -f1)
    builder_receipt=null
    if [[ -n $BUILDER_RECEIPT ]]; then
        test -f "$BUILDER_RECEIPT" ||
            die "builder receipt not found: $BUILDER_RECEIPT"
        builder_receipt=$(jq -c . "$BUILDER_RECEIPT")
    fi
    jq -n \
        --arg schema "mclone-steam-deck-stage-v1" \
        --arg commit "$commit" \
        --argjson dirty "$dirty" \
        --argjson buildSkipped "$build_skipped" \
        --argjson assetCheckSkipped "$asset_check_skipped" \
        --arg target "x86_64-unknown-linux-gnu" \
        --arg buildRuntime "$BUILD_RUNTIME" \
        --argjson builderReceipt "$builder_receipt" \
        --arg binarySha256 "$binary_sha" \
        --arg assetSha256 "$asset_sha" \
        '{
            schema: $schema,
            commit: $commit,
            dirty: $dirty,
            buildSkipped: $buildSkipped,
            assetCheckSkipped: $assetCheckSkipped,
            target: $target,
            buildRuntime: $buildRuntime,
            builderReceipt: $builderReceipt,
            binarySha256: $binarySha256,
            assetSha256: $assetSha256
        }' >"$temp_stage/build.json"

    mkdir -p "$STAGE_DIR"
    rsync -a --delete "$temp_stage/" "$STAGE_DIR/"
    trap - RETURN
    rm -rf -- "$temp_stage"

    echo "Staged Steam Deck payload: $STAGE_DIR"
    du -sh "$STAGE_DIR"
}

prepare_remote_directory()
{
    local output remote_user remote_dir
    output=$(ssh_deck \
        "python3 ~/devkit-utils/steamos-prepare-upload --gameid $(printf '%q' "$DECK_TITLE")")
    remote_user=$(jq -r '.user // empty' <<<"$output")
    remote_dir=$(jq -r '.directory // empty' <<<"$output")

    [[ $remote_user == "$DECK_USER" ]] ||
        die "unexpected upload user from Deck: ${remote_user:-missing}"
    case "$remote_dir" in
        "/home/$DECK_USER/"*"/$DECK_TITLE"|"/home/$DECK_USER/"*"/$DECK_TITLE/")
            ;;
        *)
            die "refusing unexpected upload directory: ${remote_dir:-missing}"
            ;;
    esac
    printf '%s\n' "$remote_dir"
}

register_title()
{
    local remote_dir=$1
    local mode=$2
    local run_id=${3:-}
    local parms response quoted

    if [[ -n $run_id ]]; then
        parms=$(jq -cn \
            --arg gameid "$DECK_TITLE" \
            --arg directory "$remote_dir" \
            --arg mode "$mode" \
            --arg resultRoot "$REMOTE_RESULT_ROOT" \
            --arg runId "$run_id" \
            --arg runtime "$DECK_RUNTIME" \
            '{
                gameid: $gameid,
                directory: $directory,
                argv: ["./run.sh", $mode, $runId, $resultRoot],
                env: {
                    MCLONE_DECK_RESULT_ROOT: $resultRoot,
                    MCLONE_DECK_RESULT_ID: $runId
                },
                settings: (
                    {steam_play: "0"} +
                    if $runtime == "none" then {} else {compat_tool: $runtime} end
                ),
                force_appid: ""
            }')
    else
        parms=$(jq -cn \
            --arg gameid "$DECK_TITLE" \
            --arg directory "$remote_dir" \
            --arg mode "$mode" \
            --arg resultRoot "$REMOTE_RESULT_ROOT" \
            --arg runtime "$DECK_RUNTIME" \
            '{
                gameid: $gameid,
                directory: $directory,
                argv: ["./run.sh", $mode],
                env: {MCLONE_DECK_RESULT_ROOT: $resultRoot},
                settings: (
                    {steam_play: "0"} +
                    if $runtime == "none" then {} else {compat_tool: $runtime} end
                ),
                force_appid: ""
            }')
    fi

    printf -v quoted '%q' "$parms"
    response=$(ssh_deck \
        "python3 ~/devkit-utils/steam-client-create-shortcut --parms $quoted")
    if jq -e '.error' >/dev/null 2>&1 <<<"$response"; then
        die "Deck shortcut registration failed: $(jq -r '.error' <<<"$response")"
    fi
    jq -e '.success' >/dev/null 2>&1 <<<"$response" ||
        die "Deck shortcut registration returned an unexpected response: $response"
}

upload_payload()
{
    require_deck
    test -x "$STAGE_DIR/run.sh" || die "staged payload is missing; run stage first"

    local remote_dir
    remote_dir=$(prepare_remote_directory)
    echo "Uploading payload to $DECK_HOST:$remote_dir"
    rsync \
        -av \
        --delete \
        --chmod=Du=rwx,Dgo=rx,Fu=rwx,Fog=rx \
        -e "ssh ${SSH_OPTIONS[*]}" \
        "$STAGE_DIR/" \
        "$DECK_USER@$DECK_HOST:$remote_dir/"
    register_title "$remote_dir" play
    printf '%s\n' "$remote_dir"
}

launch_title()
{
    require_deck
    stop_title
    ssh_deck \
        "python3 ~/devkit-utils/steam-devkit-rpc run-game gameid=$(printf '%q' "$DECK_TITLE")"
}

stop_title()
{
    local remote_dir
    remote_dir=$(prepare_remote_directory)
    ssh_deck "$(printf '%q' "$remote_dir/run.sh") stop"
}

new_run_id()
{
    local mode=$1
    printf '%s-%s-%s-%s\n' \
        "$(date -u +%Y%m%dT%H%M%SZ)" \
        "$(git -C "$REPO_ROOT" rev-parse --short=12 HEAD)" \
        "$mode" \
        "$$"
}

wait_for_result()
{
    local run_id=$1
    local timeout_seconds=$2
    local status_path="$REMOTE_RESULT_ROOT/$run_id/status"
    local deadline=$((SECONDS + timeout_seconds))
    local status

    echo "Waiting for Deck result $run_id"
    while (( SECONDS < deadline )); do
        status=$(ssh_deck \
            "if /usr/bin/test -f $(printf '%q' "$status_path"); then /usr/bin/cat $(printf '%q' "$status_path"); fi")
        if [[ -n $status ]]; then
            if [[ ! $status =~ ^[0-9]+$ ]]; then
                echo "steam-deck: invalid result status for $run_id: $status" >&2
                return 125
            fi
            if [[ $status != 0 ]]; then
                return "$status"
            fi
            return 0
        fi
        sleep 2
    done
    echo \
        "steam-deck: timed out after ${timeout_seconds}s waiting for Deck result $run_id" \
        >&2
    return 124
}

pull_result()
{
    require_deck
    local run_id=${1:-}
    if [[ -z $run_id ]]; then
        run_id=$(ssh_deck \
            "/usr/bin/find $(printf '%q' "$REMOTE_RESULT_ROOT") -mindepth 1 -maxdepth 1 -type d -printf '%T@ %f\\n' 2>/dev/null | /usr/bin/sort -nr | /usr/bin/head -1 | /usr/bin/cut -d' ' -f2-")
    fi
    [[ $run_id =~ ^[A-Za-z0-9._-]+$ ]] ||
        die "invalid or missing Deck result id: ${run_id:-missing}"

    local destination="$LOCAL_RESULT_ROOT/$run_id"
    mkdir -p "$destination"
    rsync \
        -av \
        -e "ssh ${SSH_OPTIONS[*]}" \
        "$DECK_USER@$DECK_HOST:$REMOTE_RESULT_ROOT/$run_id/" \
        "$destination/"
    echo "$destination"
}

cleanup_bounded_run()
{
    local remote_dir=$1
    register_title "$remote_dir" play >/dev/null 2>&1 || true
    set_deck_internal_screen_sleep true disabled off >/dev/null 2>&1 || true
}

run_bounded()
{
    local mode=$1
    local timeout_seconds=$2
    local remote_dir run_id status=0 destination
    local cleanup_command

    remote_dir=$(prepare_remote_directory)
    printf -v cleanup_command 'cleanup_bounded_run %q' \
        "$remote_dir"
    trap "$cleanup_command" EXIT
    set_deck_internal_screen_sleep false enabled on
    run_id=$(new_run_id "$mode")
    register_title "$remote_dir" "$mode" "$run_id"
    echo "Launching $mode as Devkit Game: $DECK_TITLE"
    launch_title

    set +e
    wait_for_result "$run_id" "$timeout_seconds"
    status=$?
    set -e
    register_title "$remote_dir" play
    destination=$(pull_result "$run_id")
    echo "Pulled Deck result: $destination"
    set_deck_internal_screen_sleep true disabled off
    trap - EXIT
    return "$status"
}

run_bounded_workflow()
{
    local mode=$1
    local timeout_seconds=$2
    trap \
        'set_deck_internal_screen_sleep true disabled off >/dev/null 2>&1 || true' \
        EXIT
    stage_payload
    upload_payload >/dev/null
    run_bounded "$mode" "$timeout_seconds"
}

command=${1:-help}
shift || true
if [[ ${1:-} == -- ]]; then
    shift
fi

case "$command" in
    help|-h|--help)
        usage
        ;;
    status)
        require_deck
        ssh_deck "python3 ~/devkit-utils/steamos-get-status --json"
        ;;
    power-status)
        [[ $# == 0 ]] || die "power-status takes no arguments"
        deck_power_status
        ;;
    screen-off)
        [[ $# == 0 ]] || die "screen-off takes no arguments"
        set_deck_internal_screen_sleep true disabled off
        ;;
    screen-on)
        [[ $# == 0 ]] || die "screen-on takes no arguments"
        set_deck_internal_screen_sleep false enabled on
        ;;
    stage)
        [[ $# == 0 ]] || die "stage takes no arguments"
        stage_payload
        ;;
    upload)
        [[ $# == 0 ]] || die "upload takes no arguments"
        stage_payload
        upload_payload >/dev/null
        echo "Uploaded and registered Devkit Game: $DECK_TITLE"
        ;;
    deploy)
        [[ $# == 0 ]] || die "deploy takes no arguments"
        stage_payload
        upload_payload >/dev/null
        launch_title
        ;;
    launch)
        [[ $# == 0 ]] || die "launch takes no arguments"
        launch_title
        ;;
    stop)
        [[ $# == 0 ]] || die "stop takes no arguments"
        require_deck
        stop_title
        ;;
    smoke)
        [[ $# == 0 ]] || die "smoke takes no arguments"
        run_bounded_workflow smoke 600
        ;;
    perf)
        [[ $# == 0 ]] || die "perf takes no arguments"
        run_bounded_workflow perf 1200
        ;;
    gamescope-repro)
        [[ $# == 0 ]] || die "gamescope-repro takes no arguments"
        run_bounded_workflow gamescope-repro 1200
        ;;
    pull-results)
        [[ $# -le 1 ]] || die "pull-results accepts at most one RUN_ID"
        pull_result "${1:-}"
        ;;
    *)
        die "unknown command: $command (run with --help)"
        ;;
esac
