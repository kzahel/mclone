# Diffusion Upscale Plan

Working plan for the diffusion-assisted 16x16 → 32x32 texture upscale
experiment described in
[`UPSCALING_PROBLEM_STATEMENT.md`](UPSCALING_PROBLEM_STATEMENT.md).

## Thesis

Purely algorithmic upscalers add structured pseudo-detail, not information.
Diffusion super-resolution injects a learned material prior — real chips,
grain, and boundary placement — which is the missing ingredient. But raw
diffusion output violates the lab's invariants (determinism, palette
discipline, tiling, macro fidelity, reviewable source). So the pipeline is
split:

- **Python proposes.** A diffusion img2img pass, made seamless-tileable by
  construction, generates many candidate detail proposals from the accepted
  16x16 macro plan.
- **TypeScript disciplines.** The existing lab projects each proposal into the
  constraint space: downsample, palette-quantize, macro-correct, clean up, and
  emit a 32x32 ASCII mask plus review sheets and analyzer metrics.
- **Human judges.** Analyzer metrics triage and rank; a person picks the
  winner from a review sheet.
- **The frozen mask is the source.** The committed artifact is the projected,
  human-accepted 32x32 ASCII structure mask in the pack TypeScript. The
  diffusion pipeline is a proposal tool; its raw outputs are never committed.

Determinism is preserved where it matters: source → export stays fully
deterministic because the source is the frozen mask. The stochastic step only
exists upstream of acceptance, and every proposal records its provenance
(model, prompt, seed, strength, input hash) so a run can be re-created on the
same machine.

## Legal guardrails

- **Never feed Mojang textures into the diffusion input.** The img2img input
  must be our own accepted macro art. Running vanilla stone through img2img
  would make the output derivative source art, which the pack policy forbids.
- Do not use community models fine-tuned on Minecraft texture packs. Stick to
  general-purpose base models.
- Raw diffusion PNGs stay under `/tmp`, gitignored, private-reference only —
  same policy as the vanilla reference panels.
- The committed mask must pass through projection (quantize + macro-correct +
  cleanup) and human review before it becomes source. Record provenance in a
  source comment next to the mask.

## Pipeline

```text
accepted 16x16 macro plan (pack source, already reviewed)
  │  export PNG                                   [TS, existing]
  ▼
nearest-upscale to 512x512                        [Python]
  │
  ▼
seamless img2img sweep: prompt × seeds × denoise  [Python, MPS/CUDA]
  │  raw 512x512 PNGs + manifest.json → /tmp
  ▼
project each candidate:                           [TS, new lab command]
  box-downsample 512 → 32
  palette-quantize to the material ramp
  macro-correct (pin the 16x16 downsample)
  cleanup (sparkle, connectivity, seams)
  │  32x32 PNG + ASCII mask per candidate
  ▼
triage + rank with analyzer metrics               [TS, existing + new]
  │  survivors only
  ▼
contact/review sheet vs baselines                 [TS]
  │  human picks winner
  ▼
freeze winner's ASCII mask into pack source       [manual commit]
```

### Stage detail

1. **Input.** The accepted 16x16 macro plan for the material (for stone, the
   current 16x16 structure mask in
   `packs/mclone-default/block/stone.ts`), rendered to PNG by the lab.
   Check early whether the lab can export the macro mask *alone* at 16x16 —
   not the full composite with the low-opacity procedural layers. If it
   cannot, adding that export option is a small TS change that belongs at
   the start of M2.
2. **Pre-scale.** Nearest-neighbor to 512x512 (each texel becomes a 32x32
   block). Diffusion models are trained on photographs at this scale; they
   reinterpret the blocky input as a real material.
3. **Propose.** Stable Diffusion img2img with:
   - a per-material prompt (see prompt library below)
   - denoise strength sweep, e.g. `{0.35, 0.5, 0.65}` — this knob directly
     trades macro fidelity against detail invention
   - seed sweep, e.g. 8 seeds → 24 candidates per run
   - circular-padded convolutions so the output is toroidal and tiles by
     construction (see Tiling)
   - a `manifest.json` per run recording model id, prompt, negative prompt,
     seed, strength, steps, sampler, the SHA-256 of the input PNG, and the
     torch/diffusers versions plus device — seeds are only repeatable per
     (machine, torch version), so provenance must capture both
