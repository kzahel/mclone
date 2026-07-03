#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
from random import Random
from typing import Any

DEFAULT_MODEL_ID = "stable-diffusion-v1-5/stable-diffusion-v1-5"
DEFAULT_NEGATIVE_PROMPT = (
    "blurry, smooth plastic, strong shadows, perspective, moss, colorful, glossy"
)
DEFAULT_OUT_ROOT = Path("/tmp/mclone-texture-lab/diffusion")
PROMPT_PRESETS = {
    "stone-granite": {
        "prompt": (
            "photorealistic top-down macro photograph of rough natural gray granite rock, "
            "fine mineral speckles, small chips, subtle pitted stone grain, matte dry "
            "surface, overcast diffuse lighting, seamless square material texture"
        ),
        "negative": (
            "minecraft, pixel art, voxel, square tiles, mosaic, brick wall, slab seams, "
            "large cracks, grid, checkerboard, marble, cloudy smoke, blurry, smooth "
            "plastic, strong shadows, perspective, moss, colorful, glossy"
        ),
    },
    "stone-hewn-horizontal": {
        "prompt": (
            "orthographic top-down macro photograph of rough hewn gray stone surface, "
            "horizontal chisel marks, primitive hand tooled rock, shallow layered "
            "strata, worn chipped granite, uneven matte stone, subtle pitting and "
            "mineral grain, flat overcast lighting, seamless square material texture"
        ),
        "negative": "perspective, mortar, glossy, colorful, moss, text, watermark",
    },
    "stone-dressed-courses": {
        "prompt": (
            "orthographic top-down macro photograph of rough dressed gray stone face, "
            "primitive hand hewn stone courses, uneven horizontal seams, chisel scars "
            "and tool marks, chipped block faces, gritty mineral grain, pitted matte "
            "granite, flat overcast lighting, seamless square material texture"
        ),
        "negative": "perspective, mortar, round pebbles, glossy, colorful, moss, text",
    },
}


