# Native Web

The Rust/WASM web app is served from:

```text
https://mclone.kzahel.com/
```

The live web client is the native Rust/WASM lane under [`../native/apps/mclone-web-client/`](../native/apps/mclone-web-client/). The retired browser engine is not part of the live tree.

## Local Commands

```bash
# Build and serve the Rust/WASM app locally with the headers required for
# SharedArrayBuffer/Web Workers. The command prints the local app URL.
pnpm native:web:serve

# Validate the interactive browser app with Playwright screenshots in /tmp.
pnpm native:web:app-smoke

# Build the exact deploy bundle into dist-native-web/ without uploading.
pnpm native:web:bundle

# Build, upload the native web bundle and asset pack to the mclone R2 bucket,
# deploy the Cloudflare Worker, and make it available at mclone.kzahel.com.
pnpm deploy
```

`pnpm deploy` is an alias for `pnpm native:web:deploy`.

## Deploy Contents

The deploy path packages:

- `native/apps/mclone-web-client/www`
- wasm-bindgen output under `/pkg/`
- `reference/minecraft-1.17.1/extracted.zip`

The Cloudflare Worker in [`../worker/index.js`](../worker/index.js) serves the bundle with COOP/COEP/CORP headers so browser worker and `SharedArrayBuffer` paths can run. Wrangler must be authenticated for the Cloudflare account before deploy.

## Local Post-Push Deploy Hook

For this local checkout, [`../scripts/local-deploy/deploy-after-main-push.sh`](../scripts/local-deploy/deploy-after-main-push.sh) can be installed as a `pre-push` hook:

```bash
./scripts/local-deploy/install-hook.sh
```

The hook returns immediately. A background worker waits until the pushed `main` commit is visible on the remote, then runs `pnpm deploy`.

Quick successive pushes replace the pending SHA before deployment starts. The worker deploys from a reusable sibling worktree, by default `../mclone-deploy-worktree`, which it resets to the pushed commit before running `pnpm deploy`. The active checkout can be edited immediately after pushing.

Local ignored inputs and caches such as `reference/minecraft-1.17.1` and `node_modules` are linked into that deploy worktree when present.

Status is available with:

```bash
./scripts/local-deploy/deploy-after-main-push.sh --status
```

The latest summary, completed deploy, and failed deploy are stored under `.git/mclone-deploy-after-main-push/`, with the full log in `deploy.log`. Completed records include total seconds from hook scheduling to deploy success, deploy command seconds, and the deploy worktree path.

See [`../scripts/local-deploy/README.md`](../scripts/local-deploy/README.md) for the hook implementation notes.