4. **Project.** New lab command (TS, next to the analyzer):
   - box-filter downsample 512 → 32
   - quantize each pixel to the nearest ramp tone in a perceptual space
     (Oklab), using the material's existing pack palette — no new colors, ever
   - macro-correct: for each of the 256 macro cells, if the 2x2 average
     drifts beyond tolerance from the accepted macro tone, swap the fewest
     subpixels toward the macro tone until it pins; keeps boundary detail
     while making macro fidelity exact by construction. Break ties with a
     fixed pixel scan order so projection is byte-for-byte reproducible —
     no randomness anywhere in this stage
   - cleanup: suppress isolated single pixels, cap high-contrast sparkle,
     enforce minimum run length 2, verify wrap seams
   - emit a 32x32 PNG and the equivalent ASCII mask (palette letters)
5. **Triage.** Run existing analyzer features plus the native-detail metrics
   from the problem statement (upscale flatness vs nearest baseline, 2x2 cell
   variance share, cell average error, native sparkle, native seam, micro
   repetition). Hard-kill invariant violators, rank the rest.
6. **Review.** Contact sheet: each survivor next to (a) the current
   nearest-upscaled 32, (b) the macro plan, (c) tiled 3x3, (d) mip chain.
   Human picks; at most a handful should survive triage.
7. **Freeze.** Replace the material's nearest-upscaled 16 mask with the
   winning 32x32 mask in pack source, with a provenance comment. Re-run
   export, analyze, runtime-compat, and an in-engine smoke check.

## Environment

Target dir: `tools/texture-lab/diffusion/` — an isolated Python project
managed by `uv`, with its own `pyproject.toml`. It must not become a
dependency of the TS lab; the only interface is PNG files plus
`manifest.json` under `/tmp/mclone-texture-lab/diffusion/`.

- **This machine (macOS, Apple M4 Pro, 48 GB):** PyTorch with the MPS
  backend. fp16. Set `PYTORCH_ENABLE_MPS_FALLBACK=1` for any op gaps.
- **Windows dev box:** same project, CUDA wheels instead. Keep the code
  device-agnostic (`mps` / `cuda` / `cpu` autodetect).
- **Python version:** pin 3.12 via `uv python pin` — PyTorch wheel coverage
  for 3.14 (the system default here) may lag.
- **Deps:** `torch`, `diffusers`, `transformers`, `accelerate`,
  `safetensors`, `pillow`.
- **Model:** Stable Diffusion 1.5 img2img from the community mirror
  `stable-diffusion-v1-5/stable-diffusion-v1-5` (~4 GB download on first
  run). Do NOT use `runwayml/stable-diffusion-v1-5` — that repo was removed
  from Hugging Face in 2024 and will 404. SD 1.5 is small, fast on MPS, and
  well past good enough for 32x32-after-projection output. Evaluate SDXL or
  ControlNet-Tile later only if projected quality demands it (M5).
- Model weights cache in the default Hugging Face cache dir, never in the
  repo.
- **Seeding:** create the generator on CPU
  (`torch.Generator("cpu").manual_seed(seed)`) and pass it to the pipeline
  call. CPU-generated latents keep a seed meaningful across mps/cuda/cpu;
  device-local generators do not.

Determinism note: fixed seeds are repeatable per (machine, torch version),
not across platforms. That is acceptable — proposals are not source. The
manifest is for provenance and same-machine reproduction, not a cross-platform
guarantee.

## Tiling

Make generation toroidal by patching every `Conv2d` in the UNet and VAE to
`padding_mode="circular"` before running the pipeline:

```python
def make_seamless(pipe):
    for module in pipe.unet.modules():
        if isinstance(module, torch.nn.Conv2d):
            module.padding_mode = "circular"
    for module in pipe.vae.modules():
        if isinstance(module, torch.nn.Conv2d):
            module.padding_mode = "circular"
```