@dataclass(frozen=True)
class PreparedInput:
    image: Any
    original_size: tuple[int, int]
    prepared_size: tuple[int, int]
    original_sha256: str
    prepared_sha256: str
    resized_nearest: bool


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    os.environ.setdefault("PYTORCH_ENABLE_MPS_FALLBACK", "1")

    prompt_settings = resolve_prompt_settings(args)
    seeds = parse_int_values(args.seeds, "--seeds")
    strengths = parse_float_values(args.strengths, "--strengths")
    if args.steps < 1:
        raise SystemExit("--steps must be at least 1")
    if args.pre_blur < 0:
        raise SystemExit("--pre-blur must be >= 0")
    if args.input_grain < 0 or args.input_grain > 1:
        raise SystemExit("--input-grain must be between 0 and 1")
    if args.input_grain_amplitude < 0 or args.input_grain_amplitude > 127:
        raise SystemExit("--input-grain-amplitude must be between 0 and 127")
    for strength in strengths:
        if not 0.0 <= strength <= 1.0:
            raise SystemExit(f"--strengths values must be between 0 and 1, got {strength}")
        if int(args.steps * strength) < 1:
            raise SystemExit(
                f"--steps {args.steps} and strength {strength} produce zero "
                "effective img2img denoising steps; increase --steps or --strengths"
            )

    out_dir = args.out_dir
    if out_dir is None:
        out_dir = DEFAULT_OUT_ROOT / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out_dir.mkdir(parents=True, exist_ok=True)

    torch, pipeline_class, pil_image, pil_image_filter = load_generation_modules()
    prepared = prepare_input(args, pil_image, pil_image_filter)
    prepared_path = out_dir / "input-prepared.png"
    prepared.image.save(prepared_path)

    device = detect_device(torch, args.device)
    dtype = resolve_torch_dtype(torch, device, args.dtype)
    pipeline_kwargs: dict[str, Any] = {"torch_dtype": dtype}
    if not args.safety_checker:
        pipeline_kwargs["safety_checker"] = None
        pipeline_kwargs["requires_safety_checker"] = False
    pipe = pipeline_class.from_pretrained(args.model_id, **pipeline_kwargs)
    pipe = pipe.to(device)

    if hasattr(pipe, "enable_attention_slicing"):
        pipe.enable_attention_slicing()

    seamless_patch = {
        "enabled": not args.no_seamless,
        "unet_conv2d_patched": 0,
        "vae_conv2d_patched": 0,
    }
    if not args.no_seamless:
        seamless_patch = make_seamless(pipe, torch)

    candidates: list[dict[str, Any]] = []
    candidate_images: list[tuple[str, Any]] = []
    for seed in seeds:
        for strength in strengths:
            generator = torch.Generator("cpu").manual_seed(seed)
            candidate_id = format_candidate_id(seed, strength)
            image = pipe(
                prompt=prompt_settings["prompt"],
                negative_prompt=prompt_settings["negative"],
                image=prepared.image,
                strength=strength,
                guidance_scale=args.guidance_scale,
                num_inference_steps=args.steps,
                generator=generator,
            ).images[0]

            png_path = out_dir / f"{candidate_id}.png"
            tile_path = out_dir / f"{candidate_id}-tile3x3.png"
            image.save(png_path)
            make_tile_sheet(image, pil_image).save(tile_path)
            seam = seam_metrics(image)
            candidates.append(
                {
                    "id": candidate_id,
                    "seed": seed,
                    "strength": strength,
                    "png": str(png_path),
                    "tile3x3_png": str(tile_path),
                    "sha256": sha256_file(png_path),
                    "image": image_metrics(image),
                    "seam": seam,
                }
            )
            candidate_images.append((candidate_id, image.copy()))

    contact_sheet_path: Path | None = None
    if candidate_images:
        contact_sheet_path = out_dir / "contact-sheet.png"
        make_contact_sheet(candidate_images, pil_image).save(contact_sheet_path)

    manifest = {
        "schema_version": 1,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "model_id": args.model_id,
        "prompt_preset": args.prompt_preset,
        "prompt": prompt_settings["prompt"],
        "negative_prompt": prompt_settings["negative"],
        "steps": args.steps,
        "guidance_scale": args.guidance_scale,
        "scheduler": pipe.scheduler.__class__.__name__,
        "device": device,
        "requested_dtype": args.dtype,
        "torch_dtype": str(dtype).replace("torch.", ""),
        "safety_checker_enabled": args.safety_checker,
        "seamless_patch": seamless_patch,
        "input": {
            "path": str(args.input),
            "prepared_png": str(prepared_path),
            "original_size": list(prepared.original_size),
            "prepared_size": list(prepared.prepared_size),
            "original_sha256": prepared.original_sha256,
            "prepared_pixels_sha256": prepared.prepared_sha256,
            "prepared_png_sha256": sha256_file(prepared_path),
            "resized_nearest": prepared.resized_nearest,
            "preprocess": {
                "pre_blur": args.pre_blur,
                "input_grain": args.input_grain,
                "input_grain_amplitude": args.input_grain_amplitude,
                "input_grain_seed": args.input_grain_seed,
            },
        },
        "versions": package_versions(
            [
                "accelerate",
                "diffusers",
                "pillow",
                "safetensors",
                "torch",
                "transformers",
            ]
        ),
        "runtime": {
            "python": sys.version.split()[0],
            "platform": platform.platform(),
            "pytorch_enable_mps_fallback": os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK"),
        },
        "contact_sheet_png": str(contact_sheet_path) if contact_sheet_path else None,
        "candidates": candidates,
    }
    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    print(f"Wrote {len(candidates)} candidate(s) to {out_dir}")
    if contact_sheet_path:
        print(f"Wrote contact sheet to {contact_sheet_path}")
    print(f"Wrote manifest to {out_dir / 'manifest.json'}")
    return 0


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate seamless diffusion img2img texture proposals."
    )
    parser.add_argument("--input", type=Path, required=True, help="Owned input PNG.")
    parser.add_argument(
        "--prompt-preset",
        choices=sorted(PROMPT_PRESETS.keys()),
        default=None,
        help="Built-in prompt/negative pair. Explicit --prompt or --negative override the preset text.",
    )
    parser.add_argument("--prompt", default=None, help="Positive img2img prompt.")
    parser.add_argument(
        "--negative",
        default=None,
        help="Negative prompt.",
    )
    parser.add_argument(
        "--seeds",
        nargs="+",
        default=["0"],
        help="Seed list, accepting spaces and/or commas. Example: 1 2 3 or 1,2,3.",
    )
    parser.add_argument(
        "--strengths",
        nargs="+",
        default=["0.5"],
        help="Denoise strength list, accepting spaces and/or commas.",
    )
    parser.add_argument("--steps", type=int, default=20, help="Inference steps.")
    parser.add_argument(
        "--guidance-scale",
        type=float,
        default=7.5,
        help="Classifier-free guidance scale.",
    )
    parser.add_argument(
        "--image-size",
        type=int,
        default=512,
        help="Prepared square input size. Non-matching inputs are nearest-resized.",
    )
    parser.add_argument(
        "--pre-blur",
        type=float,
        default=0.0,
        help="Gaussian blur radius applied after nearest resize, before img2img.",
    )
    parser.add_argument(
        "--input-grain",
        type=float,
        default=0.0,
        help="Blend amount for deterministic neutral grain applied after blur. 0 disables.",
    )
    parser.add_argument(
        "--input-grain-amplitude",
        type=int,
        default=18,
        help="Maximum +/- RGB value around 128 for --input-grain noise.",
    )
    parser.add_argument(
        "--input-grain-seed",
        type=int,
        default=12345,
        help="Seed for deterministic --input-grain noise.",
    )
    parser.add_argument(
        "--model-id",
        default=DEFAULT_MODEL_ID,
        help="Hugging Face diffusers model id.",
    )
    parser.add_argument(
        "--device",
        choices=["auto", "mps", "cuda", "cpu"],
        default="auto",
        help="Generation device.",
    )
    parser.add_argument(
        "--dtype",
        choices=["auto", "fp16", "fp32"],
        default="auto",
        help="Torch dtype. Auto uses fp16 on MPS/CUDA and fp32 on CPU.",
    )
    parser.add_argument(
        "--safety-checker",
        action="store_true",
        help=(
            "Enable diffusers safety checker. It is off by default because "
            "this material-only tool otherwise blackens false-positive texture candidates."
        ),
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="Output directory. Defaults under /tmp/mclone-texture-lab/diffusion/.",
    )
    parser.add_argument(
        "--no-seamless",
        action="store_true",
        help="Disable circular Conv2d padding patch for A/B seam tests.",
    )
    return parser.parse_args(argv)


