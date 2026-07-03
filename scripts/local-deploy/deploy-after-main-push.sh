#!/usr/bin/env bash
# Coordinate a background native web deploy after GitHub accepts a push to main.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_PARENT="$(dirname "$PROJECT_DIR")"
PROJECT_NAME="$(basename "$PROJECT_DIR")"
SCRIPT_PATH="$SCRIPT_DIR/deploy-after-main-push.sh"
GIT_DIR="$(git -C "$PROJECT_DIR" rev-parse --git-dir)"
case "$GIT_DIR" in
  /*) ;;
  *) GIT_DIR="$PROJECT_DIR/$GIT_DIR" ;;
esac

STATE_DIR="$GIT_DIR/mclone-deploy-after-main-push"
DESIRED_FILE="$STATE_DIR/desired.tsv"
COMPLETED_FILE="$STATE_DIR/completed.tsv"
FAILED_FILE="$STATE_DIR/failed.tsv"
SUMMARY_FILE="$STATE_DIR/summary.tsv"
LOCK_DIR="$STATE_DIR/worker.lock"
LOG_FILE="$STATE_DIR/deploy.log"
DEPLOY_WORKTREE="${MCLONE_DEPLOY_AFTER_PUSH_WORKTREE:-$PROJECT_PARENT/$PROJECT_NAME-deploy-worktree}"
REFERENCE_SOURCE="$PROJECT_DIR/reference/minecraft-1.17.1"
NODE_MODULES_SOURCE="$PROJECT_DIR/node_modules"

DEFAULT_BRANCH="${MCLONE_DEPLOY_AFTER_PUSH_BRANCH:-main}"
DEFAULT_REMOTE="${MCLONE_DEPLOY_AFTER_PUSH_REMOTE:-origin}"
POLL_SECONDS="${MCLONE_DEPLOY_AFTER_PUSH_POLL_SECONDS:-10}"
SETTLE_SECONDS="${MCLONE_DEPLOY_AFTER_PUSH_SETTLE_SECONDS:-30}"
MAX_WAIT_SECONDS="${MCLONE_DEPLOY_AFTER_PUSH_MAX_WAIT_SECONDS:-1800}"

usage() {
  cat <<EOF
Usage:
  $0 --schedule --remote REMOTE --branch BRANCH --sha SHA
  $0 --worker
  $0 --status

The pre-push hook calls --schedule. A background worker then waits until the
remote branch reports the pushed SHA before running 'pnpm run deploy'.
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

write_summary() {
  local state="$1"
  local sha="${2:-}"
  local remote="${3:-}"
  local branch="${4:-}"
  local detail="${5:-}"
  local total_seconds="${6:-}"
  local deploy_seconds="${7:-}"
  mkdir -p "$STATE_DIR"
  write_tsv "$SUMMARY_FILE" "$(timestamp)" "$state" "$sha" "$remote" "$branch" "$total_seconds" "$deploy_seconds" "$DEPLOY_WORKTREE" "$detail"
}

record_failure() {
  local sha="$1"
  local remote="$2"
  local branch="$3"
  local status="$4"
  local detail="$5"
  local total_seconds="${6:-}"
  write_tsv "$FAILED_FILE" "$sha" "$remote" "$branch" "$(timestamp)" "$status" "$total_seconds" "$DEPLOY_WORKTREE" "$detail"
  write_summary "failed" "$sha" "$remote" "$branch" "$status: $detail" "$total_seconds" ""
}

read_desired() {
  [ -f "$DESIRED_FILE" ] || return 1
  IFS=$'\t' read -r DESIRED_SHA DESIRED_REMOTE DESIRED_BRANCH DESIRED_AT DESIRED_EPOCH < "$DESIRED_FILE" || return 1
  DESIRED_EPOCH="${DESIRED_EPOCH:-0}"
  [ -n "${DESIRED_SHA:-}" ] && [ -n "${DESIRED_REMOTE:-}" ] && [ -n "${DESIRED_BRANCH:-}" ]
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
  [ "$DESIRED_SHA" = "$sha" ] && [ "$DESIRED_REMOTE" = "$remote" ] && [ "$DESIRED_BRANCH" = "$branch" ]
}

remote_head_sha() {
  local remote="$1"
  local branch="$2"
  git -C "$PROJECT_DIR" ls-remote --heads "$remote" "$branch" | awk -v ref="refs/heads/$branch" '$2 == ref { print $1; exit }'
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
  [ -d "$path" ] && git -C "$path" rev-parse --is-inside-work-tree >/dev/null 2>&1
}

ensure_deploy_worktree() {
  local sha="$1"

  if is_git_worktree "$DEPLOY_WORKTREE"; then
    log "using deploy worktree at $DEPLOY_WORKTREE"
  else
    if [ -e "$DEPLOY_WORKTREE" ]; then
      echo "$DEPLOY_WORKTREE exists but is not a Git worktree" >&2
      return 1
    fi

    log "creating deploy worktree at $DEPLOY_WORKTREE"
    mkdir -p "$(dirname "$DEPLOY_WORKTREE")"
    git -C "$PROJECT_DIR" worktree add --detach "$DEPLOY_WORKTREE" "$sha"
  fi

  git -C "$DEPLOY_WORKTREE" fetch --quiet "$DESIRED_REMOTE" "$DESIRED_BRANCH" || true
  git -C "$DEPLOY_WORKTREE" checkout --detach "$sha"
  git -C "$DEPLOY_WORKTREE" reset --hard "$sha"
  git -C "$DEPLOY_WORKTREE" clean -fd
}

ensure_reference_assets_link() {
  local target_parent="$DEPLOY_WORKTREE/reference"
  local target="$target_parent/minecraft-1.17.1"

  if [ ! -d "$REFERENCE_SOURCE" ]; then
    echo "$REFERENCE_SOURCE not found. Run ./scripts/decompile-mc.sh in the primary checkout first." >&2
    return 1
  fi

  mkdir -p "$target_parent"
  if [ -L "$target" ]; then
    local current
    current="$(readlink "$target")"
    if [ "$current" != "$REFERENCE_SOURCE" ]; then
      rm "$target"
      ln -s "$REFERENCE_SOURCE" "$target"
    fi
  elif [ -e "$target" ]; then
    echo "$target exists and is not the expected symlink to $REFERENCE_SOURCE" >&2
    return 1
  else
    ln -s "$REFERENCE_SOURCE" "$target"
  fi
}

ensure_node_modules_link() {
  local target="$DEPLOY_WORKTREE/node_modules"

  if [ ! -d "$NODE_MODULES_SOURCE" ]; then
    return 0
  fi

  if [ -L "$target" ]; then
    local current
    current="$(readlink "$target")"
    if [ "$current" != "$NODE_MODULES_SOURCE" ]; then
      rm "$target"
      ln -s "$NODE_MODULES_SOURCE" "$target"
    fi
  elif [ -e "$target" ]; then
    return 0
  else
    ln -s "$NODE_MODULES_SOURCE" "$target"
  fi
}

acquire_lock() {
  mkdir -p "$STATE_DIR"

  while ! mkdir "$LOCK_DIR" 2>/dev/null; do
    local pid=""
    if [ -f "$LOCK_DIR/pid" ]; then
      pid="$(sed -n '1p' "$LOCK_DIR/pid" 2>/dev/null || true)"
    fi

    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
      log "another deploy-after-push worker is already active as pid $pid; exiting"
      return 1
    fi

    log "removing stale deploy-after-push lock"
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

run_worker() {
  cd "$PROJECT_DIR"
  acquire_lock || exit 0

  local last_target=""
  local remote_deadline=$((SECONDS + MAX_WAIT_SECONDS))

  log "deploy-after-push worker started"
  while true; do
    if ! read_desired; then
      log "no pending deploy; worker exiting"
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
      write_summary "waiting_remote" "$sha" "$remote" "$branch" "waiting for remote branch to reach pushed SHA"
    fi

    local completed_sha
    completed_sha="$(read_completed_sha)"
    if [ "$completed_sha" = "$sha" ]; then
      log "$sha is already deployed; clearing pending request"
      write_summary "already_deployed" "$sha" "$remote" "$branch" "pending SHA matches latest completed deploy"
      if desired_still_matches "$sha" "$remote" "$branch"; then
        rm -f "$DESIRED_FILE"
      fi
      continue
    fi

    local current_sha=""
    current_sha="$(remote_head_sha "$remote" "$branch" 2>/dev/null || true)"
    if [ "$current_sha" != "$sha" ]; then
      if ((SECONDS > remote_deadline)); then
        local detail="timed out waiting for $target to appear on the remote"
        log "$detail"
        record_failure "$sha" "$remote" "$branch" "remote-timeout" "$detail" "$(elapsed_since_desired)"
        exit 1
      fi

      if [ -n "$current_sha" ]; then
        log "remote $remote/$branch is $current_sha; waiting for $sha"
        write_summary "waiting_remote" "$sha" "$remote" "$branch" "remote is $current_sha"
      else
        log "remote $remote/$branch is not readable yet; waiting for $sha"
        write_summary "waiting_remote" "$sha" "$remote" "$branch" "remote is not readable yet"
      fi
      sleep "$POLL_SECONDS"
      continue
    fi

    log "remote reached $sha; waiting ${SETTLE_SECONDS}s for quick follow-up pushes"
    write_summary "waiting_settle" "$sha" "$remote" "$branch" "remote reached pushed SHA; waiting for quick follow-up pushes"
    wait_for_settle_window "$sha" "$remote" "$branch" || continue

    current_sha="$(remote_head_sha "$remote" "$branch" 2>/dev/null || true)"
    if [ "$current_sha" != "$sha" ]; then
      log "remote moved from $sha to ${current_sha:-unknown}; rechecking pending request"
      write_summary "waiting_remote" "$sha" "$remote" "$branch" "remote moved to ${current_sha:-unknown}"
      continue
    fi

    if ! desired_still_matches "$sha" "$remote" "$branch"; then
      log "pending request changed before deploy; rechecking"
      write_summary "superseded" "$sha" "$remote" "$branch" "newer push replaced this pending deploy"
      continue
    fi

    log "preparing deploy worktree for $sha"
    write_summary "preparing_worktree" "$sha" "$remote" "$branch" "checking out deploy worktree"
    if ! ensure_deploy_worktree "$sha"; then
      local detail="failed to prepare deploy worktree at $DEPLOY_WORKTREE"
      log "$detail"
      record_failure "$sha" "$remote" "$branch" "worktree-failed" "$detail" "$(elapsed_since_desired)"
      exit 1
    fi

    if ! ensure_reference_assets_link; then
      local detail="failed to link local reference assets into deploy worktree"
      log "$detail"
      record_failure "$sha" "$remote" "$branch" "reference-assets-failed" "$detail" "$(elapsed_since_desired)"
      exit 1
    fi

    if ! ensure_node_modules_link; then
      local detail="failed to link node_modules into deploy worktree"
      log "$detail"
      record_failure "$sha" "$remote" "$branch" "node-modules-failed" "$detail" "$(elapsed_since_desired)"
      exit 1
    fi

    local deploy_start_epoch
    deploy_start_epoch="$(epoch_seconds)"
    log "starting pnpm run deploy for $sha"
    write_summary "deploying" "$sha" "$remote" "$branch" "running pnpm run deploy"
    if (cd "$DEPLOY_WORKTREE" && pnpm run deploy); then
      local deploy_end_epoch total_seconds deploy_seconds
      deploy_end_epoch="$(epoch_seconds)"
      deploy_seconds="$((deploy_end_epoch - deploy_start_epoch))"
      total_seconds="$(elapsed_since_desired)"
      log "pnpm run deploy succeeded for $sha"
      write_tsv "$COMPLETED_FILE" "$sha" "$remote" "$branch" "$(timestamp)" "$total_seconds" "$deploy_seconds" "$DEPLOY_WORKTREE"
      write_summary "succeeded" "$sha" "$remote" "$branch" "pnpm run deploy succeeded" "$total_seconds" "$deploy_seconds"
      if desired_still_matches "$sha" "$remote" "$branch"; then
        rm -f "$DESIRED_FILE"
      fi
    else
      local status=$?
      log "pnpm run deploy failed for $sha with exit code $status"
      local deploy_seconds
      deploy_seconds="$(( $(epoch_seconds) - deploy_start_epoch ))"
      record_failure "$sha" "$remote" "$branch" "deploy-failed" "pnpm run deploy exited with $status after ${deploy_seconds}s" "$(elapsed_since_desired)"
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

  mkdir -p "$STATE_DIR"
  write_tsv "$DESIRED_FILE" "$sha" "$remote" "$branch" "$(timestamp)" "$(epoch_seconds)"
  write_summary "scheduled" "$sha" "$remote" "$branch" "background worker scheduled"
  append_log "scheduled deploy after $remote/$branch reaches $sha"

  nohup "$SCRIPT_PATH" --worker >> "$LOG_FILE" 2>&1 </dev/null &
  append_log "worker wake-up requested as pid $!"
}

show_status() {
  mkdir -p "$STATE_DIR"
  echo "state dir: $STATE_DIR"
  echo "deploy worktree: $DEPLOY_WORKTREE"
  echo "summary fields: timestamp state sha remote branch total_seconds deploy_seconds worktree detail"
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
  echo "completed fields: sha remote branch completed_at total_seconds deploy_seconds worktree"
  if [ -f "$COMPLETED_FILE" ]; then
    echo "latest completed: $(cat "$COMPLETED_FILE")"
  else
    echo "latest completed: none"
  fi
  echo "failed fields: sha remote branch failed_at status total_seconds worktree detail"
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

case "${1:-}" in
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
  -h|--help|"")
    usage
    ;;
  *)
    echo "unknown argument: $1" >&2
    usage >&2
    exit 2
    ;;
esac