Known gap: this loop is best-effort, not a proof. In diffusers' VAE encoder
the downsample blocks apply a manual asymmetric `F.pad(x, (0, 1, 0, 1))`
that a `Conv2d.padding_mode` patch never touches, so the encode path is not
perfectly toroidal. In practice the output still tiles well because the
UNet and decoder dominate, but the seam check is the real gate, not the
patch. Two consequences for the implementation:

- Give the runner a `--no-seamless` flag so seams can be A/B-compared with
  the patch on and off.
- Validate every run: tile each output 3x3 into a contact sheet and compute
  a wrap-boundary pixel diff. At projection time (M3) the lab's existing
  seam diagnostic re-checks the projected 32x32; a candidate with visible
  wrap mismatch is a hard kill.

X-only (asymmetric) tiling for directional materials like logs is possible
by patching only horizontal padding, but is deferred until a directional
material needs it.

## Prompt library

Per-material prompts live in a small checked-in file next to the runner
(`prompts.json` or a `.py` table). Prompting rules that matter for this use:

- Describe a **flat, evenly lit, top-down material surface**. Baked
  directional lighting or perspective breaks rotation safety and reads wrong
  on block faces: include "flat lighting, top-down, no shadows".
- Name the material language you want — "chipped gray granite" and
  "weathered limestone" produce different micro-structure. This is the
  semantic hint that makes the detail real rather than generic.
- Negative prompt the known failure modes: "blurry, smooth plastic, strong
  shadows, perspective, moss, colorful, glossy".
- Tint-role textures (grass top, foliage) are authored as neutral grayscale:
  either prompt for a grayscale material or desaturate before quantization.
  Defer these to M5; stone first.

Starting point for stone:

> top-down photograph of a rough gray stone surface, chipped granite, matte,
> flat even lighting, seamless texture, no shadows

## Milestones

Each milestone has an acceptance check so it can be handed to an agent and
verified independently.

### Implementation status

2026-07-03: M0/M1 runner bring-up is implemented under
`tools/texture-lab/diffusion/`:

- `uv` Python 3.12 project with locked dependencies.
- `propose.py` CLI for `--input`, `--prompt`, `--negative`, `--seeds`,
  `--strengths`, `--steps`, `--out-dir`, device autodetect, `--dtype`, and
  `--no-seamless`.
- SD 1.5 img2img loads from
  `stable-diffusion-v1-5/stable-diffusion-v1-5`.
- CPU-seeded generators, nearest resize to 512x512, circular Conv2d padding
  patch by default, one PNG per seed/strength, 3x3 self-tiled PNGs, image
  stats, seam metrics, and `manifest.json` provenance.

Local validation on macOS/M4/MPS:

- `uv sync`, CLI help, Python syntax, and a one-candidate img2img smoke pass.
- MPS/fp16 (`--dtype auto`) currently produces NaN/black output with torch
  2.12.1 + diffusers 0.39.0 on this machine; use `--dtype fp32` for real local
  proposal runs until the fp16 path is revisited.
- 20-step fp32 seamless smoke against an owned synthetic input wrote
  `/tmp/mclone-texture-lab/diffusion/smoke-m0m1-fp32-20step/manifest.json`.
  The patch touched 98 UNet Conv2d modules and 64 VAE Conv2d modules.
  Recorded wrap means were horizontal 7.0443 and vertical 9.6081 RGB levels;
  the 3x3 sheet had no obvious hard seam on visual review.
- Matching 20-step fp32 `--no-seamless` control recorded horizontal 23.6504
  and vertical 18.9355 mean wrap error, so the circular-padding patch is
  materially improving seams but is still a best-effort patch, not a proof.
