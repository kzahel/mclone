# 111 - AI Figure Asset Lab

Status: active; Slice 2 locomotion review metadata landed.

## Purpose

Create a disposable, fast-iteration asset lab for AI-authored blocky character
and creature figures before committing any runtime entity/model contract to the
native engine.

The lab should make it cheap for an agent or human to generate, preview,
validate, and revise simple Minecraft-adjacent models such as pigs, sheep, dogs,
cats, butterflies, and player stand-ins. The output contract is a small
engine-friendly figure asset, not arbitrary Three.js code.

## Workstream

This is an isolated tooling experiment under `tools/asset-lab/`.

It is not the native runtime entity system. Do not put generated figures,
preview-specific Three.js code, or asset-authoring helpers into
`mclone-native-client`. If a later slice promotes the exported format into the
engine, add a shared owner first, likely under `mclone-assets`,
`mclone-render-session`, or a small shared figure/model crate.

## Core Direction

Use TypeScript as the authoring surface:

- asset files get type-checked DSL calls and editor completion
- agents get a constrained vocabulary instead of free-form renderer code
- generated files stay small and reviewable
- runtime validation still checks the exported figure data

Use Three.js only for the lab preview and screenshot path. Three.js is familiar
to agents and is good for quick iteration, but it is not the source-of-truth
runtime contract.

Use Playwright for visual smoke capture. It drives the same browser preview a
human sees, can write deterministic screenshots to `/tmp`, and can later grow
pixel probes, animation sheets, mobile-sized captures, and side-by-side diffs.

## Target Shape

Authoring file:

```text
tools/asset-lab/examples/piglet/figure.ts
```

The authoring DSL records a figure asset while the preview layer turns the same
data into Three.js objects:

```ts
export default figure("piglet", ({ mat, asciiTexture, part, box, capsule, clip }) => {
  mat("skin", "#d88a92");

  asciiTexture("face", {
    palette: {
      ".": "#d88a92",
      "e": "#261718",
      "n": "#a85b65",
    },
    pixels: [
      "........",
      "..e..e..",
      "........",
      "...nn...",
      "...nn...",
      "........",
    ],
  });

  part("body", box({ size: [1.2, 0.7, 0.8], material: "skin" }));
  part("leg_fl", capsule({
    parent: "body",
    at: [-0.42, -0.5, -0.28],
    radius: 0.1,
    length: 0.45,
    material: "skin",
  }));

  clip("walk", {
    fps: 12,
    loop: true,
    keys: [
      ["leg_fl", 0.0, { rot: [25, 0, 0] }],
      ["leg_fl", 0.5, { rot: [-25, 0, 0] }],
      ["leg_fl", 1.0, { rot: [25, 0, 0] }],
    ],
  });
});
```

Exported contract:

```text
/tmp/mclone-asset-lab/piglet/figure.json
```

The exported asset should contain only simple data:

- schema version
- named materials
- ASCII palette textures
- named primitive parts
- per-face box material/texture overrides
- parent relationships and local transforms
- optional joint/pivot metadata, with `joint.pivot` measured from the part's
  unrotated local center
- simple animation clips generated either by raw keyframes, procedural
  `walkCycle` tracks such as `swing` and `bob`, or gait macros such as
  `quadrupedWalk`, `bipedWalk`, and `wingFlap`
- optional clip locomotion metadata: cycle distance, speed, forward direction,
  and stance/contact windows for gait review and future runtime import

## Initial Primitive Set

Start intentionally small:

- `box`
- `sphere`
- `capsule`
- `cylinder`

This is enough for blocky Minecraft-style figures while allowing softer toy-like
forms. Avoid free-form triangle meshes, imported glTF, skinning, IK, and
per-asset custom renderer code in the first phase.

## ASCII Texture Policy

ASCII textures are first-class source data:

- a texture is a fixed-width list of strings
- each character maps to a palette color
- validation fails on ragged rows or unknown palette characters
- the preview creates nearest-filtered canvas textures from the ASCII source

This keeps AI-generated textures compact, diffable, and easy to regenerate.
PNG generation can be added as a derived export later, but the ASCII source
should remain the editable form.

## Visual Smoke Loop

Required first commands:

```powershell
pnpm asset-lab:install
pnpm asset-lab:typecheck
pnpm asset-lab:export
pnpm asset-lab:smoke
pnpm asset-lab:sheet
pnpm asset-lab:video
pnpm asset-lab:preview
```

The default smoke writes to `/tmp`, for example:

```text
/tmp/mclone-asset-lab/piglet-preview.png
```

The sheet capture writes a larger review image:

```text
/tmp/mclone-asset-lab/piglet-sheet.png
```

The video capture writes an MP4 animation review:

```text
/tmp/mclone-asset-lab/piglet-walk.mp4
```

Walk-cycle review captures keep the figure centered and move the grid floor
backward by the clip's authored cycle distance. The default MP4 spans multiple
cycles so stance timing, recovery timing, and visible foot sliding are easier to
critique than in a single-loop pose preview.

