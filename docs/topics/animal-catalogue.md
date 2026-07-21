# Deployed Animal Catalogue

Topic: `animal-catalogue`

Status: complete and live-accepted 2026-07-21, including runtime promotion
metadata and filtering. The read-only, production-built Asset Lab catalogue is
available at
`https://mclone.kzahel.com/animals/`, delivered by the existing native-web
bundle and after-main-push deployment path.

## Scope

This topic owns the browser catalogue for canonical Asset Lab figures: static
catalogue generation, the interactive semantic Three.js viewport, catalogue
navigation and animation controls, subpath-safe production output, deployment,
and live-site validation.

It does not promote examples into the runtime asset pack, move figure policy
into the browser, expose Asset Lab authoring mutations publicly, or make the
Three.js preview an engine dependency. Runtime promotion and shared Rust figure
preparation remain owned by
[`compiled-figure-rendering.md`](compiled-figure-rendering.md).

## Product Contract

- The public entry point is `/animals/` on the existing mclone site.
- The catalogue is read-only and contains canonical box-only sources discovered
  from `tools/asset-lab/examples/*/figure.ts` at build time. Deprecated rounded
  compatibility sources under `legacy-examples/` are excluded.
- Visitors can search and select figures, inspect semantic facts, choose an
  authored clip, play or pause continuous animation, scrub presentation time,
  adjust playback speed, orbit, pan, zoom, reset framing, and choose standard
  camera views.
- Clip selectors group locomotion, idle, and action roles. Named action buttons
  restart one-shot clips, hold their final pose when no continuation is
  authored, and automatically play an optional `nextClip` when they complete.
- Visitors can distinguish runtime-promoted figures from Asset Lab-only
  examples, filter by that status, and inspect the promoted runtime figure ID
  and packed semantic JSON path.
- Figure selection and clip selection are shareable through URL query
  parameters without requiring a Worker-side single-page-app fallback.
- The layout is responsive and keyboard accessible, supports light and dark
  presentation, and retains one WebGL viewport rather than allocating one
  renderer per catalogue row.
- Catalogue rows may use deterministic static thumbnails generated through the
  same semantic renderer. They must not create a grid of live WebGL contexts.

## Shared-Code Boundary

The deployed catalogue is another Asset Lab display path, not a second figure
implementation:

```text
examples/<name>/figure.ts
  -> existing serialize/reparse validation boundary
  -> generated canonical figure JSON
  -> shared Three.js figure scene and viewport controller
       -> existing development preview
       -> deployed React catalogue
       -> catalogue thumbnail capture
```

The browser-neutral semantic contract owns `FigureAsset`, validation,
serialization, clip facts, and animation semantics. The shared Three.js layer
owns semantic geometry, materials, texture construction, scene pose updates,
camera framing, controls, resize behavior, lighting, and GPU disposal. React
owns only catalogue/UI state and delegates rendering through the viewport
controller.

The current development preview must become a thin client of that same
controller. React components must not create figure meshes, reinterpret clip
keys, calculate semantic pivots, or maintain a parallel animation evaluator.
Native Rust continues sharing semantic JSON rather than TypeScript or Three.js.

Runtime promotion metadata is not maintained in React. Static generation
derives it from `src/first-party-figures.ts`, the same checked mapping that
owns promoted TypeScript sources and generated runtime JSON. The deployed
manifest carries the result, while the Rust registry remains the runtime
consumer of those checked files.

## Static Build Contract

The production build executes canonical TypeScript sources only on the build
host. It writes a versioned catalogue manifest plus validated semantic JSON and
thumbnail artifacts beneath the Asset Lab web distribution directory. The
deployed browser fetches JSON; it never dynamically imports `figure.ts`.

The manifest records at least:

- stable figure name and artifact URL;
- semantic SHA-256 and byte length;
- part, material, texture, and clip counts;
- clip names, labels, roles, duration, loop state, authored fps, locomotion
  kind, and optional next-clip behavior;
- a deterministic default clip and thumbnail URL;
- the total runtime-promoted figure count; and
- for promoted entries, the runtime figure ID and packed semantic JSON path.

Generation fails on duplicate names, invalid JSON round trips, non-box
canonical parts, missing clips, unsafe output names, duplicate promotion IDs or
paths, unsafe runtime paths, or promotion-summary drift. The source directory
is the inventory authority. `ANIMALS.md` remains a human roadmap whose
cross-listed rows and narrative summaries are not parsed as deployed data.

## Deployment Contract

`pnpm asset-lab:web:build` produces the exact `/animals/` subtree. The root
`native:web:bundle` command builds it and copies it into
`dist-native-web/animals/` before R2 upload. Vite uses `/animals/` as its base,
and all catalogue/data URLs derive from that base rather than root-relative
development paths.

The Cloudflare Worker continues serving directory paths as `index.html` and
supplying the site's isolation headers. Its immutable-cache recognition must
cover hashed Vite assets nested below `/animals/`; HTML and stable catalogue
metadata must not remain stale across a deploy.

