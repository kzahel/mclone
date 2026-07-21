# Web/WASM

The Rust/WASM web app is served from:

```text
https://mclone.kzahel.com/
```

The live web client is the Rust/WASM target under
[`../native/apps/mclone-web-client/`](../native/apps/mclone-web-client/).

## Runtime Ownership

The production browser uses the same `mclone_scene::McloneSceneHost` policy
owner as desktop, Android, and XR. TypeScript `WebFrameDriver` owns rAF cadence,
DOM input, visibility/resize events, typed promises, resource fetches, and
presentation. The wasm-bindgen `WebSceneHost` wrapper supplies WebGPU targets
and browser implementations of the neutral runtime, compiler, catalog, clock,
deferred-drop, and platform-operation contracts. Local worker, persistent
IndexedDB local-world, and remote WebSocket modes all install services into
that one host.

The web settings-feature profile remains intentionally honest: travel assist,
frame-pipeline overlay, debug diagnostics, and server simulation cadence are
its exact remaining reason-bearing gaps in `mclone-app-runtime`. `Enter Lobby`
is supported. Worker-resident Rust instantiates a fresh transient authored
protected lobby in memory, then opens the most recent compatible catalog world
or a stable catalog-excluded app-private IndexedDB world as its mutable
destination. The browser starts two independent integrated-server Workers,
multiplexes both stable world identities through one compiler Worker, and
submits the destination through the shared scene/render path. The two draw
stores share compatible immutable atlas/pipeline resources.
At the scene and product-policy boundary, TypeScript owns only IndexedDB,
Promise, Worker, input, and presentation mechanics; it does not choose or
author scenario content, decide readiness, place geometry, or activate worlds.
Tactical
[`201-lobby-content-simplification.md`](tactical/201-lobby-content-simplification.md)
records the deletion of the former managed provisioning Worker and TypeScript
workflow. Tactical
[`178-shared-web-lobby-scenario-parity.md`](tactical/178-shared-web-lobby-scenario-parity.md)
records the underlying parity refactor and feature-off/scenario-on performance
evidence. Far LOD is supported through the production shared resident
compiler/cache/render path.

The high-value lower-level Worker ownership campaign is complete. Each Worker
keeps its private Wasm heap and current external `SharedArrayBuffer` mailboxes,
while Rust actors now own server-job dispatch, render coordination and worker
sessions, integrated-server startup and authority policy, and catalog
descriptors. The remaining TypeScript owns browser mechanics such as Worker,
IndexedDB, WebSocket, timer, event-loop, DOM/input, and typed SAB access. This
boundary and its measured stopping decision are recorded in
[`topics/web-worker-runtime-ownership.md`](topics/web-worker-runtime-ownership.md)
and Tactical
[`197-domain-blind-web-worker-broker.md`](tactical/197-domain-blind-web-worker-broker.md).
A new human decision is required before further browser-native long-tail
reduction or one shared Wasm linear memory.

Far LOD uses 4/8/16 rings with two-chunk hysteresis and
replacement-before-suppress transitions. Browser audio and teleport preview
remain explicit absent service capabilities.

Tactical
[`179-composable-world-presentation-and-live-preview-actors.md`](tactical/179-composable-world-presentation-and-live-preview-actors.md)
extends that same product lane with live composed actors. The destination's
authoritative cow/chicken, source-local full body, and an ordinary dedicated
remote player all travel through the shared Rust server, client replica, scene,
and actor renderer. The validation-only auxiliary-player script is shared Rust
server policy enabled by one query/config boolean; TypeScript does not author
positions, inject updates, or decide actor visibility. Desktop, CPU-throttled
mobile, and lifecycle receipts assert stable ids, exact 1:8 movement, walk and
source-light facts, changed WebGPU pixels, activation/return, shutdown, and
relaunch. Browser WebGPU was a per-slice acceptance lane, not a post-native
parity port.

## Local Commands

### Browser capture environment

Run `pnpm host:check` before browser/WebGPU capture on an unfamiliar Linux
shell. On the current Linux host, agent shells commonly omit
`WAYLAND_DISPLAY` while `$XDG_RUNTIME_DIR/wayland-0` is live. The
`native:web:*` browser runner detects that socket and automatically supplies a
headed Wayland Chrome launch, including `--ozone-platform=wayland`; it prints
the selected display before Chrome starts. Plain commands below therefore use
the validated capture lane without copying environment variables by hand.

Confirm the host path independently with:

```bash
pnpm host:check -- --probe-browser-webgpu
```

The probe must report an opaque WebGPU canvas pixel and write an inspectable
screenshot under `/tmp`. Headless Chrome on this machine can return black or
transparent WebGPU captures even when the product renders correctly. Such a
capture is invalid runner evidence, not a renderer failure or a reason to give
up on browser validation. `MCLONE_NATIVE_WEB_FORCE_HEADLESS=1` is available
only for intentional headless diagnostics.

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

# Validate Rust menu-driven world catalog create/open/delete over IndexedDB.
pnpm native:web:catalog-smoke

# Enter the shared protected lobby, wait for its live island preview and
# authoritative creature/remote-player motion, and validate desktop,
# CPU-throttled mobile, activation/persistence, and lifecycle cancellation
# paths. Reports and screenshots stay under /tmp.
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:web:lobby-scenario-lifecycle-smoke

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
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:web:lobby-scenario-lifecycle-smoke

# Build the exact deploy bundle into dist-native-web/ without uploading.
pnpm native:web:bundle

# Build and test the read-only Asset Lab animal catalogue served at /animals/.
pnpm asset-lab:web:build
pnpm asset-lab:web:test

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
- the validated Asset Lab animal catalogue under `/animals/`
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

Local ignored inputs and caches such as `reference/minecraft-1.17.1` and the
root `node_modules` are linked into that deploy worktree when present. The
bundle command installs the separately locked Asset Lab package with
`--frozen-lockfile` when its local dependencies are absent, so a clean deploy
worktree can build `/animals/` without depending on nested ignored files from
the active checkout.

Status is available with:

```bash
./scripts/local-deploy/deploy-after-main-push.sh --status
```

The latest summary, completed deploy, and failed deploy are stored under `.git/mclone-deploy-after-main-push/`, with the full log in `deploy.log`. Completed records include total seconds from hook scheduling to deploy success, deploy command seconds, and the deploy worktree path.

See [`../scripts/local-deploy/README.md`](../scripts/local-deploy/README.md) for the hook implementation notes.
