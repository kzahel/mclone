# Native Web

The Rust/WASM web app is served from:

```text
https://mclone.kzahel.com/
```

The live web client is the native Rust/WASM lane under [`../native/apps/mclone-web-client/`](../native/apps/mclone-web-client/). The retired browser engine is not part of the live tree.

## Runtime Ownership

The production browser uses the same `mclone_scene::McloneSceneHost` policy
owner as desktop, Android, and XR. TypeScript `WebFrameDriver` owns rAF cadence,
DOM input, visibility/resize events, typed promises, resource fetches, and
presentation. The wasm-bindgen `WebSceneHost` wrapper supplies WebGPU targets
and browser implementations of the neutral runtime, compiler, catalog, clock,
deferred-drop, and platform-operation contracts. Local worker, persistent
IndexedDB local-world, and remote WebSocket modes all install services into
that one host.

The web feature profile remains intentionally honest: travel assist,
frame-pipeline overlay, debug diagnostics, and server simulation cadence are
the exact remaining reason-bearing gaps in `mclone-app-runtime`. Far LOD is
supported through the production shared resident compiler/cache/render path.
That path now uses 4/8/16 rings with two-chunk hysteresis and
replacement-before-suppress transitions. Browser audio and teleport preview
remain explicit absent service capabilities.

## Local Commands

```bash
# Build and serve the Rust/WASM app locally with the headers required for
# SharedArrayBuffer/Web Workers. The command prints the local app URL.
pnpm native:web:serve

# Validate the interactive browser app with Playwright screenshots in /tmp.
pnpm native:web:app-smoke

# Apply the authored-plus-generated logical selection through the shared host,
# reinitialize the resident compiler at the same epoch, and capture the native
# Asset Packs screen under /tmp.
pnpm native:web:asset-pack-smoke

# Toggle Far LOD through the production Graphics UI; prove resident shared-
# result compilation, complete 4/8/16 coverage, and a three-chunk hysteretic
# movement transition; compare inspected off/on/moved captures in every mode.
pnpm native:web:far-lod-smoke
pnpm native:web:far-lod-indexeddb-smoke
pnpm native:web:far-lod-remote-smoke

# Validate native Rust menu-driven world catalog create/open/delete over IndexedDB.
pnpm native:web:catalog-smoke

# Run the complete production host-mode and behavior matrix used for scene-host
# changes. Captures and logs remain under /tmp.
pnpm native:web:smoke
pnpm native:web:thread-smoke
pnpm native:web:canvas-smoke
pnpm native:web:chunk-smoke
pnpm native:web:app-smoke
pnpm native:web:catalog-smoke
pnpm native:web:mobile-smoke
pnpm native:web:block-edit-probe
pnpm native:web:movement-perf
pnpm native:web:remote-smoke

# Build the exact deploy bundle into dist-native-web/ without uploading.
pnpm native:web:bundle

# Build, upload the native web bundle and asset pack to the mclone R2 bucket,
# deploy the Cloudflare Worker, and make it available at mclone.kzahel.com.
pnpm deploy
```

`native:web:mobile-smoke` runs with browser CPU throttling and locks startup
admission: the camera must remain at its held spawn height until the underfoot
interest chunk and drawable view are ready, then settle onto loaded ground.

`pnpm deploy` is an alias for `pnpm native:web:deploy`.

## Deploy Contents

The deploy path packages:

- `native/apps/mclone-web-client/www`
- wasm-bindgen output under `/pkg/`
- `reference/minecraft-1.17.1/extracted.zip`
- deterministic authored/fallback packs and sidecars under
  `/first-party-packs/`

The bundle command regenerates the two first-party packs before staging them.
The browser fetches all three payloads, while shared Rust owns selection order,
fallback policy, preparation, and the transactional asset epoch.

The active logical selection persists in localStorage key
`mclone.assetPacks.v1`. `native:web:asset-pack-smoke` clears that key, applies
Original, reloads the page, and requires the shared scene resources and
resident compiler to restore at the same epoch with zero resolved
Minecraft/unknown provenance. Runtime reports also expose reload timing and
estimated peak retained payload bytes.

The current browser still fetches the reference archive for epoch-0 bootstrap
even when a persisted first-party selection is restored immediately afterward.
“Proprietary-free” describes the active resolution ledger, not an assertion
that reference bytes were absent from the deploy or network bootstrap.

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
