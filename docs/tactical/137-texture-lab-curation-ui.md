# 137: Texture Lab Curation UI

Status: active; Slice A0 web shell, Zustand store, authored texture index API,
safe image serving, and read-only browser UI landed 2026-07-04. Slice A0.5
repo-local generated output root landed 2026-07-04. Slice A1 generated
candidate discovery landed 2026-07-04. Slice A2 read-only candidate selection
and detail inspection landed 2026-07-04. Next priority is preview generation
parity.

## Purpose

Build a local interactive web UI for `tools/texture-lab` so texture generation
can scale beyond static review sheets.

The UI should let us quickly inspect authored reference textures, generated
diffusion candidates, projection outputs, prompt metadata, block previews, and
temporary texture-pack choices without turning the lab into a giant ad hoc
JavaScript file. It should be a small TypeScript/React/Zustand app backed by
typed texture-lab domain modules and a local Node API server.

Workstream: texture-lab tooling and documentation. First slices should stay
inside `tools/texture-lab/` and should not change native runtime behavior. When
runtime-facing asset-pack selection becomes necessary, route that through the
shared asset/source-chain owners tracked in
[`115-first-party-texture-pack-integration.md`](115-first-party-texture-pack-integration.md).

## Context

The current texture-lab workflow is productive but sheet-bound:

- authored pack source lives in TypeScript modules under
  `tools/texture-lab/packs/mclone-default/`
- review sheets, runtime-compatible exports, projection sheets, and diffusion
  archives default to the repo-local `generated-assets/texture-lab/` root
- diffusion proposals carry useful prompt, seed, strength, hash, projection,
  and codename metadata
- accepted outputs are frozen back into source as deterministic ASCII masks,
  not committed PNGs

The stone and grass experiments exposed the next bottleneck. Prompt wording,
projection palette, tint preview, and candidate selection matter enough that a
human needs fast navigation and side-by-side comparison. Static sheets are still
useful evidence, but they do not scale to a full texture pack or to iterative
candidate tournaments.

## Product Shape

The first durable tool should be a local browser app, not a runtime game UI.
Target workflows:

- browse texture references by structure, material family, block bundle, and
  texture role
- inspect the active authored texture, neutral/tinted export, tile repeat, mip
  strip, terrain patch, and block preview
- compare generated candidates with compact codenames, prompt metadata, seeds,
  strengths, projection metrics, palettes, and archive status
- mark candidates as favorite, rejected, needs prompt iteration, or freeze
  candidate in a local review state
- assemble a temporary pack selection quickly without editing source
- export a temporary runtime-compatible overlay pack for visual validation
- later, launch generation sweeps and freeze accepted candidates through the
  existing provenance-preserving pipeline

The app should optimize for curation speed. Every visible candidate row or card
needs a short codename so the human can say "use G5101S74" or "compare H4106S58
against D4201S58" without copying long paths.

## Non-Goals

- Do not replace the existing CLI export, sheet, analysis, projection, or
  archive commands.
- Do not add a database in the first slice.
- Do not add TanStack Query or another server-state framework in the first
  slices. Zustand async actions and explicit reload/reindex paths are enough
  until background jobs or cache invalidation prove otherwise.
- Do not mutate pack source in the read-only MVP.
- Do not commit generated PNGs, diffusion raw outputs, or temporary preview
  packs.
- Do not feed Mojang textures into diffusion input or train/fine-tune on
  proprietary reference textures.
- Do not build a public hosted service. This is a local authoring tool.
- Do not add native runtime pack-picking UI in this tactical. Temporary pack
  export may be used for local smoke validation, but runtime integration belongs
  to the asset-source workstream.

## Architecture

Use a small TypeScript/React/Vite/Zustand app with a local Node server:

```text
tools/texture-lab/
  src/
    core/
      index-model.ts
      candidate-index.ts
      review-state.ts
      preview-paths.ts
    web-server/
      server.ts
      api.ts
      image-files.ts
    web/
      index.html
      main.tsx
      App.tsx
      components/
      store/
        textureLabStore.ts
        selectors.ts
      styles.css
```

React should own display and interaction only. Zustand should be present from
Slice A and should own UI/session state plus cached API responses:

- selected texture and selected candidate
- filters, grouping, and search
- compare slots
- index loading, reindex status, and stale-artifact warnings
- active temporary pack profile
- favorite/rejected/notes state once Slice C lands

