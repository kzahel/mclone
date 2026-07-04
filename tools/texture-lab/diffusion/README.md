# Texture Lab Diffusion Proposals

Isolated Python runner for diffusion-assisted texture detail proposals. This
tool is intentionally upstream of committed texture source: it writes raw
proposal PNGs, tiled review sheets, seam metrics, and provenance manifests
under `/tmp/mclone-texture-lab/diffusion/`.

Do not use Mojang textures as img2img input. Feed only owned mclone macro art or
synthetic test images.

## Setup

```sh
cd tools/texture-lab/diffusion
uv sync
```

The first real generation downloads Stable Diffusion 1.5 weights into the
normal Hugging Face cache. Model weights must not be stored in this repository.

On Apple Silicon, the runner enables `PYTORCH_ENABLE_MPS_FALLBACK=1` by
default. The plan target is MPS/fp16, but the initial local smoke on this
machine produced NaN/black output with `--dtype auto`; use `--dtype fp32` for
MPS proposal runs until that is revisited. On CUDA machines, install the
appropriate PyTorch CUDA wheel set for that machine if the default resolver
does not select it.

The diffusers safety checker is disabled by default because this material-only
tool can otherwise blacken false-positive texture candidates. Do not repurpose
this runner for people, characters, or unsafe content. Pass `--safety-checker`
when you explicitly want the diffusers checker enabled.

## Run

```sh
cd tools/texture-lab/diffusion
uv run python propose.py \
  --input /tmp/owned-test-input.png \
  --prompt-preset stone-granite \
  --seeds 1001 \
  --strengths 0.5 \
  --steps 20 \
  --dtype fp32 \
  --out-dir /tmp/mclone-texture-lab/diffusion/smoke
```

Outputs include one PNG per seed/strength pair, a 3x3 self-tiled sheet for each
candidate, an aggregate `contact-sheet.png`, and `manifest.json` with input
hashes, model provenance, device, patched convolution counts, image stats, and
wrap seam metrics.

Use `--no-seamless` to A/B the circular-padding patch.

Useful conditioning flags:

- `--pre-blur <radius>` blurs the nearest-resized 512x512 input before img2img.
  This reduces hard 16x16 macro cell edges without losing the broad value plan.
- `--input-grain <amount>` blends deterministic neutral grain into the prepared
  input. This gives SD a natural material texture hint instead of only blurred
  blocks. `--input-grain-amplitude` and `--input-grain-seed` make the grain
  reproducible.
- `--prompt-preset stone-hewn-horizontal` and
  `--prompt-preset stone-dressed-courses` are the current best stone prompt
  families. Explicit `--prompt` or `--negative` still override preset text.
- `--prompt-preset grass-top-tufts` and
  `--prompt-preset grass-top-fine-turf` are first-pass grass top prompt
  families. Grass is more sensitive than stone: broad leaves, cracks, cells,
  and long-blade structure should be treated as prompt failures unless they
  project into convincing small turf.

## Stone Sweep

Export the owned 16x16 stone macro mask from the TypeScript lab:

```sh
pnpm --dir tools/texture-lab export -- \
  --texture stone \
  --authoring-role structure \
  --authoring-only \
  --out /tmp/mclone-texture-lab/diffusion/stone-input
```

Run the current stone proposal sweep:

```sh
cd tools/texture-lab/diffusion
uv run python propose.py \
  --input /tmp/mclone-texture-lab/diffusion/stone-input/authoring/stone-structure.png \
  --prompt "top-down photograph of a rough gray stone surface, chipped granite, matte, flat even lighting, seamless texture, no shadows" \
  --seeds 2001 2002 2003 2004 2005 2006 2007 2008 \
  --strengths 0.35 0.5 0.65 \
  --steps 20 \
  --dtype fp32 \
  --out-dir /tmp/mclone-texture-lab/diffusion/stone
```

Run the M2b prompt/preprocess sweep:

```sh
cd tools/texture-lab/diffusion
uv run python propose.py \
  --input /tmp/mclone-texture-lab/diffusion/stone-input/authoring/stone-structure.png \
  --prompt-preset stone-hewn-horizontal \
  --seeds 4101 4102 4103 4104 4105 4106 4107 4108 \
  --strengths 0.50 0.58 0.66 0.74 \
  --steps 24 \
  --dtype fp32 \
  --pre-blur 12 \
  --input-grain 0.16 \
  --input-grain-amplitude 18 \
  --input-grain-seed 12345 \
  --out-dir /tmp/mclone-texture-lab/diffusion/stone-m2b-hewn-horizontal

uv run python propose.py \
  --input /tmp/mclone-texture-lab/diffusion/stone-input/authoring/stone-structure.png \
  --prompt-preset stone-dressed-courses \
  --seeds 4201 4202 4203 4204 4205 4206 4207 4208 \
  --strengths 0.50 0.58 0.66 0.74 \
  --steps 24 \
  --dtype fp32 \
  --pre-blur 12 \
  --input-grain 0.16 \
  --input-grain-amplitude 18 \
  --input-grain-seed 12345 \
  --out-dir /tmp/mclone-texture-lab/diffusion/stone-m2b-dressed-courses
```

## Grass Top Sweep

Grass top uses a tint-neutral authoring mask. The `grass_block_top` structure
mask in the pack source has `opacity: 0`, so it is only conditioning input for
diffusion and does not change the active exported grass texture.

Export the owned 16x16 grass top macro mask from the TypeScript lab:

```sh
pnpm --dir tools/texture-lab export -- \
  --texture grass_block_top \
  --authoring-role structure \
  --authoring-only \
  --out /tmp/mclone-texture-lab/diffusion/grass-top-input
```

Run the first two grass proposal sweeps:

```sh
cd tools/texture-lab/diffusion
uv run python propose.py \
  --input /tmp/mclone-texture-lab/diffusion/grass-top-input/authoring/grass_block_top-structure.png \
  --prompt-preset grass-top-tufts \
  --seeds 5101 5102 5103 5104 \
  --strengths 0.50 0.62 0.74 \
  --steps 24 \
  --dtype fp32 \
  --pre-blur 10 \
  --input-grain 0.18 \
  --input-grain-amplitude 18 \
  --input-grain-seed 24680 \
  --out-dir /tmp/mclone-texture-lab/diffusion/grass-top-m1-tufts

uv run python propose.py \
  --input /tmp/mclone-texture-lab/diffusion/grass-top-input/authoring/grass_block_top-structure.png \
  --prompt-preset grass-top-fine-turf \
  --seeds 5201 5202 5203 5204 \
  --strengths 0.40 0.52 0.64 \
  --steps 24 \
  --dtype fp32 \
  --pre-blur 16 \
  --input-grain 0.20 \
  --input-grain-amplitude 16 \
  --input-grain-seed 24681 \
  --out-dir /tmp/mclone-texture-lab/diffusion/grass-top-m1-fine-turf
```

Project both sweeps at 64px for review:

```sh
pnpm --dir tools/texture-lab project-diffusion -- \
  --manifest /tmp/mclone-texture-lab/diffusion/grass-top-m1-tufts/manifest.json \
  --texture grass_block_top \
  --palette-colors dark,shadow,base,blade,light \
  --symbols dsmbh \
  --resolutions 64 \
  --review-sheet /tmp/mclone-texture-lab/diffusion-projection/grass-top-m1-64/projection-review-all-64.png \
  --review-top 12 \
  --review-resolution 64 \
  --out /tmp/mclone-texture-lab/diffusion-projection/grass-top-m1-64

pnpm --dir tools/texture-lab project-diffusion -- \
  --manifest /tmp/mclone-texture-lab/diffusion/grass-top-m1-fine-turf/manifest.json \
  --texture grass_block_top \
  --palette-colors dark,shadow,base,blade,light \
  --symbols dsmbh \
  --resolutions 64 \
  --review-sheet /tmp/mclone-texture-lab/diffusion-projection/grass-top-fine-m1-64/projection-review-all-64.png \
  --review-top 12 \
  --review-resolution 64 \
  --out /tmp/mclone-texture-lab/diffusion-projection/grass-top-fine-m1-64
```