def resolve_prompt_settings(args: argparse.Namespace) -> dict[str, str]:
    preset = PROMPT_PRESETS.get(args.prompt_preset) if args.prompt_preset else None
    prompt = args.prompt if args.prompt is not None else preset["prompt"] if preset else None
    if prompt is None:
        raise SystemExit("Provide --prompt or --prompt-preset")
    negative = args.negative if args.negative is not None else preset["negative"] if preset else DEFAULT_NEGATIVE_PROMPT
    return {"prompt": prompt, "negative": negative}


def parse_int_values(values: list[str], flag: str) -> list[int]:
    try:
        parsed = [int(part) for part in split_csvish(values)]
    except ValueError as exc:
        raise SystemExit(f"{flag} expects integers") from exc
    if not parsed:
        raise SystemExit(f"{flag} needs at least one value")
    return parsed


def parse_float_values(values: list[str], flag: str) -> list[float]:
    try:
        parsed = [float(part) for part in split_csvish(values)]
    except ValueError as exc:
        raise SystemExit(f"{flag} expects numbers") from exc
    if not parsed:
        raise SystemExit(f"{flag} needs at least one value")
    return parsed


def split_csvish(values: list[str]) -> list[str]:
    parts: list[str] = []
    for value in values:
        parts.extend(part.strip() for part in value.split(","))
    return [part for part in parts if part]