Texture knowledge should live in typed `src/core/` modules that can also be
reused by CLI scripts. The Zustand store may call API helpers and coordinate UI
state, but it should not own candidate parsing, provenance rules, path
normalization, projection policy, or pack-export logic. Components should read
state through selectors rather than pulling the entire store into every panel.

The server should be thin: build indexes, serve allowed images, persist local
review state, and invoke existing lab commands when later slices add actions.

Start with Vite because `tools/asset-lab` already uses it successfully for
local visual tooling. Avoid introducing a full backend framework until the
local API surface proves it needs one.

Likely package scripts:

```json
{
  "web:dev": "tsx src/web-server/server.ts",
  "web:build": "vite build --config src/web/vite.config.ts",
  "web:preview": "vite preview --host 127.0.0.1 --config src/web/vite.config.ts"
}
```

## Output Root Policy

The browser UI should treat generated texture-lab artifacts as first-class local
project data, not disposable operating-system temp files. The default generated
root should be:

```text
generated-assets/texture-lab/
  pack/
  runtime-pack/
  diffusion/
  diffusion-projection/
  diffusion-archive/
  ui-cache/
  review-state.json
```

Add `generated-assets/` to `.gitignore`. Do not put this under `assets/`,
because `assets/` reads as canonical distributable game content while these are
local generated working artifacts.

The default should still be overrideable for experiments and external disks:

```sh
MCLONE_TEXTURE_LAB_OUTPUT_ROOT=/some/path pnpm texture-lab:web
```

All texture-lab CLI defaults, the browser server index, API image allowlist,
diffusion/projection defaults, preview caches, and review state should converge
on the same output-root helper. Validation screenshots can still be written to
`/tmp`.

## Data Model

The UI should build one unified index from checked-in authored source plus
local generated artifacts.

Authoritative sources:

- `tools/texture-lab/packs/mclone-default/texture.ts`
- texture-lab catalog/export metadata under
  `generated-assets/texture-lab/`
- diffusion manifests under `generated-assets/texture-lab/diffusion/**`
- projection summaries and reports under
  `generated-assets/texture-lab/diffusion-projection/**`
- archive manifests under
  `generated-assets/texture-lab/diffusion-archive/**`
- optional local-only reference assets from the existing reference lookup

Suggested core types:

```ts
export interface TextureReferenceEntry {
  textureName: string;
  blockName?: string;
  role: "all" | "top" | "bottom" | "side" | "overlay" | "particle" | "unknown";
  materialFamily?: string;
  structureTags: string[];
  tintRole?: string;
  sourcePath?: string;
  currentExportPath?: string;
  runtimePath?: string;
}

export interface TextureCandidate {
  textureName: string;
  codename: string;
  source: "authored" | "diffusion" | "projection" | "archive";
  candidateId?: string;
  promptPreset?: string;
  prompt?: string;
  negativePrompt?: string;
  seed?: number;
  strength?: number;
  modelId?: string;
  paletteName?: string;
  resolution?: number;
  manifestPath?: string;
  projectionReportPath?: string;
  archivePath?: string;
  metrics?: Record<string, number | string | boolean>;
  images: {
    source?: string;
    tinted?: string;
    tile3x3?: string;
    mipStrip?: string;
    blockPreview?: string;
    terrainPatch?: string;
  };
}

export interface TextureReviewState {
  selectedPackProfile: string;
  byTexture: Record<
    string,
    {
      activeCandidate?: string;
      favorites: string[];
      rejected: string[];
      notes: Record<string, string>;
      tags: Record<string, string[]>;
    }
  >;
}
```

Persist review state in a local JSON file under
`generated-assets/texture-lab/review-state.json` for the first pass. It is
user/session state, not canonical source.

## API Surface

Keep the first API explicit and file-system constrained:

- `GET /api/index`
  - returns texture groups, current authored entries, available candidates,
    review-state summary, and stale-artifact warnings
- `GET /api/textures/:textureName`
  - returns full detail for one texture and its related candidates
- `GET /api/candidates?texture=<name>`
  - returns generated/projection/archive candidates for a texture
- `GET /api/image?path=<encoded-path>`
  - streams images from an allowlist only:
    - repo `tools/texture-lab/`
    - `generated-assets/texture-lab/`
    - the optional `MCLONE_TEXTURE_LAB_OUTPUT_ROOT`
    - local reference assets already permitted by texture-lab reference lookup
