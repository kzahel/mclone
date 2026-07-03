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
