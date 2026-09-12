#!/usr/bin/env python3
"""Check every shipped file against the public asset boundary.

Pack bytes must match the separately built first-party stage. Namespace names
inside those packs are compatibility identifiers, not provenance evidence.
"""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]
PACKS = ("mclone-authored.pbp", "mclone-generated-fallback.pbp", "mclone-diagnostic-missing.pbp")
STAGE = ROOT / "generated-assets/first-party-stage/first-party-packs"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def check(directory, stage=STAGE):
    approved = {name: digest((stage / name).read_bytes()) for name in PACKS}
    media = set()
    for name in PACKS:
        with zipfile.ZipFile(stage / name) as pack:
            for entry in pack.infolist():
                if not entry.is_dir():
                    media.add(digest(pack.read(entry)))
    # Original source art and generated Texture Lab previews are also used by
    # the hosted authoring tools. Never consult reference/ or attachment trees.
    for root in (ROOT / "assets/mclone", ROOT / "tools/texture-lab/packs",
                 ROOT / "generated-assets/texture-lab",
                 ROOT / "tools/asset-lab/dist/catalog-public",
                 ROOT / "tools/structure-lab/dist/catalog-public",
                 ROOT / "tools/texture-lab/dist/web/media"):
        if root.exists():
            for path in root.rglob("*"):
                if path.is_file() and path.suffix.lower() in {".png", ".ogg", ".jpg", ".webp", ".svg"}:
                    media.add(digest(path.read_bytes()))
    inventory = []
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"public output contains a symlink: {path}")
        if not path.is_file():
            continue
        relative = path.relative_to(directory).as_posix()
        lower = relative.lower()
        if any(part in {"reference", ".attachments", "oracle", "local-sounds", "sound-overlay"}
               for part in Path(lower).parts) or "extracted.zip" in lower:
            raise ValueError(f"local reference/development file in public output: {relative}")
        data = path.read_bytes()
        sha = digest(data)
        if path.suffix.lower() == ".pbp":
            if approved.get(path.name) != sha:
                raise ValueError(f"unapproved or altered asset pack: {relative}")
        elif data.startswith(b"PK\x03\x04") or path.suffix.lower() in {".zip", ".jar", ".mcpack", ".ttf", ".otf"}:
            raise ValueError(f"unapproved archive/font in public output: {relative}")
        elif (path.suffix.lower() in {".png", ".ogg", ".jpg", ".jpeg", ".webp", ".svg"}
              or data.startswith((b"\x89PNG\r\n", b"OggS", b"\xff\xd8\xff"))
              or (data.startswith(b"RIFF") and data[8:12] == b"WEBP")):
            if sha not in media:
                raise ValueError(f"media has no approved first-party input: {relative}")
        inventory.append({"path": relative, "bytes": len(data), "sha256": sha})
    found = {Path(item["path"]).name for item in inventory}
    if not set(PACKS).issubset(found):
        raise ValueError("public output must include all three approved asset packs")
    return {"schema_version": 1, "packs": approved, "files": inventory}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--stage", type=Path, default=STAGE)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report = check(args.directory, args.stage)
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Public asset boundary passed: {len(report['files'])} files, three approved packs")
