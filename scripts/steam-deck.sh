#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
STAGE_DIR="$REPO_ROOT/dist/steamdeck"
PAYLOAD_RUN="$REPO_ROOT/scripts/steam-deck/payload-run.sh"
ASSET_PACK="$REPO_ROOT/reference/minecraft-1.17.1/extracted.zip"
STEAMRT4_BUILD_SCRIPT="$REPO_ROOT/scripts/steam-deck-build-steamrt4.sh"
MATRIX_SUMMARIZER="$REPO_ROOT/scripts/steam-deck/summarize-perf-matrix.mjs"
STEAMDECK_TESTBED=${MCLONE_STEAM_DECK_TESTBED:-"$HOME/code/steamdeck-testbed/bin/steamdeck"}

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
DECK_HOST=${MCLONE_STEAM_DECK:-${CONFIGURED_DECK_HOST:-steamdeck}}
DECK_USER=${MCLONE_STEAM_DECK_USER:-deck}
DEFAULT_DECK_KEY=
if [[ -f $HOME/.config/steamos-devkit/devkit_rsa ]]; then
    DEFAULT_DECK_KEY="$HOME/.config/steamos-devkit/devkit_rsa"
fi
DECK_KEY=${MCLONE_STEAM_DECK_KEY:-$DEFAULT_DECK_KEY}
DECK_TITLE=${MCLONE_STEAM_DECK_TITLE:-mclone}
DECK_SCREEN_OFF_TITLE=${MCLONE_STEAM_DECK_SCREEN_OFF_TITLE:-screenoff}
DECK_RUNTIME=${MCLONE_STEAM_DECK_RUNTIME:-SteamLinuxRuntime_4}
REMOTE_RESULT_ROOT=${MCLONE_STEAM_DECK_REMOTE_RESULTS:-"/home/$DECK_USER/.local/state/mclone-deck/results"}
LOCAL_RESULT_ROOT=${MCLONE_STEAM_DECK_RESULTS_ROOT:-/tmp/mclone-steam-deck-results}

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
  upload        Stage, upload, and register mclone plus the screen-off tile.
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
  matrix        Run and summarize the native-panel stationary/traversal
                render-distance and scheduling-policy performance matrix.
  matrix-smoke  Run the two-row RD5 settled-start/traversal matrix smoke.
  matrix-attribution
                Run RD13 native/half-resolution and live/frozen-fluid A/Bs.
  matrix-workers
                Run RD10/RD13 traversal with one, two, and derived workers.
  pull-results  Pull the newest Deck result, or the optional RUN_ID argument.

Environment:
  MCLONE_STEAM_DECK=HOST
      Deck SSH alias, hostname, or address. Defaults to steamdeck.
  MCLONE_STEAM_DECK_TESTBED=PATH
      Shared physical-device CLI. Defaults to
      ~/code/steamdeck-testbed/bin/steamdeck.
  MCLONE_STEAM_DECK_SKIP_BUILD=1
      Reuse the selected builder's existing binary while staging.
  MCLONE_STEAM_DECK_BUILDER=host|steamrt4
      Build on Ubuntu or in the pinned SteamRT4 SDK. Defaults to host.
  MCLONE_STEAMRT4_CARGO_FEATURES=FEATURES
      Optional Cargo feature list recorded in the SteamRT4 build receipt.
      Performance matrices automatically enable perf-diagnostics.
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

steamdeck_cli()
{
    STEAMDECK_HOST="$DECK_HOST" \
    STEAMDECK_USER="$DECK_USER" \
    STEAMDECK_REMOTE_USER="$DECK_USER" \
    STEAMDECK_KEY="$DECK_KEY" \
        "$STEAMDECK_TESTBED" "$@"
}

require_deck()
{
    test -x "$STEAMDECK_TESTBED" ||
        die "Steam Deck testbed CLI not found: $STEAMDECK_TESTBED"
    steamdeck_cli probe >/dev/null
}

deck_power_status()
{
    require_deck
    steamdeck_cli power-status
}

set_deck_internal_screen_sleep()
{
    local requested=$1
    local mode=on
    require_deck
    [[ $requested == true ]] && mode=off
    steamdeck_cli "screen-$mode"
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
    local gameid=${1:-$DECK_TITLE}
    steamdeck_cli prepare-upload "$gameid"
}

write_title_manifest()
{
    local manifest=$1
    local mode=$2
    local run_id=${3:-}

    if [[ -n $run_id ]]; then
        jq -n \
            --arg schema "steamdeck-testbed.game.v1" \
            --arg gameid "$DECK_TITLE" \
            --arg payload "$STAGE_DIR" \
            --arg mode "$mode" \
            --arg resultRoot "$REMOTE_RESULT_ROOT" \
            --arg runId "$run_id" \
            --arg runtime "$DECK_RUNTIME" \
            '{
                schema: $schema,
                game_id: $gameid,
                payload: $payload,
                argv: ["./run.sh", $mode, $runId, $resultRoot],
                env: {
                    MCLONE_DECK_RESULT_ROOT: $resultRoot,
                    MCLONE_DECK_RESULT_ID: $runId
                },
                runtime: (if $runtime == "none" then null else $runtime end),
                force_appid: ""
            }' >"$manifest"
    else
        jq -n \
            --arg schema "steamdeck-testbed.game.v1" \
            --arg gameid "$DECK_TITLE" \
            --arg payload "$STAGE_DIR" \
            --arg mode "$mode" \
            --arg resultRoot "$REMOTE_RESULT_ROOT" \
            --arg runtime "$DECK_RUNTIME" \
            '{
                schema: $schema,
                game_id: $gameid,
                payload: $payload,
                argv: ["./run.sh", $mode],
                env: {MCLONE_DECK_RESULT_ROOT: $resultRoot},
                runtime: (if $runtime == "none" then null else $runtime end),
                force_appid: ""
            }' >"$manifest"
    fi
}