- `POST /api/review-state`
  - writes local favorite/reject/active/notes changes
- `POST /api/reindex`
  - rebuilds the in-memory index after new CLI outputs are generated

Future action endpoints:

- `POST /api/preview-pack`
  - writes a temporary selected-candidate runtime-compatible pack under
    `generated-assets/texture-lab/ui-preview-pack/`
- `POST /api/freeze-request`
  - writes a provenance-rich freeze request manifest; first implementation can
    still require an agent to apply the source patch
- `POST /api/generate`
  - launches diffusion/projection jobs through existing scripts and streams job
    status

Path handling must be strict. Never let arbitrary query strings read outside
the allowlist, and never let the UI write into repo source except through a
future explicit freeze flow.

## UI Surfaces

First viewport should be the actual tool, not a landing page:

- Left navigation: material family, block bundle, texture role, tint role,
  status, and tag filters.
- Center candidate grid: compact codename, current/favorite/rejected markers,
  source/tinted/tile/mip thumbnails, score or status, and quick compare.
- Right inspector: selected candidate prompt, negative prompt, seed, strength,
  model id, palette/projection settings, archive state, source paths, and notes.
- Preview strip: authored source, runtime export, 3x3 tile, mip strip, block
  preview, terrain patch, and optional local reference image.
- Pack profile control: temporary active candidate choices per texture, with
  export path and validation hints when overlay generation is added.

Use dense, utilitarian UI. This is a production tool for scanning many small
images; avoid large marketing-style panels and decorative layouts.

## Implementation Slices

### Slice A0 - Web Shell And Authored Texture Index

Status: landed 2026-07-04.

Add the Vite/React shell, local server, typed authored-texture index builder,
image serving, and first UI without any source mutation.

Deliverables:

- React/Vite/Zustand dependencies and `web:dev`, `web:build`, `web:preview`
  scripts.
- Root `texture-lab:web` and `texture-lab:web:build` wrappers.
- Framework-free `src/core/` index model and authored texture index builder.
- Local Node/Vite server with `/api/index`, `/api/textures/:name`,
  `/api/reindex`, and allowlisted `/api/image`.
- React UI with sidebar filters, texture list, current/runtime/sheet image
  panels, and inspector.
- Zustand store for index loading, selected texture, filters, reindex action,
  and selector-based component reads.
- Missing-artifact states that point to the relevant CLI commands.

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
pnpm --dir tools/texture-lab web:build
pnpm --dir tools/texture-lab web:dev
```

Additional validation:

- `/api/index` returned `mclone-default` with 81 textures and 9 blocks.
- `/api/image` served an exported texture through the allowlisted endpoint.
- Playwright loaded `http://127.0.0.1:5177/`, found 81 texture rows, reported
  no browser console errors, and wrote
  `/tmp/mclone-texture-lab-ui-a0.png`.

### Slice A0.5 - Repo-Local Generated Output Root

Status: landed 2026-07-04.

Move texture-lab generated outputs from `/tmp/mclone-texture-lab/` to the
repo-local gitignored `generated-assets/texture-lab/` default before expanding
candidate discovery.

Deliverables:

- add `generated-assets/` to `.gitignore`
- add a shared output-root helper that defaults to
  `generated-assets/texture-lab/`
- support `MCLONE_TEXTURE_LAB_OUTPUT_ROOT` as an override
- route `export`, `catalog`, `project-diffusion`, the browser index, and image
  allowlist through that helper
- leave existing archived provenance comments alone; historical paths in source
  comments are evidence for already-frozen candidates
- update texture-lab docs after the implementation, avoiding stale `/tmp`
  command examples

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
pnpm --dir tools/texture-lab export
pnpm --dir tools/texture-lab web:build
pnpm --dir tools/texture-lab web:dev
```

Additional validation:

- `python3 -m py_compile tools/texture-lab/diffusion/propose.py tools/minecraft_assets/overlay_pack.py`
- `pnpm --dir tools/texture-lab catalog` wrote catalog outputs under
  `generated-assets/texture-lab/`
- `pnpm texture-lab:pack-overlay` wrote
  `generated-assets/texture-lab/mclone-default-overlay.pbp`
- `/api/index` reported output root
  `/Users/kgraehl/code/mclone/generated-assets/texture-lab`, 81 textures, 9
  blocks, 81 current exports, and 81 sheets
- `/api/image` served an exported texture from `generated-assets/texture-lab/`
- Playwright loaded `http://127.0.0.1:5177/`, found 81 texture rows, reported
  no browser console errors, and wrote
  `/tmp/mclone-texture-lab-ui-a05.png`