The existing local after-main-push hook remains the deployment coordinator.
No separate hosted workflow, bucket, or site is introduced.

## Validation And Acceptance

Required gates are:

- strict Asset Lab TypeScript with `noUncheckedIndexedAccess` and
  `exactOptionalPropertyTypes`;
- semantic unit tests and first-party figure drift checks;
- deterministic catalogue-generation tests covering discovery, hashes,
  summaries, canonical-only admission, and invalid-source rejection;
- a production `/animals/` build served from its actual subpath;
- Playwright coverage for initial load, search and selection, URL restoration,
  play/pause/scrub/speed controls, camera actions, figure replacement, theme,
  runtime-status filtering and details, keyboard access, responsive layout,
  and browser/page errors;
- inspected local desktop and mobile screenshots written under `/tmp`;
- deploy-bundle inspection proving `dist-native-web/animals/index.html`, hashed
  app assets, manifest, figure JSON, and thumbnails are present; and
- post-push live HTTP and browser validation at the public URL.

The live acceptance receipt must identify the deployed commit and prove that a
non-default figure loads, animation time advances and can be paused/scrubbed,
camera interaction changes the rendered view, and no console or page errors
occur.

## Implementation Slices

1. [x] Extract a reusable viewport/controller around the existing scene
   renderer, add correct bounds-based framing and disposal, and move the
   development preview onto it.
2. [x] Add deterministic static catalogue generation and thumbnails from
   canonical semantic JSON.
3. [x] Add the React catalogue shell and responsive, accessible interaction
   model.
4. [x] Add production-subpath tests, bundle/deploy integration, and
   documentation.
5. [x] Push `main`, monitor the local deploy worker, and validate `/animals/`
   live.
6. [x] Derive runtime promotion metadata from the checked first-party registry,
   validate it in the catalogue contract, and display summary, filter, badge,
   and inspector states.
7. [x] Publish and live-verify the runtime promotion follow-up.
8. [x] Add authored defaults and action-aware grouped controls, completion,
   filtering, inspector facts, and shareable selection.

## Local Acceptance Evidence

The implemented path passes:

- `pnpm asset-lab:typecheck`;
- `pnpm asset-lab:test`: 16 semantic, scene, discovery, hash, and generation
  tests plus the first-party figure drift gate;
- `pnpm asset-lab:web:test`: desktop and 390px production-subpath Playwright
  lanes covering deep links, animation advance/pause/keyboard scrub, camera
  pixel change, search, keyboard selection, figure replacement, one-canvas
  ownership, theme, responsive width, and browser/page errors;
- `pnpm native:web:worker:test`: directory routing, isolation headers, nested
  hashed-asset immutable caching, and catalogue-data revalidation; and
- `pnpm native:web:bundle`: complete asset packs, WASM, web glue, catalogue
  generation, and final `dist-native-web/animals/` staging.

The desktop dark-mode and 390px mobile screenshots at
`/tmp/mclone-animal-catalogue-desktop.png` and
`/tmp/mclone-animal-catalogue-mobile.png` were inspected. Figure framing,
textures, floor contact, selected-row visibility, controls, catalogue layout,
inspector, and mobile stacking are coherent.

The final bundle inspection re-hashed every staged semantic figure, required
every thumbnail, confirmed hashed JS/CSS references use `/animals/assets/`,
and found one manifest, one JSON and one PNG per local canonical source. The
local inventory can include uncommitted sources; production inventory is
deliberately generated from the pushed deploy worktree.

The current application JavaScript is about 748 KB minified and 201 KB gzip,
dominated by the one Three.js viewport plus React. Figure JSON is fetched lazily
per selection and catalogue thumbnails use native lazy loading. Further code
splitting is measurement-gated rather than required for first publication.

The runtime-promotion follow-up retains all local gates. Its deterministic
catalogue test proves the checked Chicken mapping, promoted-count consistency,
and unsafe runtime-path rejection. Production-subpath Playwright proves the
three-entry runtime filter, all three promoted names, exclusion by the Asset
Lab-only filter, row metadata, the Chicken runtime ID and packed path, and the
existing one-canvas interaction contract. The focused promoted capture at
`/tmp/mclone-animal-catalogue-runtime-filter.png` was inspected.

The animation-action follow-up retains the existing gates and adds semantic
default/label/role/`nextClip` validation to the generated manifest. Local
production output contains 104 figures, 109 clips, and 2,141 parts. Playwright
proves grouped Roly-poly clips, the Special actions filter, repeated action
restart, non-looping final-pose hold, automatic return from `unroll` to
`crawl`, URL synchronization, and clean browser/page errors. The held action
capture at `/tmp/mclone-roly-poly-action-catalogue.png` was inspected.

## Live Acceptance Evidence