- M2 input export and sweep are wired:
  - `src/export.ts` supports `--texture <name>`, `--authoring-role structure`,
    and `--authoring-only`.
  - Stone's 16x16 authoring structure was exported to
    `/tmp/mclone-texture-lab/diffusion/stone-input/authoring/stone-structure.png`.
  - The M2 sweep ran against that owned macro PNG with seeds `2001..2008`,
    strengths `0.35, 0.5, 0.65`, 20 steps, and `--dtype fp32`.
  - Output landed under `/tmp/mclone-texture-lab/diffusion/stone/`: 24 raw
    candidate PNGs, 24 per-candidate 3x3 tiled PNGs, `contact-sheet.png`, and
    `manifest.json`.
  - Manifest validation found 24 non-flat candidates, input nearest-resized
    from 16x16 to 512x512, and max mean wrap errors of horizontal 20.3822 and
    vertical 19.4798 RGB levels. Several candidates are visibly too blocky or
    tile-like, which is expected input for M3 triage rather than an M2 failure.
- M2b prompt/preprocess experiments changed the recommendation:
  - The original nearest-resized 16x16 macro over-conditioned SD toward blocky
    output. Blur plus deterministic neutral grain makes the model propose more
    natural rock surfaces while still starting from the owned macro plan.
  - `propose.py` now records first-class preprocessing knobs:
    `--pre-blur`, `--input-grain`, `--input-grain-amplitude`, and
    `--input-grain-seed`.
  - `propose.py` also has prompt presets for `stone-granite`,
    `stone-hewn-horizontal`, and `stone-dressed-courses`; explicit prompts and
    negative prompts can still override the preset text.
  - The M2b sweep ran two 32-candidate groups from the stone structure PNG:
    `/tmp/mclone-texture-lab/diffusion/stone-m2b-hewn-horizontal/` and
    `/tmp/mclone-texture-lab/diffusion/stone-m2b-dressed-courses/`.
  - Both used `--pre-blur 12`, `--input-grain 0.16`,
    `--input-grain-amplitude 18`, 24 steps, strengths
    `0.50, 0.58, 0.66, 0.74`, and 8 seeds. Each wrote 32 raw PNGs, 32 3x3
    sheets, `contact-sheet.png`, and `manifest.json`.
  - Visual read: hewn-horizontal has useful rough natural-rock candidates but
    can drift into marble/crack planes; dressed-courses gets closest to the
    rough-hewn horizontal/course idea but includes over-rectangular failures.
    This is now the better M3 input pool than the original M2 sweep.
- Initial M3 projection is implemented in `src/project-diffusion.ts`:
  - `pnpm --dir tools/texture-lab project-diffusion -- ...` consumes a
    diffusion `manifest.json`, one or more candidate ids, a target texture, a
    restricted palette color list, ASCII symbols, and one or more output
    resolutions.
  - Projection currently performs area downsample, Oklab nearest-palette
    quantization, deterministic macro majority correction against the
    manifest's owned 16x16 input mask, wrap-seam metrics, macro-fidelity
    metrics, detail metrics, triage scoring, per-candidate reports, and a run
    summary. Raw diffusion PNGs still stay under `/tmp`; the projected masks
    are review artifacts, not committed source.
  - Focused dressed candidate run:
    `/tmp/mclone-texture-lab/diffusion-projection/stone-dressed-4204-066/`
    projected `candidate-seed4204-strength0p660` at 32, 64, and 128. Macro
    majority mismatch was pinned to zero at every resolution. Correction cost
    dropped as resolution increased: 32px corrected 41.2109% of pixels, 64px
    corrected 24.7803%, and 128px corrected 20.4529%. All three remain
    `review` because the projected wrap seam still needs visual judgment.
  - 64px batch ranking was run for both M2b prompt pools:
    `/tmp/mclone-texture-lab/diffusion-projection/stone-dressed-m2b-64/` and
    `/tmp/mclone-texture-lab/diffusion-projection/stone-hewn-m2b-64/`.
    Dressed-courses ranked slightly better and looked quieter on visual
    inspection; hewn-horizontal remains interesting but tends toward
    veiny/marble structure.
  - Best 64px dressed review candidates by current score:
    `candidate-seed4201-strength0p500` (1.549082),
    `candidate-seed4201-strength0p580` (1.563073),
    `candidate-seed4205-strength0p580` (1.610107), and
    `candidate-seed4205-strength0p500` (1.61236).
  - Best 64px hewn review candidate by current score:
    `candidate-seed4107-strength0p660` (1.623365).
  - M3 is not done enough to freeze source yet. Next slice should build a
    review sheet for the top dressed/hewn projections with 3x3 tiling,
    baselines, and mip/distance previews, then decide whether 64px or 128px is
    worth carrying into `stone.ts`.