### Slice A1 - Generated Candidate Discovery

Status: landed 2026-07-04.

Extend the read-only UI from authored textures to generated candidate artifacts.

Deliverables:

- discover diffusion manifests under
  `generated-assets/texture-lab/diffusion/**`
- discover projection summaries and reports under
  `generated-assets/texture-lab/diffusion-projection/**`
- discover archive manifests under
  `generated-assets/texture-lab/diffusion-archive/**`
- normalize generated outputs into `TextureCandidate` records with codename,
  source type, prompt, negative prompt, seed, strength, projection palette,
  resolution, triage status/score/reasons, archive status, and image paths
- associate candidate records with target texture names from projection/archive
  metadata, authored texture names in source paths, or historical prompt aliases
- show generated candidate cards beside the authored/current texture panels
- keep unknown or unassociated artifacts visible through index warnings and
  `/api/candidates` diagnostics

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
pnpm --dir tools/texture-lab web:build
pnpm --dir tools/texture-lab web:dev
```

Additional validation:

- copied a representative ignored grass/stone candidate subset from the old
  `/tmp/mclone-texture-lab/` output into
  `generated-assets/texture-lab/{diffusion,diffusion-projection,diffusion-archive}/`
  for local validation
- `/api/index` reported output root
  `/Users/kgraehl/code/mclone/generated-assets/texture-lab`, 79 generated
  candidates, 79 associated candidates, and 2 archived candidates
- `/api/candidates?texture=grass_block_top` returned 14 candidates with compact
  codenames such as `G5101S74`
- `/api/candidates?texture=stone` returned 65 candidates with compact codenames
  such as `H4101S58`
- `/api/image` served a generated candidate PNG from
  `generated-assets/texture-lab/` with HTTP 200
- headless Chrome loaded `http://127.0.0.1:5177/`, selected
  `grass_block_top`, found 14 candidate cards, and wrote
  `/tmp/mclone-texture-lab-ui-a1.png`; the only browser log event was the
  unrelated missing `favicon.ico`

### Slice A2 - Read-Only Candidate Selection And Detail

Status: landed 2026-07-04.

Add session-local selected-candidate state and expose candidate provenance in
the inspector without writing review state or mutating pack source.

Deliverables:

- add `selectedCandidateId` and `selectCandidate` to the Zustand store
- preserve selected candidate across reload/reindex when it still belongs to
  the selected texture
- default texture selection to the first associated candidate when present
- make candidate cards clickable, keyboard-focusable buttons with selected and
  `aria-pressed` state
- show the selected candidate's codename, source, status, score, seed,
  strength, resolution, archive state, prompt, negative prompt, model,
  scheduler, steps, projection palette swatches, triage reasons, and artifact
  paths in the right inspector
- keep the flow read-only; do not write `review-state.json` yet

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
pnpm --dir tools/texture-lab web:build
pnpm --dir tools/texture-lab web:dev
```

Additional validation:

- `/api/index` still reported 79 generated candidates and 79 associated
  candidates
- `/api/candidates?texture=grass_block_top` still returned 14 candidates
- headless Chrome loaded `http://127.0.0.1:5177/`, selected
  `grass_block_top`, clicked the `G5101S74` archive card, found exactly one
  selected/pressed candidate card, verified the inspector contained
  `G5101S74`, `grass-top-tufts`, archive artifact paths, and palette swatches,
  and wrote `/tmp/mclone-texture-lab-ui-a2.png`
- the browser smoke reported no non-favicon browser log events

### Slice B - Preview Generation Parity

Move the reusable preview generation pieces behind typed helpers so the UI can
show the same evidence as static sheets without hand-copying sheet code into
React.

Deliverables:

- cache preview artifacts under `generated-assets/texture-lab/ui-cache/`
- render source, tinted, 3x3 tile, mip strip, block preview, terrain patch, and
  optional local reference panels on demand
- reuse existing tint, projection, image, and review-sheet helpers where
  possible