Current read: `G5101S74` is the active grass top trial. The first 5-tone
projection made grass candidates read too gray and constrained; a 20-tone
tint-neutral ramp preserved fine tuft coverage better after the default grass
tint was applied. `G5101S74` was archived at
`/tmp/mclone-texture-lab/diffusion-archive/grass-top-g5101s74-active-20-2026-07-04/`
and frozen into `grass_block_top` as a 64px source mask. The remaining failure
mode is repetition/directional texture at tile scale, which should be judged in
block/game context before another prompt sweep.

## Project Candidates

Projection is handled by the TypeScript lab, not the Python runner. It consumes
a diffusion `manifest.json`, area-downsamples raw candidates to the requested
source resolution, quantizes into the pack palette in Oklab space, pins each
16x16 macro cell back to the owned input mask by deterministic majority
correction, then emits PNGs, ASCII masks, and per-candidate reports.

Project one candidate at 32/64/128:

```sh
pnpm --dir tools/texture-lab project-diffusion -- \
  --manifest /tmp/mclone-texture-lab/diffusion/stone-m2b-dressed-courses/manifest.json \
  --candidate candidate-seed4204-strength0p660 \
  --texture stone \
  --palette-colors pit,mid,base,light \
  --symbols pmbh \
  --resolutions 32,64,128 \
  --out /tmp/mclone-texture-lab/diffusion-projection/stone-dressed-4204-066
```

Batch-project a sweep at 64px for ranking:

```sh
pnpm --dir tools/texture-lab project-diffusion -- \
  --manifest /tmp/mclone-texture-lab/diffusion/stone-m2b-dressed-courses/manifest.json \
  --texture stone \
  --palette-colors pit,mid,base,light \
  --symbols pmbh \
  --resolutions 64 \
  --review-sheet \
  --review-top 8 \
  --review-resolution 64 \
  --out /tmp/mclone-texture-lab/diffusion-projection/stone-dressed-m2b-64
```

Outputs are written under the selected `--out` directory:

- `<candidate>/<candidate>-<resolution>.png`
- `<candidate>/<candidate>-<resolution>.mask.txt`
- `<candidate>/<candidate>-projection-report.json`
- `projection-summary.json`
- `projection-review-<resolution>.png` when `--review-sheet` is passed

The review sheet sorts projected candidates by triage score and shows each
candidate with a compact codename against the owned 16x16 macro mask, the
current authored texture, the projected source pixels, a 3x3 tile preview, a
16x16 distance preview, and 64/32/16/8 mip views. Codenames use the prompt
family prefix, seed, and strength, for example `D4201S58` means dressed-courses
seed 4201 at strength 0.58 and `H4107S66` means hewn-horizontal seed 4107 at
strength 0.66. For 128px projections the 3x3 tile panel uses a 64px view so
64px and 128px candidates can be judged on the same distance read.

## Archive Freeze Candidates

Raw diffusion output is not committed to git, but freeze candidates should be
archived as a durable local artifact bundle before their ASCII mask is copied
into pack source. The bundle keeps the raw candidate PNG, prepared input,
diffusion manifest, projection reports, projected masks, a review sheet, hashes,
and a source-ready provenance comment.

```sh
pnpm --dir tools/texture-lab project-diffusion -- \
  --manifest /tmp/mclone-texture-lab/diffusion/stone-m2b-dressed-courses/manifest.json \
  --candidate candidate-seed4201-strength0p580 \
  --candidate candidate-seed4205-strength0p580 \
  --texture stone \
  --palette-colors pit,mid,base,light \
  --symbols pmbh \
  --resolutions 64 \
  --review-sheet \
  --review-top 2 \
  --review-resolution 64 \
  --archive-bundle /tmp/mclone-texture-lab/diffusion-archive/stone-dressed-freeze-candidates-2026-07-03 \
  --out /tmp/mclone-texture-lab/diffusion-projection/stone-dressed-freeze-64
```

The archive bundle includes `archive-manifest.json` as its machine-readable
index and `source-provenance-comment.txt` under each candidate directory.
Exact byte-for-byte diffusion regeneration across another machine is still not
guaranteed; reprojection from the archived raw candidate PNG is deterministic
for the same texture-lab code and pack source.