- M3b review sheets are implemented:
  - `project-diffusion` accepts `--review-sheet`, `--review-top <n>`, and
    `--review-resolution <n>`. The sheet sorts projected candidates by triage
    score and shows each row with a compact codename, metrics, the owned 16x16
    macro baseline, the current authored texture baseline, source pixels, a 3x3
    tile preview, a 16x16 distance preview, and 64/32/16/8 mip views.
    Codenames are prompt family + seed + strength, e.g. `D4201S58` for
    dressed-courses seed 4201 strength 0.58 and `H4107S66` for hewn-horizontal
    seed 4107 strength 0.66.
  - Review outputs:
    `/tmp/mclone-texture-lab/diffusion-projection/stone-dressed-m2b-64/projection-review-64.png`,
    `/tmp/mclone-texture-lab/diffusion-projection/stone-hewn-m2b-64/projection-review-64.png`,
    `/tmp/mclone-texture-lab/diffusion-projection/stone-dressed-shortlist-128/projection-review-128.png`,
    and
    `/tmp/mclone-texture-lab/diffusion-projection/stone-hewn-shortlist-128/projection-review-128.png`.
  - Full 64px candidate review sheets were regenerated with row codenames:
    `/tmp/mclone-texture-lab/diffusion-projection/stone-dressed-m2b-64/projection-review-all-64.png`
    and
    `/tmp/mclone-texture-lab/diffusion-projection/stone-hewn-m2b-64/projection-review-all-64.png`.
  - Visual read: dressed-courses is still the better prompt family. Its top
    64px candidates read as rough natural stone with some broad clouding and
    horizontal course hints. Hewn-horizontal remains useful prompt evidence but
    tends toward vertical panel seams, diagonal veins, or marble-like fracture
    structure.
  - 128px shortlist read: correction cost improves, but the 64-view tile and
    mip panels do not show enough visible benefit to justify freezing a 128px
    source yet. Current recommendation is to freeze a 64px dressed candidate
    first, then validate in-engine distance/mip behavior before considering a
    larger source.
  - Best freeze candidates by combined metric/visual read:
    `candidate-seed4201-strength0p580` for the quietest natural stone, or
    `candidate-seed4205-strength0p580` if the desired direction is more
    visibly horizontal dressed courses.
  - User review pick for the first active in-game trial is `H4106S58`
    (`candidate-seed4106-strength0p580`), from the hewn-horizontal prompt pool.
- M3c provenance/archive support is implemented:
  - `project-diffusion` accepts `--archive-bundle [dir]`. It writes a durable
    local bundle outside git containing `archive-manifest.json`, the source
    diffusion manifest, owned original/prepared inputs, raw candidate PNGs,
    raw 3x3 tiles, projected PNGs, ASCII masks, projection reports, optional
    review sheet, and `source-provenance-comment.txt` for each candidate.
  - The archive verifies expected SHA-256 hashes for raw candidates and input
    PNGs when those hashes are present in the diffusion manifest.
  - Current stone freeze-candidate bundle:
    `/tmp/mclone-texture-lab/diffusion-archive/stone-dressed-freeze-candidates-2026-07-03/`.
    It archives `candidate-seed4201-strength0p580` and
    `candidate-seed4205-strength0p580` at 64px.
  - Active trial bundle:
    `/tmp/mclone-texture-lab/diffusion-archive/stone-hewn-h4106s58-active-2026-07-03/`.
    It archives `H4106S58` at 64px with the source provenance comment now used
    in `tools/texture-lab/packs/mclone-default/block/stone.ts`.
  - Reproducibility position: the bundle is enough to preserve the exact raw
    artifacts and deterministically reproject them. It is not a guarantee that
    another machine can re-run diffusion and produce the same raw PNG bytes;
    seeds, prompts, versions, model id, input hashes, and preprocessing knobs
    are recorded, but device kernels and model-cache revisions can still
    diverge.
