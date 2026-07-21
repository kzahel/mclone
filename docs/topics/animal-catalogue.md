# Deployed Animal Catalogue

Topic: `animal-catalogue`

Status: implementation and local acceptance complete 2026-07-21; production
push and live-site verification remain. The target is a read-only,
production-built Asset Lab catalogue at
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

## Static Build Contract

The production build executes canonical TypeScript sources only on the build
host. It writes a versioned catalogue manifest plus validated semantic JSON and
thumbnail artifacts beneath the Asset Lab web distribution directory. The
deployed browser fetches JSON; it never dynamically imports `figure.ts`.

The manifest records at least:

- stable figure name and artifact URL;
- semantic SHA-256 and byte length;
- part, material, texture, and clip counts;
- clip names, duration, loop state, authored fps, and locomotion kind; and
- a deterministic default clip and thumbnail URL.

Generation fails on duplicate names, invalid JSON round trips, non-box
canonical parts, missing clips, or unsafe output names. The source directory is
the inventory authority. `ANIMALS.md` remains a human roadmap whose cross-listed
rows and narrative summaries are not parsed as deployed data.

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
  keyboard access, responsive layout, and browser/page errors;
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
5. [ ] Push `main`, monitor the local deploy worker, and validate `/animals/`
   live.

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

The current application JavaScript is about 745 KB minified and 200 KB gzip,
dominated by the one Three.js viewport plus React. Figure JSON is fetched lazily
per selection and catalogue thumbnails use native lazy loading. Further code
splitting is measurement-gated rather than required for first publication.

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

Push `main`, let the existing deployment hook publish the same bundle, and run
the live HTTP/browser acceptance receipt. Then mark this topic complete with
the deployed commit and live evidence. Reconsider a workspace package only
when a consumer outside `tools/asset-lab` needs the TypeScript viewer contract.
