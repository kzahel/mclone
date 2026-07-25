#!/usr/bin/env python3
"""Build deterministic standalone first-party asset packs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import struct
import sys
import zipfile
import zlib
from pathlib import Path
from typing import Any, Callable

import asset_pack


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOL_PATH = "tools/minecraft_assets/first_party_pack.py"
FONT_PATH = REPO_ROOT / "tools/minecraft_assets/missing_font.v1.json"
OUTPUT_ROOT_ENV = "MCLONE_TEXTURE_LAB_OUTPUT_ROOT"
DEFAULT_OUTPUT_ROOT = Path(
    os.environ.get(OUTPUT_ROOT_ENV, REPO_ROOT / "generated-assets/texture-lab")
).expanduser()
DEFAULT_RUNTIME_ROOT = DEFAULT_OUTPUT_ROOT / "runtime-pack"
DEFAULT_AUTHORED_ROOT = DEFAULT_OUTPUT_ROOT / "pack"
DEFAULT_INVENTORY = DEFAULT_OUTPUT_ROOT / "first-party-inventory.v1.json"
DEFAULT_AUTHORED_OUTPUT = DEFAULT_OUTPUT_ROOT / "mclone-authored.pbp"
DEFAULT_FALLBACK_OUTPUT = DEFAULT_OUTPUT_ROOT / "mclone-generated-fallback.pbp"
DEFAULT_FALLBACK_ROOT = DEFAULT_OUTPUT_ROOT / "generated-fallback-root"
DEFAULT_DIAGNOSTIC_OUTPUT = DEFAULT_OUTPUT_ROOT / "mclone-diagnostic-missing.pbp"
DEFAULT_DIAGNOSTIC_ROOT = DEFAULT_OUTPUT_ROOT / "diagnostic-missing-root"
DEFAULT_STAGE_ROOT = REPO_ROOT / "generated-assets/first-party-stage/first-party-packs"
AUTHORED_PACK_ID = "mclone-authored"
FALLBACK_PACK_ID = "mclone-generated-fallback"
DIAGNOSTIC_PACK_ID = "mclone-diagnostic-missing"
ASSET_SCHEMA = "mclone-visuals-v1"
SHORT_CODE_MIN_LENGTH = 4
SHORT_CODE_ALPHABET = "0123456789ABCDEF"


def canonical_json_bytes(payload: Any) -> bytes:
    return json.dumps(payload, indent=2, sort_keys=True).encode("utf-8") + b"\n"


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical_json_bytes(payload))


def load_inventory(path: Path) -> dict[str, Any]:
    payload = asset_pack.read_json(path)
    if payload.get("schema_version") != 1:
        raise SystemExit(
            f"Unsupported first-party inventory schema {payload.get('schema_version')!r}"
        )
    if payload.get("asset_schema") != ASSET_SCHEMA:
        raise SystemExit(
            f"Inventory asset_schema {payload.get('asset_schema')!r} is not {ASSET_SCHEMA!r}"
        )
    for key in ("block_visuals", "materials", "direct_assets"):
        if not isinstance(payload.get(key), list):
            raise SystemExit(f"Inventory field {key!r} must be an array")
    return payload


def load_font(path: Path) -> dict[str, Any]:
    font = asset_pack.read_json(path)
    if font.get("schema_version") != 1:
        raise SystemExit(f"Unsupported missing font schema {font.get('schema_version')!r}")
    width = font.get("glyph_width")
    height = font.get("glyph_height")
    glyphs = font.get("glyphs")
    if not isinstance(width, int) or not isinstance(height, int) or not isinstance(glyphs, dict):
        raise SystemExit("Missing font dimensions/glyphs are invalid")
    for character in SHORT_CODE_ALPHABET:
        rows = glyphs.get(character)
        if (
            not isinstance(rows, list)
            or len(rows) != height
            or any(not isinstance(row, str) or len(row) != width for row in rows)
        ):
            raise SystemExit(f"Missing font glyph {character!r} is invalid")
    return font


def digest_code(resource_id: str, digest_fn: Callable[[bytes], bytes]) -> str:
    return digest_fn(resource_id.encode("utf-8")).hex().upper()


def assign_short_codes(
    resource_ids: list[str],
    digest_fn: Callable[[bytes], bytes] = lambda data: hashlib.sha256(data).digest(),
) -> dict[str, str]:
    ordered_ids = sorted(set(resource_ids))
    full_codes = {resource_id: digest_code(resource_id, digest_fn) for resource_id in ordered_ids}
    lengths = {resource_id: SHORT_CODE_MIN_LENGTH for resource_id in ordered_ids}

    while True:
        groups: dict[str, list[str]] = {}
        for resource_id in ordered_ids:
            full_code = full_codes[resource_id]
            length = lengths[resource_id]
            if length > len(full_code):
                raise SystemExit(
                    "Missing-resource hash collision could not be resolved for "
                    + ", ".join(ordered_ids)
                )
            groups.setdefault(full_code[:length], []).append(resource_id)
        collisions = [group for group in groups.values() if len(group) > 1]
        if not collisions:
            return {
                resource_id: full_codes[resource_id][: lengths[resource_id]]
                for resource_id in ordered_ids
            }
        for group in collisions:
            if len({full_codes[resource_id] for resource_id in group}) == 1:
                raise SystemExit(
                    "Missing-resource full hash collision: " + ", ".join(sorted(group))
                )
            for resource_id in group:
                lengths[resource_id] += 1


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
    )


def encode_rgba_png(width: int, height: int, rgba: bytes) -> bytes:
    expected = width * height * 4
    if len(rgba) != expected:
        raise SystemExit(f"RGBA payload had {len(rgba)} bytes, expected {expected}")
    rows = b"".join(
        b"\x00" + rgba[row * width * 4 : (row + 1) * width * 4]
        for row in range(height)
    )
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib_store(rows))
        + png_chunk(b"IEND", b"")
    )


def zlib_store(data: bytes) -> bytes:
    stream = bytearray(b"\x78\x01")
    if not data:
        stream.extend(b"\x01\x00\x00\xff\xff")
    for offset in range(0, len(data), 0xFFFF):
        block = data[offset : offset + 0xFFFF]
        final = offset + len(block) == len(data)
        stream.append(1 if final else 0)
        stream.extend(struct.pack("<H", len(block)))
        stream.extend(struct.pack("<H", (~len(block)) & 0xFFFF))
        stream.extend(block)
    stream.extend(struct.pack(">I", zlib.adler32(data) & 0xFFFFFFFF))
    return bytes(stream)


def missing_png(resource_id: str, code: str, font: dict[str, Any], width: int, height: int) -> bytes:
    digest = hashlib.sha256(resource_id.encode("utf-8")).digest()
    dark = tuple(20 + digest[index] % 44 for index in range(3))
    bright = tuple(168 + digest[index + 3] % 72 for index in range(3))
    pixels = bytearray(width * height * 4)

    def set_pixel(x: int, y: int, color: tuple[int, int, int, int]) -> None:
        if 0 <= x < width and 0 <= y < height:
            offset = (y * width + x) * 4
            pixels[offset : offset + 4] = bytes(color)

    for y in range(height):
        for x in range(width):
            border = x in (0, width - 1) or y in (0, height - 1)
            rgb = (255, 0, 255) if border and (x + y) % 2 == 0 else dark
            if not border and ((x // 4) + (y // 4)) % 2 == 0:
                rgb = bright
            set_pixel(x, y, (*rgb, 255))

    glyph_width = font["glyph_width"]
    glyph_height = font["glyph_height"]
    visible_code = code[: max(1, (width + 1) // (glyph_width + 1))]
    label_width = len(visible_code) * (glyph_width + 1) - 1
    origin_x = max(0, (width - label_width) // 2)
    origin_y = max(1, height - glyph_height - 1)
    for y in range(origin_y - 1, min(height - 1, origin_y + glyph_height + 1)):
        for x in range(max(1, origin_x - 1), min(width - 1, origin_x + label_width + 1)):
            set_pixel(x, y, (0, 0, 0, 255))
    for index, character in enumerate(visible_code):
        rows = font["glyphs"][character]
        for row_index, row in enumerate(rows):
            for column_index, bit in enumerate(row):
                if bit == "1":
                    set_pixel(
                        origin_x + index * (glyph_width + 1) + column_index,
                        origin_y + row_index,
                        (255, 255, 255, 255),
                    )
    return encode_rgba_png(width, height, bytes(pixels))


def provisional_base_rgba(resource_id: str) -> tuple[int, int, int, int]:
    name = resource_id.lower()
    palettes = (
        (("water", "underwater"), (54, 112, 180, 190)),
        (("lava", "magma"), (218, 89, 24, 255)),
        (("grass", "leaves", "fern", "poppy", "dandelion", "cornflower"), (91, 139, 63, 255)),
        (("dirt", "podzol", "farmland"), (122, 86, 54, 255)),
        (("sand", "sandstone"), (196, 178, 119, 255)),
        (("snow", "ice"), (202, 222, 224, 235)),
        (("oak", "wood", "log", "planks", "hay"), (151, 113, 67, 255)),
        (("spruce",), (104, 76, 48, 255)),
        (("brick", "terracotta"), (153, 84, 65, 255)),
        (("clay",), (150, 158, 172, 255)),
        (("coal", "deepslate", "bedrock"), (67, 69, 72, 255)),
        (("stone", "cobble", "gravel", "ore", "tuff", "andesite"), (126, 128, 125, 255)),
    )
    for tokens, color in palettes:
        if any(token in name for token in tokens):
            return color
    digest = hashlib.sha256(resource_id.encode("utf-8")).digest()
    return (88 + digest[0] % 96, 88 + digest[1] % 96, 88 + digest[2] % 96, 255)


def provisional_png(resource_id: str, width: int, height: int) -> bytes:
    """Create restrained deterministic first-party art, never diagnostic labels."""
    base = provisional_base_rgba(resource_id)
    name = resource_id.lower()
    pixels = bytearray(width * height * 4)
    plant = any(
        token in name
        for token in ("fern", "poppy", "dandelion", "cornflower", "sapling", "grass_cross")
    )
    ore_colors = (
        (("coal",), (45, 45, 43)),
        (("iron",), (190, 145, 108)),
        (("gold",), (231, 192, 56)),
        (("redstone",), (178, 43, 37)),
        (("lapis",), (45, 80, 164)),
        (("diamond",), (77, 198, 190)),
        (("copper",), (184, 107, 76)),
    )
    ore_color = next(
        (color for tokens, color in ore_colors if any(token in name for token in tokens)),
        None,
    )

    for y in range(height):
        for x in range(width):
            digest = hashlib.sha256(f"{resource_id}:{x // 2}:{y // 2}".encode("utf-8")).digest()
            variation = (digest[0] % 23) - 11
            red = max(0, min(255, base[0] + variation))
            green = max(0, min(255, base[1] + variation))
            blue = max(0, min(255, base[2] + variation))
            alpha = base[3]

            if plant:
                center = width // 2
                stem = abs(x - center) <= max(0, width // 16) and y >= height // 4
                leaves = abs(x - center) <= max(1, (height - y) // 3) and (x + y) % 3 != 0
                if not stem and not leaves:
                    alpha = 0
                elif "poppy" in name:
                    red, green, blue = (184, 47, 43) if y < height // 2 else (70, 126, 55)
                elif "cornflower" in name:
                    red, green, blue = (58, 101, 184) if y < height // 2 else (70, 126, 55)
                elif "dandelion" in name:
                    red, green, blue = (226, 190, 48) if y < height // 2 else (70, 126, 55)
            elif any(token in name for token in ("planks", "brick")):
                seam = y % max(3, height // 4) == 0
                stagger = (x + (y // max(3, height // 4)) * (width // 3)) % max(4, width // 2) == 0
                if seam or stagger:
                    red, green, blue = (max(0, red - 35), max(0, green - 35), max(0, blue - 35))
            elif "log" in name and ("top" in name or "end" in name):
                ring = max(abs(x * 2 - width + 1), abs(y * 2 - height + 1))
                if ring % 5 <= 1:
                    red, green, blue = (max(0, red - 24), max(0, green - 24), max(0, blue - 24))
            elif "log" in name and x % max(3, width // 5) == 0:
                red, green, blue = (max(0, red - 26), max(0, green - 26), max(0, blue - 26))

            if ore_color is not None and digest[1] < 32:
                red, green, blue = ore_color

            offset = (y * width + x) * 4
            pixels[offset : offset + 4] = bytes((red, green, blue, alpha))
    return encode_rgba_png(width, height, bytes(pixels))


def material_asset_path(material: str) -> str:
    try:
        namespace, path = material.split(":", 1)
    except ValueError:
        raise SystemExit(f"Invalid inventory material id {material!r}") from None
    if not namespace or not path:
        raise SystemExit(f"Invalid inventory material id {material!r}")
    return f"assets/{namespace}/textures/{path}.png"


def reset_staging_root(root: Path, expected_names: tuple[str, ...]) -> None:
    if root.exists():
        if root.name not in expected_names:
            raise SystemExit(
                f"Refusing to replace staging root {root}; expected one of {expected_names!r}"
            )
        shutil.rmtree(root)
    root.mkdir(parents=True)


def copy_repo_asset(repo_assets_root: Path, pack_path: str, staging_root: Path) -> None:
    prefix = "assets/"
    if not pack_path.startswith(prefix):
        raise SystemExit(f"Repo asset path {pack_path!r} does not start with assets/")
    source = repo_assets_root / pack_path[len(prefix) :]
    if not source.is_file():
        raise SystemExit(f"Required first-party asset is missing: {source}")
    destination = staging_root / pack_path
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(asset_pack.pack_file_bytes(source))


def write_generated_pack_metadata(
    inventory: dict[str, Any],
    repo_assets_root: Path,
    staging_root: Path,
) -> tuple[list[dict[str, Any]], list[str]]:
    direct_assets = inventory["direct_assets"]
    for asset in direct_assets:
        path = str(asset.get("path", ""))
        if asset.get("policy") == "required" and path.endswith(".json"):
            copy_repo_asset(repo_assets_root, path, staging_root)

    suppressed_audio = sorted(
        asset["path"]
        for asset in direct_assets
        if asset.get("consumer") == "audio" and asset.get("policy") == "suppressible"
    )
    write_json(
        staging_root / "assets/mclone/visuals/blocks.v1.json",
        {
            "schema_version": 1,
            "asset_schema": inventory["asset_schema"],
            "block_visuals": inventory["block_visuals"],
        },
    )
    write_json(
        staging_root / "assets/mclone/audio/missing-policy.v1.json",
        {"schema_version": 1, "suppressed": suppressed_audio},
    )
    return direct_assets, suppressed_audio


def prepare_generated_fallback(
    inventory: dict[str, Any],
    repo_assets_root: Path,
    staging_root: Path,
    _font_path: Path,
) -> dict[str, Any]:
    """Build the coherent provisional source kept at the legacy pack id."""
    reset_staging_root(staging_root, ("generated-fallback-root",))
    materials = sorted(set(inventory["materials"]))
    direct_assets, suppressed_audio = write_generated_pack_metadata(
        inventory, repo_assets_root, staging_root
    )
    required_pngs = sorted(
        asset["path"]
        for asset in direct_assets
        if asset.get("policy") == "required" and str(asset.get("path", "")).endswith(".png")
    )
    registry_entries = []
    for material in materials:
        path = material_asset_path(material)
        destination = staging_root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(provisional_png(material, 16, 16))
        registry_entries.append({"path": path, "resource_id": material})

    for path in required_pngs:
        resource_id = f"asset:{path}"
        dimensions = (64, 32) if path.endswith("entity/cow/cow.png") else (16, 16)
        destination = staging_root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(provisional_png(resource_id, *dimensions))
        registry_entries.append({"path": path, "resource_id": resource_id})

    write_json(
        staging_root / "assets/mclone/provisional/registry.v1.json",
        {
            "schema_version": 1,
            "entries": sorted(registry_entries, key=lambda item: item["resource_id"]),
        },
    )
    write_json(
        staging_root / "assets/mclone/reports/generated-fallback-coverage.v1.json",
        {
            "schema_version": 1,
            "origin": "first_party_provisional",
            "block_state_count": len(inventory["block_visuals"]),
            "material_count": len(materials),
            "generated_png_count": len(materials) + len(required_pngs),
            "copied_first_party_metadata_count": sum(
                1
                for asset in direct_assets
                if asset.get("policy") == "required" and str(asset.get("path", "")).endswith(".json")
            ),
            "suppressed_audio_count": len(suppressed_audio),
            "minecraft_payload_count": 0,
            "unknown_payload_count": 0,
        },
    )
    return {
        "material_count": len(materials),
        "block_state_count": len(inventory["block_visuals"]),
        "generated_png_count": len(materials) + len(required_pngs),
        "provisional_entry_count": len(registry_entries),
    }


def prepare_diagnostic_missing(
    inventory: dict[str, Any],
    repo_assets_root: Path,
    staging_root: Path,
    font_path: Path,
) -> dict[str, Any]:
    reset_staging_root(staging_root, ("diagnostic-missing-root",))
    font = load_font(font_path)
    materials = sorted(set(inventory["materials"]))
    direct_assets, suppressed_audio = write_generated_pack_metadata(
        inventory, repo_assets_root, staging_root
    )
    required_pngs = sorted(
        asset["path"]
        for asset in direct_assets
        if asset.get("policy") == "required" and str(asset.get("path", "")).endswith(".png")
    )
    visible_resources = materials + [f"asset:{path}" for path in required_pngs]
    short_codes = assign_short_codes(visible_resources)
    registry_entries = []

    for material in materials:
        path = material_asset_path(material)
        code = short_codes[material]
        destination = staging_root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(missing_png(material, code, font, max(16, len(code) * 4), 16))
        registry_entries.append({"code": code, "path": path, "resource_id": material})

    for path in required_pngs:
        resource_id = f"asset:{path}"
        code = short_codes[resource_id]
        dimensions = (
            (64, 32)
            if path.endswith("entity/cow/cow.png")
            else (max(16, len(code) * 4), 16)
        )
        destination = staging_root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(missing_png(resource_id, code, font, *dimensions))
        registry_entries.append({"code": code, "path": path, "resource_id": resource_id})

    write_json(
        staging_root / "assets/mclone/missing/registry.v1.json",
        {
            "schema_version": 1,
            "entries": sorted(registry_entries, key=lambda item: item["resource_id"]),
        },
    )
    write_json(
        staging_root / "assets/mclone/reports/diagnostic-missing-coverage.v1.json",
        {
            "schema_version": 1,
            "origin": "diagnostic",
            "block_state_count": len(inventory["block_visuals"]),
            "material_count": len(materials),
            "generated_png_count": len(materials) + len(required_pngs),
            "suppressed_audio_count": len(suppressed_audio),
            "minecraft_payload_count": 0,
            "unknown_payload_count": 0,
        },
    )
    return {
        "material_count": len(materials),
        "block_state_count": len(inventory["block_visuals"]),
        "generated_png_count": len(materials) + len(required_pngs),
        "short_code_count": len(short_codes),
    }


def entries_for_root(root: Path, path_prefix: str = "") -> list[asset_pack.PackEntry]:
    if not root.is_dir():
        raise SystemExit(f"Required first-party source root is missing: {root}")
    files = asset_pack.collect_files(root)
    entries = asset_pack.build_entries(root, files, path_prefix)
    return [
        asset_pack.PackEntry(
            path=entry.path,
            source_path=entry.source_path,
            bytes=entry.bytes,
            sha256=entry.sha256,
            compression="store",
        )
        for entry in entries
    ]


def first_party_manifest(
    pack_id: str,
    display_name: str,
    origin: str,
    roles: list[str],
    entries: list[asset_pack.PackEntry],
    source_names: list[str],
    extra: dict[str, Any] | None = None,
) -> dict[str, Any]:
    fingerprints = asset_pack.build_fingerprints(entries)
    payload_fingerprint = fingerprints[asset_pack.PAYLOAD_FINGERPRINT_ID]["sha256"]
    manifest = {
        "format_version": asset_pack.PACK_FORMAT_VERSION,
        "asset_set": pack_id,
        "pack_id": pack_id,
        "display_name": display_name,
        "origin": origin,
        "roles": roles,
        "asset_schema": ASSET_SCHEMA,
        "content_fingerprint": f"sha256:{payload_fingerprint}",
        "source_kind": "first_party_standalone",
        "tool": TOOL_PATH,
        "source_roots": [{"name": name} for name in source_names],
        "fingerprints": fingerprints,
        "file_count": len(entries),
        "files": [
            {
                "path": entry.path,
                "bytes": entry.bytes,
                "sha256": entry.sha256,
                "compression": entry.compression,
            }
            for entry in sorted(entries, key=lambda item: item.path)
        ],
    }
    if extra:
        manifest.update(extra)
    return manifest


def write_and_verify_pack(
    output: Path,
    sidecar: Path,
    manifest: dict[str, Any],
    entries: list[asset_pack.PackEntry],
) -> dict[str, Any]:
    asset_pack.assert_unique_entries(entries)
    # Store the manifest and every payload so archive bytes do not depend on the
    # host zlib version. Payload hashes still provide compact integrity records
    # inside the manifest.
    asset_pack.write_pack(
        output,
        sidecar,
        manifest,
        entries,
        manifest_compression="store",
    )
    verified = asset_pack.verify_pack(output, sidecar)
    if verified.get("content_fingerprint") != manifest["content_fingerprint"]:
        raise SystemExit("First-party pack content fingerprint changed during write")
    return verified


def build_authored_pack(
    runtime_root: Path,
    authored_root: Path,
    repo_assets_root: Path,
    output: Path,
    sidecar: Path,
) -> dict[str, Any]:
    entries = entries_for_root(runtime_root)
    entries.extend(entries_for_root(authored_root))
    entries.extend(entries_for_root(repo_assets_root, "assets"))
    asset_pack.assert_unique_entries(entries)
    manifest = first_party_manifest(
        AUTHORED_PACK_ID,
        "Mclone Original Assets",
        "first_party",
        ["authored_override", "render_content", "metadata"],
        entries,
        ["texture_lab_authored", "texture_lab_runtime_compat", "repo_first_party_assets"],
    )
    return write_and_verify_pack(output, sidecar, manifest, entries)


def build_generated_fallback_pack(
    inventory_path: Path,
    repo_assets_root: Path,
    staging_root: Path,
    font_path: Path,
    output: Path,
    sidecar: Path,
) -> dict[str, Any]:
    inventory = load_inventory(inventory_path)
    coverage = prepare_generated_fallback(inventory, repo_assets_root, staging_root, font_path)
    entries = entries_for_root(staging_root)
    manifest = first_party_manifest(
        FALLBACK_PACK_ID,
        "Mclone Provisional Textures",
        "first_party_provisional",
        ["provisional_base", "render_content", "audio_content", "metadata"],
        entries,
        ["canonical_first_party_inventory", "first_party_procedural_recipes", "repo_first_party_metadata"],
        {"coverage": coverage},
    )
    return write_and_verify_pack(output, sidecar, manifest, entries)


def build_diagnostic_missing_pack(
    inventory_path: Path,
    repo_assets_root: Path,
    staging_root: Path,
    font_path: Path,
    output: Path,
    sidecar: Path,
) -> dict[str, Any]:
    inventory = load_inventory(inventory_path)
    coverage = prepare_diagnostic_missing(
        inventory, repo_assets_root, staging_root, font_path
    )
    entries = entries_for_root(staging_root)
    manifest = first_party_manifest(
        DIAGNOSTIC_PACK_ID,
        "Numbered Missing Diagnostics",
        "diagnostic",
        ["diagnostic_fallback", "render_content", "audio_content", "metadata"],
        entries,
        ["canonical_first_party_inventory", "missing_bitmap_font", "repo_first_party_metadata"],
        {"coverage": coverage},
    )
    return write_and_verify_pack(output, sidecar, manifest, entries)


def verify_first_party_pack(output: Path, sidecar: Path | None = None) -> dict[str, Any]:
    manifest = asset_pack.verify_pack(output, sidecar)
    if manifest.get("origin") not in {
        "first_party",
        "first_party_provisional",
        "diagnostic",
    }:
        raise SystemExit(f"Unexpected first-party pack origin {manifest.get('origin')!r}")
    if not manifest.get("pack_id") or not manifest.get("content_fingerprint"):
        raise SystemExit("First-party manifest is missing identity/fingerprint metadata")
    _portable, payload = asset_pack.pack_fingerprint_manifests(output)
    payload_summary = asset_pack.payload_fingerprint_summary(payload)
    declared_payload = manifest.get("fingerprints", {}).get(asset_pack.PAYLOAD_FINGERPRINT_ID)
    if declared_payload != payload_summary:
        raise SystemExit("First-party payload fingerprint does not match archive contents")
    if manifest["content_fingerprint"] != f"sha256:{payload_summary['sha256']}":
        raise SystemExit("First-party content_fingerprint does not match archive contents")
    with zipfile.ZipFile(output) as archive:
        if any(info.compress_type != zipfile.ZIP_STORED for info in archive.infolist()):
            raise SystemExit("First-party pack contains a host-zlib-dependent compressed entry")
    forbidden = ("reference/minecraft-1.17.1", "minecraft_extracted", "client-deobf.jar")
    encoded_manifest = canonical_json_bytes(manifest).decode("utf-8")
    if any(marker in encoded_manifest for marker in forbidden):
        raise SystemExit("First-party manifest names a forbidden Minecraft reference source")
    return manifest


def stage_first_party_packs(
    authored: Path,
    fallback: Path,
    diagnostic: Path,
    stage_root: Path,
) -> dict[str, Any]:
    if stage_root.exists():
        if stage_root.name != "first-party-packs":
            raise SystemExit(
                f"Refusing to replace stage root {stage_root}; expected basename first-party-packs"
            )
        shutil.rmtree(stage_root)
    stage_root.mkdir(parents=True)

    packs = []
    for source in (authored, fallback, diagnostic):
        sidecar = Path(f"{source}.json")
        manifest = verify_first_party_pack(source, sidecar)
        destination = stage_root / source.name
        destination_sidecar = stage_root / sidecar.name
        shutil.copyfile(source, destination)
        shutil.copyfile(sidecar, destination_sidecar)
        packs.append(
            {
                "pack_id": manifest["pack_id"],
                "origin": manifest["origin"],
                "pack": destination.name,
                "manifest": destination_sidecar.name,
                "sha256": asset_pack.file_sha256(destination),
                "content_fingerprint": manifest["content_fingerprint"],
            }
        )
    catalog = {"schema_version": 1, "asset_schema": ASSET_SCHEMA, "packs": packs}
    write_json(stage_root / "asset-pack-catalog.v1.json", catalog)
    return catalog


def output_and_sidecar(output_value: str, manifest_output: str | None) -> tuple[Path, Path]:
    output = Path(output_value).expanduser()
    return output, asset_pack.sidecar_output(output, manifest_output)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    authored = commands.add_parser("authored", help="Build mclone-authored.pbp")
    authored.add_argument("--runtime-root", default=str(DEFAULT_RUNTIME_ROOT))
    authored.add_argument("--authored-root", default=str(DEFAULT_AUTHORED_ROOT))
    authored.add_argument("--repo-assets-root", default=str(REPO_ROOT / "assets"))
    authored.add_argument("--output", default=str(DEFAULT_AUTHORED_OUTPUT))
    authored.add_argument("--manifest-output")

    fallback = commands.add_parser(
        "generated-fallback", help="Build mclone-generated-fallback.pbp"
    )
    fallback.add_argument("--inventory", default=str(DEFAULT_INVENTORY))
    fallback.add_argument("--repo-assets-root", default=str(REPO_ROOT / "assets"))
    fallback.add_argument("--staging-root", default=str(DEFAULT_FALLBACK_ROOT))
    fallback.add_argument("--font", default=str(FONT_PATH))
    fallback.add_argument("--output", default=str(DEFAULT_FALLBACK_OUTPUT))
    fallback.add_argument("--manifest-output")

    diagnostic = commands.add_parser(
        "diagnostic-missing", help="Build mclone-diagnostic-missing.pbp"
    )
    diagnostic.add_argument("--inventory", default=str(DEFAULT_INVENTORY))
    diagnostic.add_argument("--repo-assets-root", default=str(REPO_ROOT / "assets"))
    diagnostic.add_argument("--staging-root", default=str(DEFAULT_DIAGNOSTIC_ROOT))
    diagnostic.add_argument("--font", default=str(FONT_PATH))
    diagnostic.add_argument("--output", default=str(DEFAULT_DIAGNOSTIC_OUTPUT))
    diagnostic.add_argument("--manifest-output")

    verify = commands.add_parser("verify", help="Verify an existing first-party pack")
    verify.add_argument("pack")
    verify.add_argument("--manifest")

    stage = commands.add_parser("stage", help="Stage canonical first-party release inputs")
    stage.add_argument("--authored", default=str(DEFAULT_AUTHORED_OUTPUT))
    stage.add_argument("--fallback", default=str(DEFAULT_FALLBACK_OUTPUT))
    stage.add_argument("--diagnostic", default=str(DEFAULT_DIAGNOSTIC_OUTPUT))
    stage.add_argument("--stage-root", default=str(DEFAULT_STAGE_ROOT))
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if args.command == "verify":
        manifest = verify_first_party_pack(
            Path(args.pack).expanduser(),
            Path(args.manifest).expanduser() if args.manifest else None,
        )
        print(f"[OK] verified {args.pack} ({manifest['file_count']} files)")
        return 0
    if args.command == "stage":
        catalog = stage_first_party_packs(
            Path(args.authored).expanduser(),
            Path(args.fallback).expanduser(),
            Path(args.diagnostic).expanduser(),
            Path(args.stage_root).expanduser(),
        )
        print(f"[OK] staged {len(catalog['packs'])} packs at {args.stage_root}")
        return 0

    output, sidecar = output_and_sidecar(args.output, args.manifest_output)
    if args.command == "authored":
        manifest = build_authored_pack(
            Path(args.runtime_root).expanduser(),
            Path(args.authored_root).expanduser(),
            Path(args.repo_assets_root).expanduser(),
            output,
            sidecar,
        )
    elif args.command == "generated-fallback":
        inventory_path = Path(args.inventory).expanduser()
        if not inventory_path.is_file():
            raise SystemExit(
                f"{inventory_path} is missing; run `pnpm assets:inventory:first-party` first"
            )
        manifest = build_generated_fallback_pack(
            inventory_path,
            Path(args.repo_assets_root).expanduser(),
            Path(args.staging_root).expanduser(),
            Path(args.font).expanduser(),
            output,
            sidecar,
        )
    else:
        inventory_path = Path(args.inventory).expanduser()
        if not inventory_path.is_file():
            raise SystemExit(
                f"{inventory_path} is missing; run `pnpm assets:inventory:first-party` first"
            )
        manifest = build_diagnostic_missing_pack(
            inventory_path,
            Path(args.repo_assets_root).expanduser(),
            Path(args.staging_root).expanduser(),
            Path(args.font).expanduser(),
            output,
            sidecar,
        )
    verify_first_party_pack(output, sidecar)
    print(f"[OK] wrote {output} ({output.stat().st_size} bytes, {manifest['file_count']} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