- M5 grass top experiment is started:
  - `grass_block_top` now has an authoring-only `structure` mask in
    `packs/mclone-default/block/grass-block.ts`. The layer has `opacity: 0`,
    so it is diffusion conditioning input only and does not change the active
    exported grass texture.
  - `propose.py` now has prompt presets for `grass-top-tufts` and
    `grass-top-fine-turf`.
  - The grass structure input exported to
    `/tmp/mclone-texture-lab/diffusion/grass-top-input/authoring/grass_block_top-structure.png`.
  - Two 12-candidate grass sweeps ran locally:
    `/tmp/mclone-texture-lab/diffusion/grass-top-m1-tufts/` and
    `/tmp/mclone-texture-lab/diffusion/grass-top-m1-fine-turf/`.
  - Both sweeps were projected at 64px with the tint-neutral grass ramp
    `dark,shadow,base,blade,light` and codename prefix `G`:
    `/tmp/mclone-texture-lab/diffusion-projection/grass-top-m1-64/projection-review-all-64.png`
    and
    `/tmp/mclone-texture-lab/diffusion-projection/grass-top-fine-m1-64/projection-review-all-64.png`.
  - Visual read: `G5101S50`, `G5101S62`, and `G5203S40` were useful directions,
    but first-pass 5-tone projection flattened too much tuft detail and made
    the candidates read gray until tint was applied.
  - Follow-up palette experiments projected selected candidates into 11-tone
    and 20-tone neutral grass ramps, then reviewed them with the default grass
    tint `#79b34e`. The broader ramp worked better for 64px tintable grass:
    extra tones preserve coverage/antialias values for fine blades and tufts
    without introducing new final biome colors.
  - Active grass top trial: `G5101S74` at 64px, projected into a 20-tone
    neutral ramp and frozen into
    `packs/mclone-default/block/grass-block.ts`.
  - Follow-up correction: the first committed 20-tone ramp was still olive,
    not neutral. `grass_top` is now a neutral grayscale ramp so raw tintable
    source matches the local vanilla raw hue/luminance before biome tint is
    applied.
  - Grass freeze archive:
    `/tmp/mclone-texture-lab/diffusion-archive/grass-top-g5101s74-active-20-2026-07-04/`.
    The committed mask structure still comes from the archived projected PNG;
    the exported PNG no longer byte-matches that archive because the
    tintable source palette was corrected from olive to neutral grayscale.
  - Current workflow bottleneck is no longer only prompt tuning. The next
    texture-lab scaling slice should be an interactive candidate browser:
    reference texture navigation by structure/brick/overlay/material role,
    generated candidate galleries with quick pack swapping, block/rendered
    previews, prompt metadata, and candidate accept/reject/freeze actions.

Implementation order: **M0 and M1 are one chunk — build them together.**
The circular-padding patch is ~10 lines and must be exercised from day one;
a runner built without it and retrofitted later invalidates any early
tuning. M2 (sweep wiring) comes next, then M3 (TS projection). Do NOT start
with M3: without real diffusion candidates, quantization and
macro-correction tuning is speculative work that will be redone.

- **M0 — environment bring-up.** `uv`-managed project under
  `tools/texture-lab/diffusion/` runs SD 1.5 img2img on MPS end to end on a
  test image at 512x512. One CLI entry point (e.g. `propose.py`) taking
  `--input`, `--prompt`, `--negative`, `--seeds`, `--strengths`, `--steps`,
  `--out-dir`, with device autodetect and the seamless patch on by default
  (`--no-seamless` to disable). Accept: a single command produces a PNG, a
  3x3 self-tiled contact sheet, and a `manifest.json` under
  `/tmp/mclone-texture-lab/diffusion/` in tens of seconds, not minutes
  (excluding the one-time model download).
