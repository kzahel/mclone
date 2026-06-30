#!/usr/bin/env python3
"""Build and lock the local Minecraft reference asset pack."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import zipfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


PACK_FORMAT_VERSION = 1
LOCK_FORMAT_VERSION = 1
PACK_MANIFEST_PATH = "mclone-pack.json"
DEFAULT_VERSION = "1.17.1"
DEFAULT_PACK_NAME = "extracted.zip"
FIRST_PARTY_ASSET_DIR = "assets"
ZIP_TIMESTAMP = (1980, 1, 1, 0, 0, 0)
TEXT_SUFFIXES = {".json", ".mcmeta", ".txt", ".lang", ".fsh", ".vsh"}
PORTABLE_FINGERPRINT_ID = "portable_json_text_v1"
PAYLOAD_FINGERPRINT_ID = "payload_raw_v1"
TOOL_PATH = "tools/minecraft_assets/asset_pack.py"

REPO_ROOT = Path(__file__).resolve().parents[2]
LOCK_DIR = Path(__file__).resolve().parent / "locks"


@dataclass(frozen=True)
class PackEntry:
    path: str
    source_path: Path
    bytes: int
    sha256: str
    compression: str


def repo_relative(path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return str(resolved)


def reference_dir(version: str) -> Path:
    return REPO_ROOT / "reference" / f"minecraft-{version}"


def asset_dir(version: str) -> Path:
    return reference_dir(version) / "extracted"


def first_party_asset_dir() -> Path:
    return REPO_ROOT / FIRST_PARTY_ASSET_DIR


def asset_set_name(version: str) -> str:
    return f"mclone-game-{version}"


def default_output(version: str) -> Path:
    return reference_dir(version) / DEFAULT_PACK_NAME


def sidecar_output(output: Path, explicit: str | None) -> Path:
    if explicit:
        return Path(explicit).expanduser()
    return Path(f"{output}.json")


def lock_path(asset_set: str) -> Path:
    safe = asset_set.replace("/", "_").replace("\\", "_")
    return LOCK_DIR / f"{safe}.lock.json"


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def canonical_json_bytes(payload: Any) -> bytes:
    return (
        json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode("utf-8")


def canonical_json_sha256(payload: Any) -> str:
    return hashlib.sha256(canonical_json_bytes(payload)).hexdigest()


def normalized_text_bytes(data: bytes) -> bytes:
    return data.replace(b"\r\n", b"\n").replace(b"\r", b"\n")


def read_normalized_text(path: Path) -> bytes:
    return normalized_text_bytes(path.read_bytes())


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def pack_file_bytes(path: Path) -> bytes:
    data = path.read_bytes()
    if path.suffix.lower() in TEXT_SUFFIXES:
        return normalized_text_bytes(data)
    return data


def compression_mode(path: Path) -> str:
    if path.suffix.lower() == ".png":
        return "store"
    return "deflate"


def zip_compression(mode: str) -> int:
    if mode == "store":
        return zipfile.ZIP_STORED
    if mode == "deflate":
        return zipfile.ZIP_DEFLATED
    raise SystemExit(f"Unsupported compression mode: {mode}")


def compression_name(info: zipfile.ZipInfo) -> str:
    if info.compress_type == zipfile.ZIP_STORED:
        return "store"
    if info.compress_type == zipfile.ZIP_DEFLATED:
        return "deflate"
    raise SystemExit(f"Unsupported ZIP compression type {info.compress_type} for {info.filename}")


def zip_info(name: str, mode: str) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, ZIP_TIMESTAMP)
    info.compress_type = zip_compression(mode)
    info.create_system = 3
    info.external_attr = 0o644 << 16
    return info


def validate_pack_path(path: str) -> None:
    if (
        not path
        or path == PACK_MANIFEST_PATH
        or path.startswith("/")
        or path.startswith("\\")
        or "\\" in path
        or any(part in {"", ".."} for part in path.split("/"))
    ):
        raise SystemExit(f"Invalid pack path: {path!r}")


def assert_asset_tree(root: Path) -> None:
    if not root.is_dir():
        raise SystemExit(f"{root} not found. Run ./scripts/decompile-mc.sh first.")
    if not (root / "assets").is_dir():
        raise SystemExit(f"{root}/assets not found. Re-run ./scripts/decompile-mc.sh.")


def collect_files(root: Path) -> list[str]:
    files = []
    for path in root.rglob("*"):
        if path.is_file():
            rel = path.relative_to(root).as_posix()
            validate_pack_path(rel)
            files.append(rel)
    return sorted(files)


def build_entries(root: Path, files: list[str], path_prefix: str = "") -> list[PackEntry]:
    entries = []
    for rel in files:
        source_path = root / rel
        pack_path = f"{path_prefix}/{rel}" if path_prefix else rel
        validate_pack_path(pack_path)
        data = pack_file_bytes(source_path)
        entries.append(
            PackEntry(
                path=pack_path,
                source_path=source_path,
                bytes=len(data),
                sha256=hashlib.sha256(data).hexdigest(),
                compression=compression_mode(source_path),
            )
        )
    return entries


def build_first_party_entries() -> list[PackEntry]:
    root = first_party_asset_dir()
    if not root.is_dir():
        return []
    return build_entries(root, collect_files(root), FIRST_PARTY_ASSET_DIR)


def assert_unique_entries(entries: list[PackEntry]) -> None:
    seen = set()
    for entry in sorted(entries, key=lambda item: item.path):
        if entry.path in seen:
            raise SystemExit(f"Duplicate pack path from source roots: {entry.path}")
        seen.add(entry.path)


def source_root_records(version: str) -> list[dict[str, str]]:
    roots = [
        {
            "name": "minecraft_extracted",
            "root": repo_relative(asset_dir(version)),
            "pack_path_prefix": "",
        }
    ]
    if first_party_asset_dir().is_dir():
        roots.append(
            {
                "name": "mclone_first_party",
                "root": repo_relative(first_party_asset_dir()),
                "pack_path_prefix": FIRST_PARTY_ASSET_DIR,
            }
        )
    return roots


def portable_file_record(path: str, data: bytes) -> dict[str, Any]:
    suffix = Path(path).suffix.lower()
    if suffix == ".json":
        try:
            payload = json.loads(data.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            normalized = normalized_text_bytes(data)
            return {
                "bytes": len(normalized),
                "kind": "text",
                "path": path,
                "sha256": hashlib.sha256(normalized).hexdigest(),
            }
        canonical = canonical_json_bytes(payload)
        return {
            "bytes": len(canonical),
            "kind": "json",
            "path": path,
            "sha256": hashlib.sha256(canonical).hexdigest(),
        }
    if suffix in TEXT_SUFFIXES:
        normalized = normalized_text_bytes(data)
        return {
            "bytes": len(normalized),
            "kind": "text",
            "path": path,
            "sha256": hashlib.sha256(normalized).hexdigest(),
        }
    return {
        "path": path,
        "reason": "binary-or-image-byte-parity-deferred",
        "suffix": suffix or "<none>",
    }


def payload_file_record(path: str, data: bytes) -> dict[str, Any]:
    return {
        "bytes": len(data),
        "path": path,
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def fingerprint_records(entries: list[PackEntry]) -> tuple[dict[str, Any], dict[str, Any]]:
    portable_files = []
    portable_excluded = []
    payload_files = []
    for entry in sorted(entries, key=lambda item: item.path):
        data = pack_file_bytes(entry.source_path)
        portable = portable_file_record(entry.path, data)
        if "sha256" in portable:
            portable_files.append(portable)
        else:
            portable_excluded.append(portable)
        payload_files.append(payload_file_record(entry.path, data))

    return (
        {
            "algorithm": "sha256",
            "excluded_files": portable_excluded,
            "files": portable_files,
            "policy": PORTABLE_FINGERPRINT_ID,
        },
        {
            "algorithm": "sha256",
            "files": payload_files,
            "policy": PAYLOAD_FINGERPRINT_ID,
        },
    )


def portable_fingerprint_summary(manifest: dict[str, Any]) -> dict[str, Any]:
    return {
        "algorithm": "sha256",
        "excluded_file_count": len(manifest.get("excluded_files", [])),
        "fingerprinted_file_count": len(manifest.get("files", [])),
        "sha256": canonical_json_sha256(manifest),
    }


def payload_fingerprint_summary(manifest: dict[str, Any]) -> dict[str, Any]:
    files = manifest.get("files", [])
    return {
        "algorithm": "sha256",
        "bytes": sum(file["bytes"] for file in files),
        "file_count": len(files),
        "sha256": canonical_json_sha256(manifest),
    }


def build_fingerprints(entries: list[PackEntry]) -> dict[str, Any]:
    portable, payload = fingerprint_records(entries)
    return {
        PORTABLE_FINGERPRINT_ID: portable_fingerprint_summary(portable),
        PAYLOAD_FINGERPRINT_ID: payload_fingerprint_summary(payload),
    }


def build_manifest(version: str, entries: list[PackEntry]) -> dict[str, Any]:
    return {
        "format_version": PACK_FORMAT_VERSION,
        "asset_set": asset_set_name(version),
        "minecraft_version": version,
        "tool": TOOL_PATH,
        "source_root": repo_relative(asset_dir(version)),
        "source_roots": source_root_records(version),
        "fingerprints": build_fingerprints(entries),
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


def write_pack(output: Path, sidecar: Path, manifest: dict[str, Any], entries: list[PackEntry]) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    sidecar.parent.mkdir(parents=True, exist_ok=True)
    manifest_bytes = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"

    with zipfile.ZipFile(output, "w") as archive:
        archive.writestr(zip_info(PACK_MANIFEST_PATH, "deflate"), manifest_bytes)
        for entry in sorted(entries, key=lambda item: item.path):
            archive.writestr(
                zip_info(entry.path, entry.compression),
                pack_file_bytes(entry.source_path),
            )

    sidecar.write_bytes(manifest_bytes)


def read_json(path: Path) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            payload = json.load(handle)
    except FileNotFoundError:
        raise SystemExit(f"Required JSON file is missing: {path}") from None
    except json.JSONDecodeError as error:
        raise SystemExit(f"Could not parse JSON file {path}: {error}") from None
    if not isinstance(payload, dict):
        raise SystemExit(f"Expected JSON object in {path}")
    return payload


def verify_pack(path: Path, sidecar: Path | None = None) -> dict[str, Any]:
    with zipfile.ZipFile(path, "r") as archive:
        bad = archive.testzip()
        if bad:
            raise SystemExit(f"Pack CRC verification failed for {bad}")
        manifest = json.loads(archive.read(PACK_MANIFEST_PATH).decode("utf-8"))
        names = set(archive.namelist())
        declared = {PACK_MANIFEST_PATH}
        for item in manifest.get("files", []):
            name = item["path"]
            declared.add(name)
            if name not in names:
                raise SystemExit(f"Pack manifest file is missing from archive: {name}")
            info = archive.getinfo(name)
            if info.file_size != item["bytes"]:
                raise SystemExit(
                    f"Pack file size mismatch for {name}: {info.file_size} != {item['bytes']}"
                )
            if compression_name(info) != item["compression"]:
                raise SystemExit(f"Pack compression mismatch for {name}")
            digest = hashlib.sha256(archive.read(name)).hexdigest()
            if digest != item["sha256"]:
                raise SystemExit(f"Pack SHA-256 mismatch for {name}")
        extra = sorted(names - declared)
        if extra:
            raise SystemExit(f"Pack contains undeclared file: {extra[0]}")

    if sidecar is not None:
        sidecar_manifest = read_json(sidecar)
        if sidecar_manifest != manifest:
            raise SystemExit(f"Pack sidecar manifest does not match {PACK_MANIFEST_PATH}: {sidecar}")
    return manifest


def pack_fingerprint_manifests(path: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    with zipfile.ZipFile(path, "r") as archive:
        manifest = json.loads(archive.read(PACK_MANIFEST_PATH).decode("utf-8"))
        portable_files = []
        portable_excluded = []
        payload_files = []
        for item in sorted(manifest.get("files", []), key=lambda entry: entry["path"]):
            name = item["path"]
            data = archive.read(name)
            portable = portable_file_record(name, data)
            if "sha256" in portable:
                portable_files.append(portable)
            else:
                portable_excluded.append(portable)
            payload_files.append(payload_file_record(name, data))
    return (
        {
            "algorithm": "sha256",
            "excluded_files": portable_excluded,
            "files": portable_files,
            "policy": PORTABLE_FINGERPRINT_ID,
        },
        {
            "algorithm": "sha256",
            "files": payload_files,
            "policy": PAYLOAD_FINGERPRINT_ID,
        },
    )


def tool_fingerprint() -> dict[str, Any]:
    path = REPO_ROOT / TOOL_PATH
    data = read_normalized_text(path)
    return {
        "algorithm": "sha256",
        "files": [
            {
                "path": TOOL_PATH,
                "normalized_bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            }
        ],
        "sha256": canonical_json_sha256(
            [
                {
                    "path": TOOL_PATH,
                    "normalized_bytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest(),
                }
            ]
        ),
    }


def build_source_record(version: str, entries: list[PackEntry], *, full: bool) -> dict[str, Any]:
    portable, payload = fingerprint_records(entries)
    record: dict[str, Any] = {
        "root": repo_relative(asset_dir(version)),
        "roots": source_root_records(version),
        "file_count": len(entries),
    }
    if full:
        record["fingerprints"] = {
            PORTABLE_FINGERPRINT_ID: portable_fingerprint_summary(portable),
            PAYLOAD_FINGERPRINT_ID: payload_fingerprint_summary(payload),
        }
    return record


def build_pack_record(
    version: str,
    output: Path,
    sidecar: Path,
    *,
    full: bool,
) -> dict[str, Any]:
    manifest = verify_pack(output, sidecar)
    record: dict[str, Any] = {
        "path": repo_relative(output),
        "manifest_path": repo_relative(sidecar),
        "asset_set": manifest.get("asset_set"),
        "file_count": manifest.get("file_count"),
    }
    if full:
        portable, payload = pack_fingerprint_manifests(output)
        record["fingerprints"] = {
            PORTABLE_FINGERPRINT_ID: portable_fingerprint_summary(portable),
            PAYLOAD_FINGERPRINT_ID: payload_fingerprint_summary(payload),
        }
        record["content_manifest_sha256"] = record["fingerprints"][PORTABLE_FINGERPRINT_ID][
            "sha256"
        ]
        record["payload_raw_manifest"] = payload
    if record["asset_set"] != asset_set_name(version):
        raise SystemExit(
            f"Pack asset_set {record['asset_set']!r} does not match {asset_set_name(version)!r}"
        )
    return record


def build_lock(version: str, output: Path, sidecar: Path, entries: list[PackEntry]) -> dict[str, Any]:
    source = build_source_record(version, entries, full=True)
    pack = build_pack_record(version, output, sidecar, full=True)
    source_payload = source["fingerprints"][PAYLOAD_FINGERPRINT_ID]["sha256"]
    pack_payload = pack["fingerprints"][PAYLOAD_FINGERPRINT_ID]["sha256"]
    if source_payload != pack_payload:
        raise SystemExit("Pack raw payload fingerprint differs from the extracted source tree")
    return {
        "format_version": LOCK_FORMAT_VERSION,
        "asset_set": asset_set_name(version),
        "minecraft_version": version,
        "generated_at": utc_now(),
        "tool": TOOL_PATH,
        "tool_fingerprint": tool_fingerprint(),
        "source": source,
        "pack": pack,
    }


def write_lock(version: str, output: Path, sidecar: Path, entries: list[PackEntry]) -> int:
    lock = build_lock(version, output, sidecar, entries)
    path = lock_path(asset_set_name(version))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json.dumps(lock, indent=2, sort_keys=True).encode("utf-8") + b"\n")
    print(f"[OK] wrote {repo_relative(path)}")
    return 0


def check_lock(version: str, output: Path, sidecar: Path, entries: list[PackEntry]) -> int:
    path = lock_path(asset_set_name(version))
    lock = read_json(path)
    errors = []

    if lock.get("format_version") != LOCK_FORMAT_VERSION:
        errors.append(f"Unsupported lock format_version {lock.get('format_version')!r}")
    if lock.get("asset_set") != asset_set_name(version):
        errors.append(f"Lock asset_set is {lock.get('asset_set')!r}")
    if lock.get("minecraft_version") != version:
        errors.append(f"Lock minecraft_version is {lock.get('minecraft_version')!r}")
    if lock.get("tool") != TOOL_PATH:
        errors.append(f"Lock tool is {lock.get('tool')!r}, expected {TOOL_PATH!r}")
    if lock.get("tool_fingerprint") != tool_fingerprint():
        errors.append("Asset pack tooling changed since the lock was written")

    current_source = build_source_record(version, entries, full=True)
    if lock.get("source", {}).get("file_count") != current_source["file_count"]:
        errors.append("Extracted source file count differs from the lock")
    if lock.get("source", {}).get("fingerprints") != current_source["fingerprints"]:
        errors.append("Extracted source fingerprints differ from the lock")

    current_pack = build_pack_record(version, output, sidecar, full=True)
    locked_pack = lock.get("pack", {})
    if locked_pack.get("asset_set") != current_pack["asset_set"]:
        errors.append("Pack asset_set differs from the lock")
    if locked_pack.get("file_count") != current_pack["file_count"]:
        errors.append("Pack file count differs from the lock")
    if locked_pack.get("fingerprints") != current_pack["fingerprints"]:
        errors.append("Pack fingerprints differ from the lock")

    if errors:
        print(f"[FAIL] asset lock is stale for {asset_set_name(version)}", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        print(
            "Rerun `pnpm assets:pack` and inspect the result, then run "
            "`pnpm assets:pack:write-lock`.",
            file=sys.stderr,
        )
        return 1

    print(f"[OK] asset lock is current for {asset_set_name(version)}")
    return 0


def build_pack(version: str, output: Path, sidecar: Path, entries: list[PackEntry], dry_run: bool) -> int:
    manifest = build_manifest(version, entries)
    print(f"Asset set: {asset_set_name(version)}")
    for root_record in source_root_records(version):
        print(
            "Source root: "
            f"{root_record['name']}={root_record['root']} "
            f"(pack prefix {root_record['pack_path_prefix'] or '<root>'})"
        )
    print(f"Packed files: {len(entries)}")
    print(f"Output: {repo_relative(output)}")
    print(f"Sidecar manifest: {repo_relative(sidecar)}")
    if dry_run:
        return 0
    write_pack(output, sidecar, manifest, entries)
    verified = verify_pack(output, sidecar)
    print(f"[OK] wrote {repo_relative(output)} ({output.stat().st_size} bytes, {verified['file_count']} files)")
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", default=DEFAULT_VERSION)
    parser.add_argument("--output", help="Output ZIP/.pbp path")
    parser.add_argument("--manifest-output", help="Sidecar manifest path")
    parser.add_argument("--dry-run", action="store_true", help="Validate and print the pack plan")
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--verify-only", action="store_true", help="Verify existing pack and sidecar")
    action.add_argument("--check-lock", action="store_true", help="Check the checked lockfile")
    action.add_argument("--write-lock", action="store_true", help="Refresh the checked lockfile")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = asset_dir(args.version)
    output = Path(args.output).expanduser() if args.output else default_output(args.version)
    sidecar = sidecar_output(output, args.manifest_output)

    if args.verify_only:
        manifest = verify_pack(output, sidecar)
        print(f"[OK] verified {repo_relative(output)} ({manifest['file_count']} files)")
        return 0

    assert_asset_tree(root)
    files = collect_files(root)
    if not files:
        raise SystemExit(f"{root} did not contain any files")
    entries = build_entries(root, files)
    entries.extend(build_first_party_entries())
    assert_unique_entries(entries)

    if args.check_lock:
        return check_lock(args.version, output, sidecar, entries)
    if args.write_lock:
        return write_lock(args.version, output, sidecar, entries)
    return build_pack(args.version, output, sidecar, entries, args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
