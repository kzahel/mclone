# Local Web Deploy Hook

This folder contains the local, CI-like deploy path for the native web/WASM app.
It is intentionally local automation rather than a GitHub-hosted workflow.

## Why This Exists

The project already deploys with `pnpm run deploy`, which builds the Rust/WASM web
bundle, uploads it to R2, and deploys the Cloudflare Worker. That command depends
on this machine's Rust/pnpm setup, Cloudflare authentication, ignored
Minecraft reference assets, and incremental build cache.

The usual wrapper, `git push && pnpm run deploy`, does not work well when another
tool performs the push. Git also has no normal client-side `post-push` hook.
The lightweight compromise here is a `pre-push` hook that schedules work in the
background, then lets the push continue.

## How It Works

`pre-push-hook.sh` receives Git's pushed refs and schedules a deploy only for
pushes to `main`. It records the desired SHA under:

```text
.git/mclone-deploy-after-main-push/
```

`deploy-after-main-push.sh --worker` runs in the background. It waits until the
remote branch actually reports the pushed SHA before deploying, so a rejected or
failed push does not publish. A single-worker lock prevents overlapping deploys.
Quick successive pushes replace the pending SHA before deployment starts.

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

Follow the full worker log:

```bash
tail -f .git/mclone-deploy-after-main-push/deploy.log
```

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

## Tunables

Environment variables:

- `MCLONE_DEPLOY_AFTER_PUSH_BRANCH`: branch to watch, default `main`.
- `MCLONE_DEPLOY_AFTER_PUSH_REMOTE`: default remote for manual scheduling, default `origin`.
- `MCLONE_DEPLOY_AFTER_PUSH_WORKTREE`: deploy worktree path, default `../mclone-deploy-worktree`.
- `MCLONE_DEPLOY_AFTER_PUSH_POLL_SECONDS`: remote polling interval, default `10`.
- `MCLONE_DEPLOY_AFTER_PUSH_SETTLE_SECONDS`: quick-successive-push settle window, default `30`.
- `MCLONE_DEPLOY_AFTER_PUSH_MAX_WAIT_SECONDS`: max wait for remote branch visibility, default `1800`.