def load_generation_modules() -> tuple[Any, Any, Any, Any]:
    try:
        import torch
        from diffusers import StableDiffusionImg2ImgPipeline
        from PIL import Image
        from PIL import ImageFilter
    except ImportError as exc:
        raise SystemExit(
            "Missing diffusion dependencies. Run `uv sync` in tools/texture-lab/diffusion."
        ) from exc
    return torch, StableDiffusionImg2ImgPipeline, Image, ImageFilter


def prepare_input(args: argparse.Namespace, pil_image: Any, pil_image_filter: Any) -> PreparedInput:
    path = args.input
    image_size = args.image_size
    if image_size <= 0:
        raise SystemExit("--image-size must be positive")
    if not path.exists():
        raise SystemExit(f"Input does not exist: {path}")
    original_sha256 = sha256_file(path)
    image = pil_image.open(path).convert("RGB")
    original_size = image.size
    target_size = (image_size, image_size)
    resized = image.size != target_size
    if resized:
        image = image.resize(target_size, resample=pil_image.Resampling.NEAREST)
    if args.pre_blur > 0:
        image = image.filter(pil_image_filter.GaussianBlur(radius=args.pre_blur))
    if args.input_grain > 0:
        image = apply_input_grain(
            image,
            pil_image,
            opacity=args.input_grain,
            amplitude=args.input_grain_amplitude,
            seed=args.input_grain_seed,
        )
    prepared_sha256 = sha256_image(image)
    return PreparedInput(
        image=image,
        original_size=original_size,
        prepared_size=image.size,
        original_sha256=original_sha256,
        prepared_sha256=prepared_sha256,
        resized_nearest=resized,
    )


def apply_input_grain(image: Any, pil_image: Any, opacity: float, amplitude: int, seed: int) -> Any:
    rng = Random(seed)
    noise = pil_image.new("RGB", image.size)
    noise.putdata(
        [
            (value, value, value)
            for value in (
                128 + rng.randrange(-amplitude, amplitude + 1)
                for _ in range(image.size[0] * image.size[1])
            )
        ]
    )
    return pil_image.blend(image.convert("RGB"), noise, opacity)


def detect_device(torch: Any, requested: str) -> str:
    if requested != "auto":
        ensure_device_available(torch, requested)
        return requested
    if torch.cuda.is_available():
        return "cuda"
    if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def ensure_device_available(torch: Any, device: str) -> None:
    if device == "cuda" and not torch.cuda.is_available():
        raise SystemExit("Requested CUDA, but torch.cuda.is_available() is false")
    if (
        device == "mps"
        and not (hasattr(torch.backends, "mps") and torch.backends.mps.is_available())
    ):
        raise SystemExit("Requested MPS, but torch.backends.mps.is_available() is false")


def resolve_torch_dtype(torch: Any, device: str, requested: str) -> Any:
    if requested == "fp16":
        return torch.float16
    if requested == "fp32":
        return torch.float32
    if device == "cpu":
        return torch.float32
    return torch.float16


def make_seamless(pipe: Any, torch: Any) -> dict[str, Any]:
    return {
        "enabled": True,
        "unet_conv2d_patched": patch_conv2d_padding(pipe.unet, torch),
        "vae_conv2d_patched": patch_conv2d_padding(pipe.vae, torch),
    }


def patch_conv2d_padding(root: Any, torch: Any) -> int:
    count = 0
    for module in root.modules():
        if isinstance(module, torch.nn.Conv2d):
            module.padding_mode = "circular"
            count += 1
    return count


