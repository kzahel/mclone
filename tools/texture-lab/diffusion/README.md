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
  --prompt "top-down photograph of a rough gray stone surface, chipped granite, matte, flat even lighting, seamless texture, no shadows" \
  --seeds 1001 \
  --strengths 0.5 \
  --steps 20 \
  --dtype fp32 \
  --out-dir /tmp/mclone-texture-lab/diffusion/smoke
```

Outputs include one PNG per seed/strength pair, a 3x3 self-tiled sheet for each
candidate, and `manifest.json` with input hashes, model provenance, device,
patched convolution counts, and wrap seam metrics.

Use `--no-seamless` to A/B the circular-padding patch.
