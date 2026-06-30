#!/usr/bin/env python3
"""Build a first-party overlay asset pack from an assets/ tree."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import asset_pack


DEFAULT_ASSET_SET = "mclone-default-overlay"
DEFAULT_SOURCE_ROOT = Path("/tmp/mclone-texture-lab/runtime-pack")
DEFAULT_OUTPUT = Path("/tmp/mclone-texture-lab/mclone-default-overlay.pbp")
TOOL_PATH = "tools/minecraft_assets/overlay_pack.py"


def assert_overlay_tree(root: Path) -> None:
    if not root.is_dir():
        raise SystemExit(
            f"{root} not found. Run `pnpm texture-lab:runtime-compat` first."
        )
    if not (root / "assets").is_dir():
        raise SystemExit(f"{root}/assets not found; overlay roots must contain assets/.")


def build_overlay_manifest(
    asset_set: str,
    source_root: Path,
    entries: list[asset_pack.PackEntry],
) -> dict[str, Any]:
    if not asset_set.strip():
        raise SystemExit("--asset-set must not be empty")
    return {
        "format_version": asset_pack.PACK_FORMAT_VERSION,
        "asset_set": asset_set,
        "source_kind": "first_party_overlay",
        "tool": TOOL_PATH,
        "source_root": asset_pack.repo_relative(source_root),
        "source_roots": [
            {
                "name": "first_party_overlay",
                "root": asset_pack.repo_relative(source_root),
                "pack_path_prefix": "",
            }
        ],
        "fingerprints": asset_pack.build_fingerprints(entries),
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


def build_entries(source_root: Path) -> list[asset_pack.PackEntry]:
    assert_overlay_tree(source_root)
    files = asset_pack.collect_files(source_root)
    if not files:
        raise SystemExit(f"{source_root} did not contain any files")
    entries = asset_pack.build_entries(source_root, files)
    asset_pack.assert_unique_entries(entries)
    return entries


def build_pack(
    asset_set: str,
    source_root: Path,
    output: Path,
    sidecar: Path,
    dry_run: bool,
) -> int:
    entries = build_entries(source_root)
    manifest = build_overlay_manifest(asset_set, source_root, entries)
    print(f"Asset set: {asset_set}")
    print(f"Source root: {asset_pack.repo_relative(source_root)}")
    print(f"Packed files: {len(entries)}")
    print(f"Output: {asset_pack.repo_relative(output)}")
    print(f"Sidecar manifest: {asset_pack.repo_relative(sidecar)}")
    if dry_run:
        return 0
    asset_pack.write_pack(output, sidecar, manifest, entries)
    verified = asset_pack.verify_pack(output, sidecar)
    print(
        f"[OK] wrote {asset_pack.repo_relative(output)} "
        f"({output.stat().st_size} bytes, {verified['file_count']} files)"
    )
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-root",
        default=str(DEFAULT_SOURCE_ROOT),
        help="Input first-party overlay root containing assets/ "
        f"(default: {DEFAULT_SOURCE_ROOT})",
    )
    parser.add_argument(
        "--asset-set",
        default=DEFAULT_ASSET_SET,
        help=f"Manifest asset_set name (default: {DEFAULT_ASSET_SET})",
    )
    parser.add_argument(
        "--output",
        default=str(DEFAULT_OUTPUT),
        help=f"Output .pbp/.zip path (default: {DEFAULT_OUTPUT})",
    )
    parser.add_argument("--manifest-output", help="Sidecar manifest path")
    parser.add_argument("--dry-run", action="store_true", help="Validate and print the pack plan")
    parser.add_argument("--verify-only", action="store_true", help="Verify existing pack and sidecar")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    source_root = Path(args.source_root).expanduser()
    output = Path(args.output).expanduser()
    sidecar = asset_pack.sidecar_output(output, args.manifest_output)

    if args.verify_only:
        manifest = asset_pack.verify_pack(output, sidecar)
        print(f"[OK] verified {asset_pack.repo_relative(output)} ({manifest['file_count']} files)")
        return 0

    return build_pack(args.asset_set, source_root, output, sidecar, args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