run_title_manifest()
{
    local action=$1
    local mode=$2
    local run_id=${3:-}
    local manifest output status
    manifest=$(mktemp)
    write_title_manifest "$manifest" "$mode" "$run_id"
    set +e
    output=$(steamdeck_cli "$action" "$manifest")
    status=$?
    set -e
    unlink "$manifest"
    (( status == 0 )) || return "$status"
    printf '%s\n' "$output"
}

register_title()
{
    local expected_remote_dir=$1
    local mode=$2
    local run_id=${3:-}
    local remote_dir
    remote_dir=$(run_title_manifest register "$mode" "$run_id")
    [[ $remote_dir == "$expected_remote_dir" ]] ||
        die "testbed registered unexpected directory: $remote_dir"
}

upload_payload()
{
    require_deck
    test -x "$STAGE_DIR/run.sh" || die "staged payload is missing; run stage first"
    local remote_dir
    remote_dir=$(run_title_manifest install play)
    steamdeck_cli screen-shortcut install \
        --game-id "$DECK_SCREEN_OFF_TITLE" \
        >/dev/null
    printf '%s\n' "$remote_dir"
}

launch_title()
{
    require_deck
    stop_title
    steamdeck_cli launch "$DECK_TITLE"
}

stop_title()
{
    local remote_dir
    remote_dir=$(prepare_remote_directory "$DECK_TITLE")
    steamdeck_cli exec -- "$remote_dir/run.sh" stop
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
        status=$(steamdeck_cli exec -- \
            /usr/bin/bash -lc \
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
        run_id=$(steamdeck_cli exec -- \
            /usr/bin/bash -lc \
            "/usr/bin/find $(printf '%q' "$REMOTE_RESULT_ROOT") -mindepth 1 -maxdepth 1 -type d -printf '%T@ %f\\n' 2>/dev/null | /usr/bin/sort -nr | /usr/bin/head -1 | /usr/bin/cut -d' ' -f2-")
    fi
    [[ $run_id =~ ^[A-Za-z0-9._-]+$ ]] ||
        die "invalid or missing Deck result id: ${run_id:-missing}"

    local destination="$LOCAL_RESULT_ROOT/$run_id"
    mkdir -p "$destination"
    steamdeck_cli pull \
        "$REMOTE_RESULT_ROOT/$run_id/" \
        "$destination/" \
        >&2
    echo "$destination"
}

cleanup_bounded_run()
{
    local remote_dir=$1
    register_title "$remote_dir" play >/dev/null 2>&1 || true
    set_deck_internal_screen_sleep true >/dev/null 2>&1 || true
}

run_bounded()
{
    local mode=$1
    local timeout_seconds=$2
    local remote_dir run_id status=0 destination
    local cleanup_command

    remote_dir=$(prepare_remote_directory "$DECK_TITLE")
    printf -v cleanup_command 'cleanup_bounded_run %q' \
        "$remote_dir"
    trap "$cleanup_command" EXIT
    set_deck_internal_screen_sleep false
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
    if [[ $mode == perf-matrix* ]]; then
        require_command node
        node "$MATRIX_SUMMARIZER" "$destination"
    fi
    set_deck_internal_screen_sleep true
    trap - EXIT
    return "$status"
}

run_bounded_workflow()
{
    local mode=$1
    local timeout_seconds=$2
    trap \
        'set_deck_internal_screen_sleep true >/dev/null 2>&1 || true' \
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
        steamdeck_cli status --json
        ;;
    power-status)
        [[ $# == 0 ]] || die "power-status takes no arguments"
        deck_power_status
        ;;
    screen-off)
        [[ $# == 0 ]] || die "screen-off takes no arguments"
        set_deck_internal_screen_sleep true
        ;;
    screen-on)
        [[ $# == 0 ]] || die "screen-on takes no arguments"
        set_deck_internal_screen_sleep false
        ;;
    stage)
        [[ $# == 0 ]] || die "stage takes no arguments"
        stage_payload
        ;;
    upload)
        [[ $# == 0 ]] || die "upload takes no arguments"
        stage_payload
        upload_payload >/dev/null
        echo \
            "Uploaded and registered Devkit Games: $DECK_TITLE; $DECK_SCREEN_OFF_TITLE"
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
    matrix)
        [[ $# == 0 ]] || die "matrix takes no arguments"
        MCLONE_STEAMRT4_CARGO_FEATURES=perf-diagnostics \
            run_bounded_workflow perf-matrix 5400
        ;;
    matrix-smoke)
        [[ $# == 0 ]] || die "matrix-smoke takes no arguments"
        MCLONE_STEAMRT4_CARGO_FEATURES=perf-diagnostics \
            run_bounded_workflow perf-matrix-smoke 900
        ;;
    matrix-attribution)
        [[ $# == 0 ]] || die "matrix-attribution takes no arguments"
        MCLONE_STEAMRT4_CARGO_FEATURES=perf-diagnostics \
            run_bounded_workflow perf-matrix-attribution 2700
        ;;
    matrix-workers)
        [[ $# == 0 ]] || die "matrix-workers takes no arguments"
        MCLONE_STEAMRT4_CARGO_FEATURES=perf-diagnostics \
            run_bounded_workflow perf-matrix-workers 3600
        ;;
    pull-results)
        [[ $# -le 1 ]] || die "pull-results accepts at most one RUN_ID"
        pull_result "${1:-}"
        ;;
    *)
        die "unknown command: $command (run with --help)"
        ;;
esac