Commit `464ef78f30a340d10d4d79e04d12c10eec5b7958` deployed through the clean
after-main-push worktree on 2026-07-21 as Cloudflare Worker version
`f4259d04-de58-4185-b6dc-5ef9c32b60d3f`. The production receipt established:

- `/animals/`, its manifest, the selected semantic JSON, and its nested hashed
  assets all return HTTP 200 with the expected content types and site isolation
  headers;
- HTML and stable catalogue data revalidate, while
  `/animals/assets/index-C5yxnj2Q.js` is immutable for one year;
- manifest SHA-256
  `cd8a90a3810debc762b5fec87f345f172ab3e85ddac1d1943f8d7e25f27e8d81`
  covers 92 figures, 92 clips, and 1,808 parts;
- live King Cobra JSON is 46,993 bytes and re-hashes to its advertised
  `9cdf6c02bb4c0cc2bb5d70cd3695b2e51f4aa942ad22b984996e5eb600b005db`;
- the `king_cobra` / `slither` deep link restored, animation advanced, pause
  held exact time, keyboard scrubbing reached authored values, and preset,
  pointer-orbit, and pointer-pan actions each changed rendered pixels;
- search replaced King Cobra with Owl while retaining exactly one canvas, a
  390px viewport had no horizontal overflow, and all observed requests,
  browser console events, and page errors were clean; and
- `/tmp/mclone-animal-catalogue-live.png` was inspected after the live camera
  interactions.

The first clean publication also proved and repaired a deploy prerequisite:
the root worktree dependency link does not contain Asset Lab's independently
locked Vite package. The bundle now installs that package with a frozen
lockfile when absent, and the successful production build ran from the clean
deploy worktree rather than the active checkout.

### Runtime Promotion Follow-up

Commit `cd40e3eb212481b6d0bc019bbaae4cc65a357ebe` deployed through the clean
worktree on 2026-07-21 as Cloudflare Worker version
`81c9da3d-5b92-41b4-a672-5bea6dec5950`. The clean aggregate bundle passed,
including the native web-glue build, 95-figure catalogue generation, and new
hashed assets `index-9j2fmTha.js` and `index-DdxD-7mf.css`.

The live manifest revalidated with SHA-256
`feb640c947b6f6806493996669e4bb72c2883e4a82efff593d046f226e3bef5b` and
reported 95 figures, 95 clips, 1,881 parts, and three runtime promotions. Those
records are exactly:

- `chicken` → `mclone:chicken` →
  `assets/mclone/figures/chicken.figure.json`;
- `player` → `mclone:player` →
  `assets/mclone/figures/player.figure.json`; and
- `upright_bear` → `mclone:upright_bear` →
  `assets/mclone/figures/upright_bear.figure.json`.

Live Playwright acceptance proved the runtime summary, the exact three-row
Runtime-only result, all three badges, Chicken's displayed ID and packed path,
Chicken animation advance, Chicken exclusion from the Lab-only filter, King
Cobra's Asset Lab-only inspector state, one-canvas ownership, and 390px layout
without horizontal overflow. No failed requests, console errors, or page
errors occurred. The fully loaded production capture at
`/tmp/mclone-animal-catalogue-promotion-live-loaded.png` was inspected.

## Code And Documentation Map

- Asset Lab contract and commands:
  [`../../tools/asset-lab/README.md`](../../tools/asset-lab/README.md)
- Semantic JSON boundary:
  [`../../tools/asset-lab/src/figure-json.ts`](../../tools/asset-lab/src/figure-json.ts)
- Current semantic Three.js scene:
  [`../../tools/asset-lab/src/scene.ts`](../../tools/asset-lab/src/scene.ts)
- Current development preview:
  [`../../tools/asset-lab/src/preview.ts`](../../tools/asset-lab/src/preview.ts)
- Existing web bundle/deploy path:
  [`../../scripts/deploy-native-web.sh`](../../scripts/deploy-native-web.sh)
- Worker routing and cache headers:
  [`../../worker/index.js`](../../worker/index.js)
- Web deployment operating notes:
  [`../native-web.md`](../native-web.md)

## Known Risks

- Repeated figure switching leaks GPU resources unless geometry, cloned
  materials, canvas textures, controls, observers, and animation frames are
  disposed deliberately.
- A fixed floor or camera distance frames small, tall, or wide figures poorly;
  framing must derive from semantic bounds and be recomputed after selection.
- Vite development imports can hide a broken static build. Acceptance must use
  generated JSON from the production output at the real `/animals/` base.
- Build-time thumbnails can inflate deploy time if each starts a browser or
  server. One browser session and one reusable capture page should process the
  catalogue deterministically.
- The checkout can contain uncommitted examples that are absent from the pushed
  deploy worktree. Live inventory is defined by the pushed commit, not by a
  local pre-push count.

## Recommended Next Work

No acceptance work remains. Reconsider a workspace package only when a
consumer outside `tools/asset-lab` needs the TypeScript viewer contract. Treat
further bundle splitting or upload batching as measurement-led deployment
improvements rather than catalogue correctness work.