The next useful captures are:

- static front / side / three-quarter sheet
- walk-cycle side and three-quarter animation strips
- material/texture atlas preview
- debug overlay with origins, pivots, part names, and bounds
- regression mode that compares two generated figures or two revisions

## Non-Goals

- Do not integrate these figures into gameplay yet.
- Do not add a Rust loader until the lab proves the exported data shape.
- Do not make glTF the source format in the first phase.
- Do not let arbitrary Three.js mutations become accepted asset authoring.
- Do not create a large shared character file; keep one figure per folder.

## Implementation Slices

### Slice 1 - Lab Scaffold

- [x] Add `tools/asset-lab/package.json`, `tsconfig.json`, and README.
- [x] Add the typed DSL and runtime validation.
- [x] Add a browser preview backed by Three.js.
- [x] Add a Playwright smoke script that captures the preview to `/tmp`.
- [x] Add one `piglet` example with ASCII texture source and a walk clip.
- [x] Add root `pnpm asset-lab:*` wrappers.

Validation:

```powershell
pnpm asset-lab:install
pnpm asset-lab:typecheck
pnpm asset-lab:export
pnpm asset-lab:smoke
node -e "JSON.parse(require('fs').readFileSync('package.json','utf8'))"
git diff --check
```

Landed validation:

```powershell
pnpm asset-lab:install
pnpm --dir ./tools/asset-lab install:browsers
pnpm asset-lab:typecheck
pnpm asset-lab:export
pnpm asset-lab:smoke
node -e "JSON.parse(require('fs').readFileSync('package.json','utf8')); JSON.parse(require('fs').readFileSync('tools/asset-lab/package.json','utf8'))"
git diff --check
```

Smoke output inspected:

```text
/tmp/mclone-asset-lab/piglet-preview.png
```

Known follow-up: the Playwright smoke is reliable, but local startup/shutdown is
still slower than expected for a tiny preview. Keep the visual path working and
profile the harness before adding animation-sheet captures.

### Slice 2 - Better Agent Feedback

- [x] Add static multi-view sheet capture.
- [x] Add side and three-quarter animation strip capture with fixed frame
      times.
- [x] Add bounds/pivot/joint debug overlay toggles.
- [x] Add Minecraft-style box face overrides so a face texture can target only
      `north`, `south`, `east`, `west`, `up`, or `down`.
- [x] Add real pivot-group rendering so animated parts rotate around
      `joint.pivot` instead of their mesh center.
- [x] Add procedural walk-cycle authoring helpers for common `swing` and `bob`
      tracks while keeping exported clips as ordinary keyframes.
- [x] Add `quadrupedWalk`, `bipedWalk`, and `wingFlap` authoring macros that
      compile to ordinary keyframes.
- [x] Add gait locomotion metadata for cycle distance, derived speed, forward
      direction, and stance/contact windows.
- [x] Let gait contact metadata target foot/hoof parts separately from the
      animated limb segment.
- [x] Add contact-shaped `contactSwing` tracks so gait macros can separate
      planted stance from recovery swing while still exporting keyframes.
- [x] Add MP4 animation review output from deterministic Playwright frames
      assembled with `ffmpeg`.
- [x] Make sheet and MP4 review floors scroll from locomotion metadata, with
      MP4 output defaulting to several cycles.
- [ ] Add a validation report that points to the asset file and part names.
- [ ] Add starter prompts and examples for sheep, dog, cat, butterfly, and
      player stand-in variants.

Landed validation:

```powershell
pnpm asset-lab:typecheck
pnpm asset-lab:export
pnpm asset-lab:smoke
pnpm asset-lab:sheet
pnpm asset-lab:video
node -e "JSON.parse(require('fs').readFileSync('package.json','utf8')); JSON.parse(require('fs').readFileSync('tools/asset-lab/package.json','utf8'))"
git diff --check
```

Sheet output inspected:

```text
/tmp/mclone-asset-lab/piglet-sheet.png
```

Video output inspected:

```text
/tmp/mclone-asset-lab/piglet-walk.mp4
```

### Slice 3 - Runtime Promotion Decision

- [ ] Decide whether the JSON format is worth loading in native Rust.
- [ ] If yes, create a shared owner before touching app crates.
- [ ] Convert ASCII textures into the existing asset/atlas path or a deliberate
      figure atlas path.
- [ ] Render figures as ordinary entity snapshots through shared renderer
      contracts.

## Open Questions

- Should part pivots become explicit required data for animated parts, or should
  `at` remain the default pivot?
- Should the exported format use JSON for early tool compatibility or RON for
  easier Rust-side hand editing once promoted?
- Should generated texture PNGs be written beside `figure.json`, or should the
  runtime consume ASCII textures directly?
- How much vanilla Minecraft proportion should the starter examples preserve
  versus intentionally leaning into capsules/spheres and toy-like forms?