- **M1 — seamless tiling (same chunk as M0).** Circular padding patch
  verified, not just applied. Accept: a generated texture tiled 3x3 shows
  no visible wrap seam, and a pixel-difference seam check across the wrap
  boundary is within the same tolerance the lab's seam diagnostic uses. If
  the seam fails with the patch on, that is a finding to report, not to
  silently work around — see the known gap in Tiling.
- **M2 — stone proposal sweep.** Done locally: export the current
  stone 16x16 macro plan to PNG (macro mask alone, not the composite — add
  a lab export option if one is missing), nearest-upscale, run the sweep
  (8 seeds × 3 strengths). Accept: 24 raw candidates plus a simple contact
  sheet under `/tmp/mclone-texture-lab/diffusion/stone/`, each with
  manifest provenance.
- **M2b — prompt/preprocess sweep.** Done locally: add blur/grain input
  conditioning and prompt presets, then run hewn-horizontal and dressed-courses
  sweeps. Accept: 64 additional raw candidates under `/tmp`, grouped by prompt,
  with contact sheets and manifest provenance. These become the preferred M3
  projection inputs.
- **M3 — projection + triage.** New TS lab command that consumes raw
  candidate PNGs and emits, per candidate: 32x32 quantized PNG, ASCII mask,
  and an analyzer report with macro-fidelity and native-detail metrics.
  Accept: running it on the M2 sweep hard-kills at least the obvious
  violators, ranks the rest, and `analyze --compare` works between any
  projected candidate and the current nearest-upscale stone.
- **M4 — tournament + freeze.** Review sheet of survivors vs baselines;
  human picks; winner's ASCII mask replaces the nearest-upscaled macro in
  `stone.ts` with a provenance comment. Accept: export + analyze +
  runtime-compat pass, macro fidelity metrics unchanged within tolerance,
  and an in-engine screenshot
  (`MCLONE_FIRST_PARTY_ASSET_ROOT=... pnpm native:timedemo:smoke`) reviewed.
  Status: active trial done for `H4106S58`; `stone`, `coal_ore`, and `iron_ore`
  now use the selected 64px stone base mask, the generated `stone.png` SHA-256
  matches the archived projection exactly, and the packed overlay was validated
  in native offscreen screenshots.
- **M5 — generalize.** Dirt and grass top (tint-neutral handling), prompt
  library growth, x-only tiling if a directional material lands, and an
  SDXL / ControlNet-Tile quality evaluation only if projected 32x32 output
  is visibly limited by the base model. Status: grass top is in active trial
  with `G5101S74`, a 64px source mask, and a 20-tone neutral tintable source
  ramp. Next scaling work should move candidate review into an interactive web
  UI instead of one-off static sheets.

## Open questions and knobs

- Denoise strength range: does anything above ~0.65 ever survive
  macro-correction, or does it always drift too far? Sweep and learn.
- Quantization space: Oklab nearest-tone is the starting point; evaluate
  luminance-weighted matching if hue noise causes tone misassignment.
- Macro-correction tolerance: how much per-cell average drift to allow
  before pinning (problem statement's open question). Start strict (exact
  pin), loosen only at tone boundaries if results look stair-stepped.
- Input choice: feed the 16x16 macro (clean signal) vs the current composite
  32 texture (includes the low-opacity procedural layers). Start with the
  macro.
- Whether SD 1.5's material prior is enough after projection, or whether
  SDXL/ControlNet-Tile measurably improves the *placement* signal that
  survives quantization. Decide with the M4 review sheet, not upfront.
- Projection resolution: the hewn/dressed proposals look good enough at 512
  that M3 should project selected candidates to 32, 64, and 128 review outputs
  before choosing a committed source resolution. Do not jump directly to
  committed 64/128 masks; first check whether the extra detail survives palette
  quantization, macro correction, mip review, and in-engine distance views.