def make_tile_sheet(image: Any, pil_image: Any) -> Any:
    width, height = image.size
    sheet = pil_image.new("RGB", (width * 3, height * 3))
    rgb = image.convert("RGB")
    for y in range(3):
        for x in range(3):
            sheet.paste(rgb, (x * width, y * height))
    return sheet


def make_contact_sheet(candidate_images: list[tuple[str, Any]], pil_image: Any) -> Any:
    thumb_size = 192
    gutter = 8
    columns = min(6, max(1, len(candidate_images)))
    rows = (len(candidate_images) + columns - 1) // columns
    sheet = pil_image.new(
        "RGB",
        (columns * thumb_size + (columns + 1) * gutter, rows * thumb_size + (rows + 1) * gutter),
        (34, 36, 35),
    )
    for index, (_candidate_id, image) in enumerate(candidate_images):
        column = index % columns
        row = index // columns
        thumb = image.convert("RGB")
        thumb.thumbnail((thumb_size, thumb_size), resample=pil_image.Resampling.LANCZOS)
        x = gutter + column * (thumb_size + gutter) + (thumb_size - thumb.width) // 2
        y = gutter + row * (thumb_size + gutter) + (thumb_size - thumb.height) // 2
        sheet.paste(thumb, (x, y))
    return sheet


def seam_metrics(image: Any) -> dict[str, Any]:
    rgb = image.convert("RGB")
    width, height = rgb.size
    pixels = rgb.load()
    vertical = []
    horizontal = []
    for y in range(height):
        vertical.append(pixel_distance(pixels[0, y], pixels[width - 1, y]))
    for x in range(width):
        horizontal.append(pixel_distance(pixels[x, 0], pixels[x, height - 1]))
    return {
        "vertical_wrap_mean_abs_rgb": round(sum(vertical) / len(vertical), 4),
        "vertical_wrap_max_abs_rgb": max(vertical),
        "horizontal_wrap_mean_abs_rgb": round(sum(horizontal) / len(horizontal), 4),
        "horizontal_wrap_max_abs_rgb": max(horizontal),
    }


def image_metrics(image: Any) -> dict[str, Any]:
    rgb = image.convert("RGB")
    histogram = rgb.histogram()
    count = rgb.size[0] * rgb.size[1]
    mins: list[int] = []
    maxes: list[int] = []
    means: list[float] = []
    for channel in range(3):
        channel_histogram = histogram[channel * 256 : (channel + 1) * 256]
        mins.append(next(index for index, value in enumerate(channel_histogram) if value))
        maxes.append(
            max(index for index, value in enumerate(channel_histogram) if value)
        )
        total = sum(index * value for index, value in enumerate(channel_histogram))
        means.append(round(total / count, 4))
    return {
        "rgb_min": mins,
        "rgb_max": maxes,
        "rgb_mean": means,
        "is_flat": mins == maxes,
    }


def pixel_distance(left: tuple[int, int, int], right: tuple[int, int, int]) -> float:
    return sum(abs(left[channel] - right[channel]) for channel in range(3)) / 3


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_image(image: Any) -> str:
    digest = hashlib.sha256()
    digest.update(image.mode.encode("utf-8"))
    digest.update(str(image.size).encode("utf-8"))
    digest.update(image.tobytes())
    return digest.hexdigest()


def package_versions(packages: list[str]) -> dict[str, str | None]:
    versions: dict[str, str | None] = {}
    for package in packages:
        try:
            versions[package] = version(package)
        except PackageNotFoundError:
            versions[package] = None
    return versions


def format_candidate_id(seed: int, strength: float) -> str:
    strength_text = f"{strength:.3f}".replace(".", "p")
    return f"candidate-seed{seed}-strength{strength_text}"


if __name__ == "__main__":
    raise SystemExit(main())