- add stale-cache detection from source hash, candidate hash, projection
  settings, and preview kind

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
pnpm --dir tools/texture-lab web:build
```

Visually compare at least one UI-generated preview set against an existing
static review sheet.

### Slice C - Review State And Temporary Pack Profiles

Let the human curate without mutating source.

Deliverables:

- persist favorite/rejected/active/notes/tags in
  `generated-assets/texture-lab/review-state.json`
- add keyboard and click flows for quick candidate marking
- add temporary pack profiles so selected candidates can be reviewed together
- generate a temporary runtime-compatible pack from the selected profile under
  `generated-assets/texture-lab/ui-preview-pack/`
- optionally pack that output as a first-party overlay `.pbp`

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
pnpm --dir tools/texture-lab web:build
pnpm texture-lab:runtime-compat
pnpm texture-lab:pack-overlay
```

If a preview overlay is generated, run the existing native visual smoke path
from the first-party texture-pack tactical before accepting the slice.

### Slice D - Freeze Requests

Make accepted candidates durable, but keep source mutation explicit.

Deliverables:

- add a freeze request manifest containing texture name, codename, candidate id,
  raw/projection/archive paths, hashes, prompt metadata, palette, resolution,
  target source file, and intended source symbol mapping
- allow the UI to create a freeze request only for archived or archivable
  candidates
- keep the first source patch agent-mediated or CLI-mediated
- require the same deterministic projection and archive checks already used by
  `project-diffusion`
- do not auto-commit from the UI

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
pnpm --dir tools/texture-lab project-diffusion -- --help
git diff --check
```

The accepted source patch must include provenance comments when freezing a
diffusion candidate.

### Slice E - Generation Launcher

Only after curation and freeze requests work, let the UI launch experiments.

Deliverables:

- prompt preset editor backed by checked-in defaults plus local scratch edits
- seed/strength/palette/resolution sweep controls
- job queue that invokes the existing Python diffusion runner and TypeScript
  projection command
- live status, logs, generated contact sheets, projection sheets, and index
  refresh
- failed-job artifacts preserved under `/tmp` for debugging

Validation:

```sh
pnpm --dir tools/texture-lab typecheck
python3 -m py_compile tools/texture-lab/diffusion/propose.py
```

Run one tiny smoke generation before accepting the slice.

## Acceptance For Slice A

Slice A is complete when:

- `pnpm --dir tools/texture-lab web:dev` opens a local app
- the app lists authored `mclone-default` textures
- existing diffusion/projection/archive outputs in
  `generated-assets/texture-lab/` appear as candidates when present
- candidate cards show codenames and core provenance fields
- selecting a texture shows a candidate detail inspector
- texture selection, candidate selection, filters, and loading status flow
  through the Zustand store with selector-based component reads
- images are served only through the allowlisted image endpoint
- the app handles an empty `generated-assets/texture-lab/` gracefully
- `typecheck`, `web:build`, and a browser screenshot validation pass

## Risks

- Candidate count can grow quickly. Add virtualization only when the grid proves
  too slow, but keep the data model ready for pagination/filtering.
- Generated artifacts are local working data, not source. The UI should show
  missing, stale, and archived states clearly instead of silently hiding
  history.
- File serving can become a security footgun. Use resolved absolute paths and a
  narrow allowlist.
- Source mutation from a browser button is risky. Keep freeze as an explicit
  later slice and do not auto-commit from the app.
- Zustand can become a dumping ground if boundaries are loose. Keep domain
  parsing, provenance, projection, and export rules in `src/core/`; keep the
  store focused on UI/session state and API coordination.
- Metrics are helpful but not authoritative. The UI should privilege visual
  comparison and human selection over numeric auto-ranking.
- Palette/projection choices are texture-specific. Do not bake grass or stone
  assumptions into generic candidate indexing.

## Open Questions

- Should review state live only in `/tmp`, or should selected curation state be
  optionally exportable to a checked-in metadata file once the workflow settles?
- Should the first block preview be the existing software cube, or should the UI
  immediately add a small Three.js scene for rotatable inspection?
- How much of the reference texture organization should be hand-authored tags
  versus inferred from pack/block/role naming?
- Should freeze source patches be generated by a CLI command first, with the UI
  only writing freeze request manifests?

## Immediate Next Step

Implement Slice B preview generation parity:

1. factor reusable preview generation helpers behind typed `src/core/` APIs
2. cache UI preview artifacts under `generated-assets/texture-lab/ui-cache/`
3. show candidate/source/tinted/tile/mip/block/terrain evidence on demand
4. detect stale preview cache entries from source and candidate hashes
5. visually compare at least one UI-generated preview set against an existing
   static review sheet
