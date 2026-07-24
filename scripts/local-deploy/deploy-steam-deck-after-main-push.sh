#!/usr/bin/env bash
# Coordinate an opt-in Steam Deck deploy after GitHub accepts a push to main.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_PARENT="$(dirname "$PROJECT_DIR")"
PROJECT_NAME="$(basename "$PROJECT_DIR")"
SCRIPT_PATH="$SCRIPT_DIR/deploy-steam-deck-after-main-push.sh"
GIT_DIR="$(git -C "$PROJECT_DIR" rev-parse --git-dir)"
case "$GIT_DIR" in
  /*) ;;
  *) GIT_DIR="$PROJECT_DIR/$GIT_DIR" ;;
esac

STATE_DIR="$GIT_DIR/mclone-steam-deck-after-main-push"
DESIRED_FILE="$STATE_DIR/desired.tsv"
COMPLETED_FILE="$STATE_DIR/completed.tsv"
SKIPPED_FILE="$STATE_DIR/skipped.tsv"
FAILED_FILE="$STATE_DIR/failed.tsv"
SUMMARY_FILE="$STATE_DIR/summary.tsv"
LOCK_DIR="$STATE_DIR/worker.lock"
LOG_FILE="$STATE_DIR/deploy.log"
DEPLOY_WORKTREE="${MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_WORKTREE:-$PROJECT_PARENT/$PROJECT_NAME-steamdeck-deploy-worktree}"
REFERENCE_SOURCE="$PROJECT_DIR/reference/minecraft-1.17.1"

DEFAULT_BRANCH="${MCLONE_DEPLOY_AFTER_PUSH_BRANCH:-main}"
DEFAULT_REMOTE="${MCLONE_DEPLOY_AFTER_PUSH_REMOTE:-origin}"
POLL_SECONDS="${MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_POLL_SECONDS:-10}"
SETTLE_SECONDS="${MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_SETTLE_SECONDS:-30}"
MAX_WAIT_SECONDS="${MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_MAX_WAIT_SECONDS:-1800}"
PROBE_SECONDS="${MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_PROBE_SECONDS:-8}"
AUTO_DEPLOY_KEY="mclone.steamDeckAutoDeploy"
HOST_KEY="mclone.steamDeckHost"

usage() {
  cat <<EOF
Usage:
  $0 --enable [--host HOST]
  $0 --disable
  $0 --schedule --remote REMOTE --branch BRANCH --sha SHA
  $0 --worker
  $0 --status
  $0 --log

Auto-deploy is disabled by default and stored in this checkout's local Git
configuration. The pre-push hook calls --schedule. A background worker waits
until the remote branch reports the pushed SHA, probes the configured Deck,
and runs 'pnpm steamdeck:install:production' from a clean reusable worktree.
EOF
}

timestamp() {
  date -u +%Y-%m-%dT%H:%M:%SZ
}

epoch_seconds() {
  date -u +%s
}

append_log() {
  mkdir -p "$STATE_DIR"
  printf '[%s] %s\n' "$(timestamp)" "$*" >> "$LOG_FILE"
}

log() {
  printf '[%s] %s\n' "$(timestamp)" "$*"
}

write_tsv() {
  local path="$1"
  local tmp="$path.$$"
  shift
  printf '%s' "$1" > "$tmp"
  shift
  while (($#)); do
    printf '\t%s' "$1" >> "$tmp"
    shift
  done
  printf '\n' >> "$tmp"
  mv "$tmp" "$path"
}

configured_host() {
  local host="${MCLONE_STEAM_DECK:-}"
  if [ -z "$host" ]; then
    host="$(git -C "$PROJECT_DIR" config --local --get "$HOST_KEY" 2>/dev/null || true)"
  fi
  printf '%s' "${host:-steamdeck.local}"
}

auto_deploy_enabled() {
  local value
  value="$(git -C "$PROJECT_DIR" config --local --get "$AUTO_DEPLOY_KEY" 2>/dev/null || true)"
  case "${value,,}" in
    1|true|yes|on)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

write_summary() {
  local state="$1"
  local sha="${2:-}"
  local remote="${3:-}"
  local branch="${4:-}"
  local detail="${5:-}"
  local total_seconds="${6:-}"
  local deploy_seconds="${7:-}"
  mkdir -p "$STATE_DIR"
  write_tsv \
    "$SUMMARY_FILE" \
    "$(timestamp)" \
    "$state" \
    "$sha" \
    "$remote" \
    "$branch" \
    "$(configured_host)" \
    "$total_seconds" \
    "$deploy_seconds" \
    "$DEPLOY_WORKTREE" \
    "$detail"
}

record_failure() {
  local sha="$1"
  local remote="$2"
  local branch="$3"
  local status="$4"
  local detail="$5"
  local total_seconds="${6:-}"
  local deploy_seconds="${7:-}"
  write_tsv \
    "$FAILED_FILE" \
    "$sha" \
    "$remote" \
    "$branch" \
    "$(configured_host)" \
    "$(timestamp)" \
    "$status" \
    "$total_seconds" \
    "$deploy_seconds" \
    "$DEPLOY_WORKTREE" \
    "$detail"
  write_summary \
    "failed" \
    "$sha" \
    "$remote" \
    "$branch" \
    "$status: $detail" \
    "$total_seconds" \
    "$deploy_seconds"
}

record_skip() {
  local sha="$1"
  local remote="$2"
  local branch="$3"
  local reason="$4"
  local detail="$5"
  local total_seconds="${6:-}"
  write_tsv \
    "$SKIPPED_FILE" \
    "$sha" \
    "$remote" \
    "$branch" \
    "$(configured_host)" \
    "$(timestamp)" \
    "$reason" \
    "$total_seconds" \
    "$DEPLOY_WORKTREE" \
    "$detail"
  write_summary \
    "skipped" \
    "$sha" \
    "$remote" \
    "$branch" \
    "$reason: $detail" \
    "$total_seconds" \
    ""
}

read_desired() {
  [ -f "$DESIRED_FILE" ] || return 1
  IFS=$'\t' read -r \
    DESIRED_SHA \
    DESIRED_REMOTE \
    DESIRED_BRANCH \
    DESIRED_AT \
    DESIRED_EPOCH < "$DESIRED_FILE" || return 1
  DESIRED_EPOCH="${DESIRED_EPOCH:-0}"
  [ -n "${DESIRED_SHA:-}" ] &&
    [ -n "${DESIRED_REMOTE:-}" ] &&
    [ -n "${DESIRED_BRANCH:-}" ]
}

read_completed_sha() {
  if [ -f "$COMPLETED_FILE" ]; then
    IFS=$'\t' read -r COMPLETED_SHA _ < "$COMPLETED_FILE" || true
    printf '%s' "${COMPLETED_SHA:-}"
  fi
}

desired_still_matches() {
  local sha="$1"
  local remote="$2"
  local branch="$3"
  read_desired || return 1
  [ "$DESIRED_SHA" = "$sha" ] &&
    [ "$DESIRED_REMOTE" = "$remote" ] &&
    [ "$DESIRED_BRANCH" = "$branch" ]
}

remote_head_sha() {
  local remote="$1"
  local branch="$2"
  git -C "$PROJECT_DIR" ls-remote --heads "$remote" "$branch" |
    awk -v ref="refs/heads/$branch" '$2 == ref { print $1; exit }'
}

elapsed_since_desired() {
  local now
  now="$(epoch_seconds)"
  if [ "${DESIRED_EPOCH:-0}" -gt 0 ] 2>/dev/null; then
    printf '%s' "$((now - DESIRED_EPOCH))"
  else
    printf ''
  fi
}

is_git_worktree() {
  local path="$1"
  [ -d "$path" ] &&
    git -C "$path" rev-parse --is-inside-work-tree >/dev/null 2>&1
}

ensure_deploy_worktree() {
  local sha="$1"

  if is_git_worktree "$DEPLOY_WORKTREE"; then
    log "using Steam Deck deploy worktree at $DEPLOY_WORKTREE"
  else
    if [ -e "$DEPLOY_WORKTREE" ]; then
      echo "$DEPLOY_WORKTREE exists but is not a Git worktree" >&2
      return 1
    fi

    log "creating Steam Deck deploy worktree at $DEPLOY_WORKTREE"
    mkdir -p "$(dirname "$DEPLOY_WORKTREE")" || return 1
    git -C "$PROJECT_DIR" worktree add --detach "$DEPLOY_WORKTREE" "$sha" ||
      return 1
  fi

  git -C "$DEPLOY_WORKTREE" fetch --quiet "$DESIRED_REMOTE" "$DESIRED_BRANCH" || true
  git -C "$DEPLOY_WORKTREE" checkout --detach "$sha" || return 1
  git -C "$DEPLOY_WORKTREE" reset --hard "$sha" || return 1
  git -C "$DEPLOY_WORKTREE" clean -fd || return 1
}

ensure_reference_assets_copy() {
  local source="$REFERENCE_SOURCE/extracted"
  local target_parent="$DEPLOY_WORKTREE/reference/minecraft-1.17.1"
  local target="$target_parent/extracted"

  if [ ! -d "$source" ]; then
    echo "$source not found. Run ./scripts/decompile-mc.sh in the primary checkout first." >&2
    return 1
  fi

  command -v rsync >/dev/null 2>&1 || {
    echo "rsync is required to prepare isolated Deck assets" >&2
    return 1
  }

  mkdir -p "$target_parent" || return 1
  if [ -L "$target" ]; then
    rm "$target" || return 1
  elif [ -e "$target" ] && [ ! -d "$target" ]; then
    echo "$target exists and is not a directory" >&2
    return 1
  fi
  mkdir -p "$target" || return 1
  rsync -a --delete "$source/" "$target/" || return 1
}

acquire_lock() {
  mkdir -p "$STATE_DIR"

  while ! mkdir "$LOCK_DIR" 2>/dev/null; do
    local pid=""
    if [ -f "$LOCK_DIR/pid" ]; then
      pid="$(sed -n '1p' "$LOCK_DIR/pid" 2>/dev/null || true)"
    fi

    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
      log "another Steam Deck deploy worker is active as pid $pid; exiting"
      return 1
    fi

    log "removing stale Steam Deck deploy lock"
    rm -rf "$LOCK_DIR"
  done

  printf '%s\n' "$$" > "$LOCK_DIR/pid"
  trap 'rm -rf "$LOCK_DIR"' EXIT INT TERM
}

wait_for_settle_window() {
  local sha="$1"
  local remote="$2"
  local branch="$3"
  local until_seconds=$((SECONDS + SETTLE_SECONDS))

  while ((SECONDS < until_seconds)); do
    local remaining=$((until_seconds - SECONDS))
    local sleep_for="$POLL_SECONDS"
    if ((remaining < sleep_for)); then
      sleep_for="$remaining"
    fi
    if ((sleep_for > 0)); then
      sleep "$sleep_for"
    fi

    if ! desired_still_matches "$sha" "$remote" "$branch"; then
      log "newer push scheduled during settle window; skipping $sha"
      return 1
    fi
  done
}

probe_deck() {
  local host="$1"
  local user="${MCLONE_STEAM_DECK_USER:-deck}"
  local key="${MCLONE_STEAM_DECK_KEY:-$HOME/.config/steamos-devkit/devkit_rsa}"

  [ -f "$key" ] || {
    echo "Devkit key not found: $key" >&2
    return 1
  }
  command -v timeout >/dev/null 2>&1 || {
    echo "timeout command not found" >&2
    return 1
  }

  timeout "$PROBE_SECONDS" \
    ssh \
      -o BatchMode=yes \
      -o ConnectTimeout="$PROBE_SECONDS" \
      -o StrictHostKeyChecking=yes \
      -i "$key" \
      "$user@$host" \
      /usr/bin/true
}

clear_desired_if_current() {
  local sha="$1"
  local remote="$2"
  local branch="$3"
  if desired_still_matches "$sha" "$remote" "$branch"; then
    rm -f "$DESIRED_FILE"
  fi
}

run_worker() {
  cd "$PROJECT_DIR"
  acquire_lock || exit 0

  local last_target=""
  local remote_deadline=$((SECONDS + MAX_WAIT_SECONDS))

  log "Steam Deck deploy-after-push worker started"
  while true; do
    if ! auto_deploy_enabled; then
      log "Steam Deck auto-deploy is disabled; worker exiting"
      write_summary "disabled" "" "" "" "local checkout toggle is off"
      exit 0
    fi

    if ! read_desired; then
      log "no pending Steam Deck deploy; worker exiting"
      write_summary "idle" "" "" "" "no pending deploy"
      exit 0
    fi

    local sha="$DESIRED_SHA"
    local remote="$DESIRED_REMOTE"
    local branch="$DESIRED_BRANCH"
    local target="$remote/$branch@$sha"

    if [ "$target" != "$last_target" ]; then
      last_target="$target"
      remote_deadline=$((SECONDS + MAX_WAIT_SECONDS))
      log "watching for $target"
      write_summary \
        "waiting_remote" \
        "$sha" \
        "$remote" \
        "$branch" \
        "waiting for remote branch to reach pushed SHA"
    fi

    local completed_sha
    completed_sha="$(read_completed_sha)"
    if [ "$completed_sha" = "$sha" ]; then
      log "$sha is already deployed to the Deck; clearing pending request"
      write_summary \
        "already_deployed" \
        "$sha" \
        "$remote" \
        "$branch" \
        "pending SHA matches latest completed Deck deploy"
      clear_desired_if_current "$sha" "$remote" "$branch"
      continue
    fi

    local current_sha=""
    current_sha="$(remote_head_sha "$remote" "$branch" 2>/dev/null || true)"
    if [ "$current_sha" != "$sha" ]; then
      if ((SECONDS > remote_deadline)); then
        local detail="timed out waiting for $target to appear on the remote"
        log "$detail"
        record_failure \
          "$sha" \
          "$remote" \
          "$branch" \
          "remote-timeout" \
          "$detail" \
          "$(elapsed_since_desired)"
        exit 1
      fi

      if [ -n "$current_sha" ]; then
        log "remote $remote/$branch is $current_sha; waiting for $sha"
        write_summary \
          "waiting_remote" \
          "$sha" \
          "$remote" \
          "$branch" \
          "remote is $current_sha"
      else
        log "remote $remote/$branch is not readable yet; waiting for $sha"
        write_summary \
          "waiting_remote" \
          "$sha" \
          "$remote" \
          "$branch" \
          "remote is not readable yet"
      fi
      sleep "$POLL_SECONDS"
      continue
    fi

    log "remote reached $sha; waiting ${SETTLE_SECONDS}s for follow-up pushes"
    write_summary \
      "waiting_settle" \
      "$sha" \
      "$remote" \
      "$branch" \
      "remote reached pushed SHA; waiting for quick follow-up pushes"
    wait_for_settle_window "$sha" "$remote" "$branch" || continue

    current_sha="$(remote_head_sha "$remote" "$branch" 2>/dev/null || true)"
    if [ "$current_sha" != "$sha" ]; then
      log "remote moved from $sha to ${current_sha:-unknown}; rechecking"
      write_summary \
        "waiting_remote" \
        "$sha" \
        "$remote" \
        "$branch" \
        "remote moved to ${current_sha:-unknown}"
      continue
    fi

    if ! desired_still_matches "$sha" "$remote" "$branch"; then
      log "pending Steam Deck request changed before deploy; rechecking"
      write_summary \
        "superseded" \
        "$sha" \
        "$remote" \
        "$branch" \
        "newer push replaced this pending deploy"
      continue
    fi

    local host
    host="$(configured_host)"
    log "probing Steam Deck SSH at $host"
    write_summary \
      "probing" \
      "$sha" \
      "$remote" \
      "$branch" \
      "checking Devkit-managed SSH reachability"
    if ! probe_deck "$host"; then
      local detail="Devkit-managed SSH was not reachable within ${PROBE_SECONDS}s"
      log "$detail; skipping this push"
      record_skip \
        "$sha" \
        "$remote" \
        "$branch" \
        "deck-unreachable" \
        "$detail" \
        "$(elapsed_since_desired)"
      clear_desired_if_current "$sha" "$remote" "$branch"
      continue
    fi

    log "preparing Steam Deck deploy worktree for $sha"
    write_summary \
      "preparing_worktree" \
      "$sha" \
      "$remote" \
      "$branch" \
      "checking out clean pushed commit"
    if ! ensure_deploy_worktree "$sha"; then
      local detail="failed to prepare Deck worktree at $DEPLOY_WORKTREE"
      log "$detail"
      record_failure \
        "$sha" \
        "$remote" \
        "$branch" \
        "worktree-failed" \
        "$detail" \
        "$(elapsed_since_desired)"
      exit 1
    fi

    if ! ensure_reference_assets_copy; then
      local detail="failed to copy extracted reference assets into Deck worktree"
      log "$detail"
      record_failure \
        "$sha" \
        "$remote" \
        "$branch" \
        "reference-assets-failed" \
        "$detail" \
        "$(elapsed_since_desired)"
      exit 1
    fi

    local deploy_start_epoch
    deploy_start_epoch="$(epoch_seconds)"
    log "starting production Steam Deck install for $sha"
    write_summary \
      "deploying" \
      "$sha" \
      "$remote" \
      "$branch" \
      "running pnpm steamdeck:install:production"
    if (
      cd "$DEPLOY_WORKTREE"
      MCLONE_STEAM_DECK="$host" pnpm steamdeck:install:production
    ); then
      local deploy_end_epoch total_seconds deploy_seconds
      deploy_end_epoch="$(epoch_seconds)"
      deploy_seconds="$((deploy_end_epoch - deploy_start_epoch))"
      total_seconds="$(elapsed_since_desired)"
      log "production Steam Deck install succeeded for $sha"
      write_tsv \
        "$COMPLETED_FILE" \
        "$sha" \
        "$remote" \
        "$branch" \
        "$host" \
        "$(timestamp)" \
        "$total_seconds" \
        "$deploy_seconds" \
        "$DEPLOY_WORKTREE"
      write_summary \
        "succeeded" \
        "$sha" \
        "$remote" \
        "$branch" \
        "production Steam Deck install succeeded without launching" \
        "$total_seconds" \
        "$deploy_seconds"
      clear_desired_if_current "$sha" "$remote" "$branch"
    else
      local status=$?
      local deploy_seconds
      deploy_seconds="$(( $(epoch_seconds) - deploy_start_epoch ))"
      local detail="production Deck install exited with $status after ${deploy_seconds}s"
      log "$detail"
      record_failure \
        "$sha" \
        "$remote" \
        "$branch" \
        "deploy-failed" \
        "$detail" \
        "$(elapsed_since_desired)" \
        "$deploy_seconds"
      exit "$status"
    fi
  done
}

schedule() {
  local remote="$DEFAULT_REMOTE"
  local branch="$DEFAULT_BRANCH"
  local sha=""

  while (($#)); do
    case "$1" in
      --remote)
        remote="$2"
        shift 2
        ;;
      --branch)
        branch="$2"
        shift 2
        ;;
      --sha)
        sha="$2"
        shift 2
        ;;
      *)
        echo "unknown --schedule argument: $1" >&2
        usage >&2
        exit 2
        ;;
    esac
  done

  if [ -z "$remote" ] || [ -z "$branch" ] || [ -z "$sha" ]; then
    echo "--schedule requires --remote, --branch, and --sha" >&2
    exit 2
  fi

  if ! auto_deploy_enabled; then
    exit 0
  fi

  mkdir -p "$STATE_DIR"
  write_tsv \
    "$DESIRED_FILE" \
    "$sha" \
    "$remote" \
    "$branch" \
    "$(timestamp)" \
    "$(epoch_seconds)"
  write_summary \
    "scheduled" \
    "$sha" \
    "$remote" \
    "$branch" \
    "background Steam Deck worker scheduled"
  append_log "scheduled Steam Deck deploy after $remote/$branch reaches $sha"

  nohup "$SCRIPT_PATH" --worker >> "$LOG_FILE" 2>&1 </dev/null &
  append_log "Steam Deck worker wake-up requested as pid $!"
}

enable_auto_deploy() {
  local host=""

  while (($#)); do
    case "$1" in
      --host)
        host="$2"
        shift 2
        ;;
      *)
        echo "unknown --enable argument: $1" >&2
        usage >&2
        exit 2
        ;;
    esac
  done

  if [ -n "$host" ]; then
    git -C "$PROJECT_DIR" config --local "$HOST_KEY" "$host"
  fi
  git -C "$PROJECT_DIR" config --local "$AUTO_DEPLOY_KEY" true
  mkdir -p "$STATE_DIR"
  append_log "Steam Deck auto-deploy enabled for $(configured_host)"
  write_summary "enabled" "" "" "" "future main pushes will schedule Deck deploys"
  printf 'Steam Deck auto-deploy: enabled\n'
  printf 'Deck host: %s\n' "$(configured_host)"
}

disable_auto_deploy() {
  git -C "$PROJECT_DIR" config --local "$AUTO_DEPLOY_KEY" false
  mkdir -p "$STATE_DIR"
  rm -f "$DESIRED_FILE"
  append_log "Steam Deck auto-deploy disabled"
  write_summary "disabled" "" "" "" "future pushes will not schedule Deck deploys"
  printf 'Steam Deck auto-deploy: disabled\n'
  printf 'An already-running build or upload is not forcibly terminated.\n'
}

show_status() {
  mkdir -p "$STATE_DIR"
  if auto_deploy_enabled; then
    echo "enabled: yes"
  else
    echo "enabled: no"
  fi
  echo "Deck host: $(configured_host)"
  echo "state dir: $STATE_DIR"
  echo "deploy worktree: $DEPLOY_WORKTREE"
  echo "summary fields: timestamp state sha remote branch host total_seconds deploy_seconds worktree detail"
  if [ -f "$SUMMARY_FILE" ]; then
    echo "summary: $(cat "$SUMMARY_FILE")"
  else
    echo "summary: none"
  fi
  echo "pending fields: sha remote branch scheduled_at scheduled_epoch"
  if [ -f "$DESIRED_FILE" ]; then
    echo "pending: $(cat "$DESIRED_FILE")"
  else
    echo "pending: none"
  fi
  echo "completed fields: sha remote branch host completed_at total_seconds deploy_seconds worktree"
  if [ -f "$COMPLETED_FILE" ]; then
    echo "latest completed: $(cat "$COMPLETED_FILE")"
  else
    echo "latest completed: none"
  fi
  echo "skipped fields: sha remote branch host skipped_at reason total_seconds worktree detail"
  if [ -f "$SKIPPED_FILE" ]; then
    echo "latest skipped: $(cat "$SKIPPED_FILE")"
  else
    echo "latest skipped: none"
  fi
  echo "failed fields: sha remote branch host failed_at status total_seconds deploy_seconds worktree detail"
  if [ -f "$FAILED_FILE" ]; then
    echo "latest failed: $(cat "$FAILED_FILE")"
  else
    echo "latest failed: none"
  fi
  if [ -f "$LOCK_DIR/pid" ]; then
    echo "worker pid: $(cat "$LOCK_DIR/pid")"
  else
    echo "worker pid: none"
  fi
  echo "log: $LOG_FILE"
}

show_log() {
  if [ -f "$LOG_FILE" ]; then
    tail -n "${MCLONE_STEAM_DECK_DEPLOY_LOG_LINES:-200}" "$LOG_FILE"
  else
    echo "Steam Deck auto-deploy log does not exist yet: $LOG_FILE"
  fi
}

case "${1:-}" in
  --enable)
    shift
    enable_auto_deploy "$@"
    ;;
  --disable)
    disable_auto_deploy
    ;;
  --schedule)
    shift
    schedule "$@"
    ;;
  --worker)
    run_worker
    ;;
  --status)
    show_status
    ;;
  --log)
    show_log
    ;;
  -h|--help|"")
    usage
    ;;
  *)
    echo "unknown argument: $1" >&2
    usage >&2
    exit 2
    ;;
esac
