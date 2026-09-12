# Local Push Deploy Hooks

This folder contains local, CI-like deploy paths for the native web/WASM app
and the paired Steam Deck. They are intentionally local automation rather than
GitHub-hosted workflows because they use machine-local credentials, build
caches, and physical hardware. GitHub Actions runs checks on pushes; full
distribution packages are reserved for nightly and manual runs.

## Why This Exists

The project already deploys with `pnpm run deploy`, which builds the Rust/WASM web
bundle, uploads its manifest diff with Workers Static Assets, and deploys the
Cloudflare Worker. That command depends on this machine's Rust/pnpm setup,
existing Wrangler authentication, and incremental build cache. Public web
bundles build original assets from tracked inputs without Minecraft reference
files.

The usual wrapper, `git push && pnpm run deploy`, does not work well when another
tool performs the push. Git also has no normal client-side `post-push` hook.
The lightweight compromise here is a `pre-push` hook that schedules work in the
background, then lets the push continue.

## How It Works

`pre-push-hook.sh` receives Git's pushed refs and schedules a deploy only for
pushes to `main`. It schedules the always-on web lane and, when enabled in the
local checkout, an independent Steam Deck lane.

The web lane records the desired SHA under:

```text
.git/mclone-deploy-after-main-push/
```

`deploy-after-main-push.sh --worker` runs in the background. It waits until the
remote branch actually reports the pushed SHA before deploying, so a rejected or
failed push does not publish. A single-worker lock prevents overlapping deploys.
Quick successive pushes replace the pending SHA before deployment starts.

The Deck lane uses:

```text
.git/mclone-steam-deck-after-main-push/
```

It waits for the same remote confirmation, asks the standalone
`steamdeck-testbed` helper for a bounded read-only probe, and skips without
blocking the push when the Deck is unreachable. A reachable Deck is deployed
from a separate reusable sibling worktree at the exact pushed commit. The
production command checks the asset lock first, rebuilds the ignored archive
only when necessary, rechecks without ever rewriting the tracked lock, builds
in the pinned SteamRT4 SDK, and delegates validated incremental upload and
registration without launching the game or waking the panel. Launching remains
an explicit interactive command.

Deploys run from a reusable sibling worktree:

```text
../mclone-deploy-worktree
```

The worker resets that worktree to the pushed commit, links local ignored inputs
such as `reference/minecraft-1.17.1` and the root `node_modules` when present,
then runs `pnpm run deploy` there. The bundle command hydrates independently
locked nested packages such as Asset Lab with a frozen lockfile when needed.
This keeps the active checkout free for immediate follow-up editing while
preserving an incremental build cache in the deploy worktree.

## Commands

Install or refresh the local hook:

```bash
./scripts/local-deploy/install-hook.sh
```

Check pending, completed, failed, and timing state:

```bash
./scripts/local-deploy/deploy-after-main-push.sh --status
```

Enable, disable, and inspect the machine-local Deck lane:

```bash
pnpm steamdeck:auto-deploy:on
pnpm steamdeck:auto-deploy:off
pnpm steamdeck:auto-deploy:status
pnpm steamdeck:auto-deploy:log
```

New checkouts default to off. The toggle and optional Deck host are stored only
in the checkout's local Git configuration as
`mclone.steamDeckAutoDeploy` and `mclone.steamDeckHost`.

Follow the full web worker log:

```bash
tail -f .git/mclone-deploy-after-main-push/deploy.log
```

The Deck log is
`.git/mclone-steam-deck-after-main-push/deploy.log`.

## Status Files

The status command prints field headers for the TSV files so another agent can
answer "what deployed?" or "what failed?" without scraping the full log.

- `desired.tsv`: pending SHA, remote, branch, schedule time, and schedule epoch.
- `summary.tsv`: latest state, SHA, timing, worktree, and detail.
- `completed.tsv`: latest successful deploy with total seconds and deploy seconds.
- `failed.tsv`: latest failed deploy with failure kind, total seconds, worktree, and detail.
- `deploy.log`: complete append-only worker log.

`total_seconds` measures from hook scheduling to final success or failure.
`deploy_seconds` measures the `pnpm run deploy` command itself.

The Deck state directory uses the same summary/pending/completed/failed shape
and adds `skipped.tsv` for expected unavailability. Its status and completion
rows also record the machine-local Deck host and production deploy duration.

## Tunables

Environment variables:

- `MCLONE_DEPLOY_AFTER_PUSH_BRANCH`: branch to watch, default `main`.
- `MCLONE_DEPLOY_AFTER_PUSH_REMOTE`: default remote for manual scheduling, default `origin`.
- `MCLONE_DEPLOY_AFTER_PUSH_WORKTREE`: deploy worktree path, default `../mclone-deploy-worktree`.
- `MCLONE_DEPLOY_AFTER_PUSH_POLL_SECONDS`: remote polling interval, default `10`.
- `MCLONE_DEPLOY_AFTER_PUSH_SETTLE_SECONDS`: quick-successive-push settle window, default `30`.
- `MCLONE_DEPLOY_AFTER_PUSH_MAX_WAIT_SECONDS`: max wait for remote branch visibility, default `1800`.

The Deck lane shares the branch/remote settings and has:

- `MCLONE_STEAM_DECK_TESTBED`: path to the public physical-device CLI,
  default `~/code/steamdeck-testbed/bin/steamdeck`.
- `MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_WORKTREE`: reusable Deck worktree.
- `MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_POLL_SECONDS`: polling interval.
- `MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_SETTLE_SECONDS`: settle window.
- `MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_MAX_WAIT_SECONDS`: remote wait limit.
- `MCLONE_STEAM_DECK_DEPLOY_AFTER_PUSH_PROBE_SECONDS`: SSH probe limit,
  default `8`.
- `MCLONE_STEAM_DECK_DEPLOY_LOG_LINES`: lines printed by
  `steamdeck:auto-deploy:log`, default `200`.
