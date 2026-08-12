#!/usr/bin/env python3
"""Import and verify Mclone's allowlisted first-party CC0 sound bank."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import zipfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
ASSET_ROOT = REPO_ROOT / "assets"
SOUND_ROOT = ASSET_ROOT / "mclone/sounds/kenney"
LOCAL_SOUND_ROOT = ASSET_ROOT / "mclone/sounds/mclone-original"
AUDIO_ROOT = ASSET_ROOT / "mclone/audio"
CATALOG_PATH = AUDIO_ROOT / "sound-bank.v1.json"
PROVENANCE_PATH = AUDIO_ROOT / "provenance.v1.json"
LICENSE_ROOT = AUDIO_ROOT / "licenses/kenney"

ARCHIVES = {
    "impact": {
        "filename": "impact.zip",
        "title": "Impact Sounds",
        "asset_page": "https://kenney.nl/assets/impact-sounds",
        "archive_url": (
            "https://kenney.nl/media/pages/assets/impact-sounds/"
            "87b4ddecda-1677589768/kenney_impact-sounds.zip"
        ),
        "sha256": "029d734af1582474edf3a694d1b0cebc97c1c152f2f39fa34d4c2bafc5de77f8",
        "license_sha256": "b49aa9c56b04528b95913de13e506a0f7c5e807b9925db9bfef86af1f91120db",
    },
    "rpg": {
        "filename": "rpg.zip",
        "title": "RPG Audio",
        "asset_page": "https://kenney.nl/assets/rpg-audio",
        "archive_url": (
            "https://kenney.nl/media/pages/assets/rpg-audio/"
            "8e99002d76-1677590336/kenney_rpg-audio.zip"
        ),
        "sha256": "6dbeaf8544da958d8f2adcb4a4a4b76c1ade34a05f8ab9edccd327da7375f38b",
        "license_sha256": "5735dfd72cb64cbbceda4ebc00c380c41ca680edb82ff153aa7c9ab97614c539",
    },
    "interface": {
        "filename": "interface.zip",
        "title": "Interface Sounds",
        "asset_page": "https://kenney.nl/assets/interface-sounds",
        "archive_url": (
            "https://kenney.nl/media/pages/assets/interface-sounds/"
            "fa43c1dd4d-1677589452/kenney_interface-sounds.zip"
        ),
        "sha256": "f2193d072726d6758a5f7871b2dcc54dcce0d5c35c6f0a62f92549b327c81232",
        "license_sha256": "f7966c773bbed0eca6a9c75081c44a178b38eae112724dbb5fdfbd4192d118a9",
    },
}


def numbered(prefix: str, count: int, width: int = 3) -> list[str]:
    return [f"{prefix}{index:0{width}d}.ogg" for index in range(count)]


IMPACT_FILES = sorted(
    filename
    for prefix in (
        "footstep_carpet_",
        "footstep_concrete_",
        "footstep_grass_",
        "footstep_snow_",
        "footstep_wood_",
        "impactGeneric_light_",
        "impactGlass_heavy_",
        "impactGlass_light_",
        "impactGlass_medium_",
        "impactMetal_heavy_",
        "impactMetal_light_",
        "impactMetal_medium_",
        "impactMining_",
        "impactPlank_medium_",
        "impactSoft_heavy_",
        "impactSoft_medium_",
        "impactWood_heavy_",
        "impactWood_light_",
        "impactWood_medium_",
    )
    for filename in numbered(prefix, 5)
)
RPG_FILES = sorted(
    numbered("footstep", 10, 2)
    + [f"cloth{index}.ogg" for index in range(1, 5)]
    + ["handleCoins.ogg", "handleCoins2.ogg"]
    + [f"creak{index}.ogg" for index in range(1, 4)]
)
INTERFACE_FILES = sorted(
    [
        "back_001.ogg",
        "confirmation_001.ogg",
        "error_001.ogg",
        "open_001.ogg",
        "select_001.ogg",
    ]
)
SELECTED_FILES = {
    "impact": IMPACT_FILES,
    "rpg": RPG_FILES,
    "interface": INTERFACE_FILES,
}
LOCAL_FILES = [
    "deer_alarm_00.ogg",
    "deer_alarm_01.ogg",
    "deer_contact_00.ogg",
    "deer_contact_01.ogg",
    "deer_impact_00.ogg",
    "deer_impact_01.ogg",
    "mallard_call_00.ogg",
    "mallard_call_01.ogg",
]


def family(
    key: str,
    pack: str,
    filenames: list[str],
    gain: float,
    pitch_min: float,
    pitch_max: float,
) -> dict[str, Any]:
    return {
        "key": key,
        "gain": gain,
        "pitch_min": pitch_min,
        "pitch_max": pitch_max,
        "no_immediate_repeat": len(filenames) > 1,
        "variants": [asset_path(pack, filename) for filename in filenames],
    }


def asset_path(pack: str, filename: str) -> str:
    return f"assets/mclone/sounds/kenney/{pack}/{filename}"


FAMILIES = [
    family("mclone:footstep_carpet", "impact", numbered("footstep_carpet_", 5), 0.45, 0.94, 1.06),
    family("mclone:footstep_grass", "impact", numbered("footstep_grass_", 5), 0.46, 0.94, 1.06),
    family("mclone:footstep_neutral", "rpg", numbered("footstep", 10, 2), 0.38, 0.94, 1.06),
    family("mclone:footstep_snow", "impact", numbered("footstep_snow_", 5), 0.43, 0.94, 1.06),
    family("mclone:footstep_stone", "impact", numbered("footstep_concrete_", 5), 0.42, 0.94, 1.06),
    family("mclone:footstep_wood", "impact", numbered("footstep_wood_", 5), 0.44, 0.94, 1.06),
    family("mclone:landing_carpet", "impact", numbered("impactSoft_medium_", 5), 0.58, 0.90, 1.02),
    family("mclone:landing_grass", "impact", numbered("impactSoft_medium_", 5), 0.58, 0.90, 1.02),
    family(
        "mclone:landing_neutral",
        "impact",
        numbered("impactGeneric_light_", 5),
        0.52,
        0.90,
        1.02,
    ),
    family("mclone:landing_snow", "impact", numbered("impactSoft_medium_", 5), 0.54, 0.90, 1.02),
    family("mclone:landing_stone", "impact", numbered("impactGeneric_light_", 5), 0.56, 0.90, 1.02),
    family("mclone:landing_wood", "impact", numbered("impactPlank_medium_", 5), 0.56, 0.90, 1.02),
    family("mclone:break_glass", "impact", numbered("impactGlass_heavy_", 5), 0.64, 0.94, 1.06),
    family("mclone:break_metal", "impact", numbered("impactMetal_heavy_", 5), 0.60, 0.94, 1.06),
    family("mclone:break_soft", "impact", numbered("impactSoft_heavy_", 5), 0.62, 0.94, 1.06),
    family("mclone:break_stone", "impact", numbered("impactMining_", 5), 0.64, 0.94, 1.06),
    family("mclone:break_wood", "impact", numbered("impactWood_heavy_", 5), 0.62, 0.94, 1.06),
    family("mclone:place_glass", "impact", numbered("impactGlass_light_", 5), 0.45, 0.96, 1.06),
    family("mclone:place_metal", "impact", numbered("impactMetal_light_", 5), 0.46, 0.96, 1.06),
    family("mclone:place_soft", "impact", numbered("impactSoft_medium_", 5), 0.45, 0.96, 1.06),
    family("mclone:place_stone", "impact", numbered("impactGeneric_light_", 5), 0.45, 0.96, 1.06),
    family("mclone:place_wood", "impact", numbered("impactWood_light_", 5), 0.46, 0.96, 1.06),
    family("mclone:impact_glass", "impact", numbered("impactGlass_medium_", 5), 0.55, 0.94, 1.06),
    family("mclone:impact_metal", "impact", numbered("impactMetal_medium_", 5), 0.55, 0.94, 1.06),
    family("mclone:impact_wood", "impact", numbered("impactWood_medium_", 5), 0.55, 0.94, 1.06),
    family(
        "mclone:cloth_move",
        "rpg",
        [f"cloth{index}.ogg" for index in range(1, 5)],
        0.40,
        0.97,
        1.03,
    ),
    family("mclone:item_pickup", "rpg", ["handleCoins.ogg", "handleCoins2.ogg"], 0.52, 0.97, 1.05),
    {
        "key": "mclone:mallard_call",
        "gain": 0.72,
        "pitch_min": 0.96,
        "pitch_max": 1.04,
        "no_immediate_repeat": True,
        "variants": [
            f"assets/mclone/sounds/mclone-original/{filename}"
            for filename in LOCAL_FILES
        ],
    },
    {
        "key": "mclone:deer_contact",
        "gain": 0.58,
        "pitch_min": 0.96,
        "pitch_max": 1.04,
        "no_immediate_repeat": True,
        "variants": [
            f"assets/mclone/sounds/mclone-original/deer_contact_{index:02}.ogg"
            for index in range(2)
        ],
    },
    {
        "key": "mclone:deer_alarm",
        "gain": 0.74,
        "pitch_min": 0.97,
        "pitch_max": 1.03,
        "no_immediate_repeat": True,
        "variants": [
            f"assets/mclone/sounds/mclone-original/deer_alarm_{index:02}.ogg"
            for index in range(2)
        ],
    },
    {
        "key": "mclone:deer_impact",
        "gain": 0.62,
        "pitch_min": 0.94,
        "pitch_max": 1.02,
        "no_immediate_repeat": True,
        "variants": [
            f"assets/mclone/sounds/mclone-original/deer_impact_{index:02}.ogg"
            for index in range(2)
        ],
    },
    family(
        "mclone:wood_creak",
        "rpg",
        [f"creak{index}.ogg" for index in range(1, 4)],
        0.52,
        0.96,
        1.04,
    ),
    family("mclone:ui_back", "interface", ["back_001.ogg"], 0.36, 1.0, 1.0),
    family("mclone:ui_confirm", "interface", ["confirmation_001.ogg"], 0.36, 1.0, 1.0),
    family("mclone:ui_error", "interface", ["error_001.ogg"], 0.38, 1.0, 1.0),
    family("mclone:ui_open", "interface", ["open_001.ogg"], 0.34, 1.0, 1.0),
    family("mclone:ui_select", "interface", ["select_001.ogg"], 0.30, 1.0, 1.0),
]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_json_bytes(payload: Any) -> bytes:
    return json.dumps(payload, indent=2, sort_keys=True).encode("utf-8") + b"\n"


def catalog_payload() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "license": "CC0-1.0",
        "selected_file_count": len(expected_sound_paths()),
        "families": sorted(FAMILIES, key=lambda entry: entry["key"]),
    }


def family_uses() -> dict[str, list[str]]:
    uses: dict[str, list[str]] = {}
    for entry in FAMILIES:
        for path in entry["variants"]:
            uses.setdefault(path, []).append(entry["key"])
    return {path: sorted(keys) for path, keys in uses.items()}


def provenance_payload() -> dict[str, Any]:
    uses = family_uses()
    archives = []
    files = []
    for pack, details in ARCHIVES.items():
        license_path = f"assets/mclone/audio/licenses/kenney/{pack}-License.txt"
        archives.append(
            {
                "source_pack": pack,
                "title": details["title"],
                "asset_page": details["asset_page"],
                "archive_url": details["archive_url"],
                "archive_sha256": details["sha256"],
                "license": "CC0-1.0",
                "license_path": license_path,
                "license_sha256": details["license_sha256"],
                "selected_file_count": len(SELECTED_FILES[pack]),
            }
        )
        for filename in SELECTED_FILES[pack]:
            path = asset_path(pack, filename)
            source = REPO_ROOT / path
            files.append(
                {
                    "source_pack": pack,
                    "upstream_path": f"Audio/{filename}",
                    "path": path,
                    "bytes": source.stat().st_size,
                    "sha256": sha256_file(source),
                    "families": uses.get(path, []),
                }
            )
    for filename in LOCAL_FILES:
        path = f"assets/mclone/sounds/mclone-original/{filename}"
        source = REPO_ROOT / path
        files.append(
            {
                "source_pack": "mclone-original",
                "upstream_path": (
                    "tools/minecraft_assets/generate_mallard_calls.sh"
                    if filename.startswith("mallard_")
                    else "tools/minecraft_assets/generate_deer_sounds.sh"
                ),
                "path": path,
                "bytes": source.stat().st_size,
                "sha256": sha256_file(source),
                "families": uses.get(path, []),
            }
        )
    return {
        "schema_version": 1,
        "license": "CC0-1.0",
        "selected_file_count": len(files),
        "archives": sorted(archives, key=lambda entry: entry["source_pack"]),
        "files": sorted(files, key=lambda entry: entry["path"]),
    }


def expected_sound_paths() -> set[str]:
    return {
        asset_path(pack, filename)
        for pack, filenames in SELECTED_FILES.items()
        for filename in filenames
    } | {
        f"assets/mclone/sounds/mclone-original/{filename}"
        for filename in LOCAL_FILES
    }


def validate_static_contract() -> None:
    selected_count = sum(len(files) for files in SELECTED_FILES.values())
    if len(IMPACT_FILES) != 95 or len(RPG_FILES) != 19 or len(INTERFACE_FILES) != 5:
        raise SystemExit("Kenney source-pack selection counts changed")
    if selected_count != 119 or len(expected_sound_paths()) != 119 + len(LOCAL_FILES):
        raise SystemExit(
            f"Sound selection must contain 119 Kenney and {len(LOCAL_FILES)} original files"
        )
    selected = expected_sound_paths()
    referenced = {
        path
        for entry in FAMILIES
        for path in entry["variants"]
    }
    missing = sorted(selected - referenced)
    unexpected = sorted(referenced - selected)
    if missing or unexpected:
        raise SystemExit(
            f"Catalog/selection mismatch; unused={missing}, unselected={unexpected}"
        )
    keys = [entry["key"] for entry in FAMILIES]
    if len(keys) != len(set(keys)):
        raise SystemExit("Sound family keys must be unique")


def import_archives(archive_dir: Path) -> None:
    validate_static_contract()
    archive_dir = archive_dir.resolve()
    for pack, details in ARCHIVES.items():
        archive_path = archive_dir / str(details["filename"])
        if not archive_path.is_file():
            raise SystemExit(f"Missing pinned archive: {archive_path}")
        digest = sha256_file(archive_path)
        if digest != details["sha256"]:
            raise SystemExit(
                f"Archive hash mismatch for {archive_path}: {digest}, "
                f"expected {details['sha256']}"
            )
        with zipfile.ZipFile(archive_path) as archive:
            license_bytes = archive.read("License.txt")
            if sha256_bytes(license_bytes) != details["license_sha256"]:
                raise SystemExit(f"License drift in {archive_path}")
            license_path = LICENSE_ROOT / f"{pack}-License.txt"
            license_path.parent.mkdir(parents=True, exist_ok=True)
            license_path.write_bytes(license_bytes)
            for filename in SELECTED_FILES[pack]:
                payload = archive.read(f"Audio/{filename}")
                if not payload.startswith(b"OggS"):
                    raise SystemExit(f"Selected input is not an OGG stream: {filename}")
                destination = SOUND_ROOT / pack / filename
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(payload)

    AUDIO_ROOT.mkdir(parents=True, exist_ok=True)
    CATALOG_PATH.write_bytes(canonical_json_bytes(catalog_payload()))
    PROVENANCE_PATH.write_bytes(canonical_json_bytes(provenance_payload()))
    validate_repo()


def validate_repo() -> None:
    validate_static_contract()
    actual = {
        str(path.relative_to(REPO_ROOT))
        for root in (SOUND_ROOT, LOCAL_SOUND_ROOT)
        for path in root.rglob("*.ogg")
        if path.is_file()
    }
    expected = expected_sound_paths()
    if actual != expected:
        raise SystemExit(
            f"Checked-in OGG set mismatch; missing={sorted(expected - actual)}, "
            f"unexpected={sorted(actual - expected)}"
        )
    for relative in sorted(expected):
        if not (REPO_ROOT / relative).read_bytes().startswith(b"OggS"):
            raise SystemExit(f"Checked-in file is not an OGG stream: {relative}")

    for pack, details in ARCHIVES.items():
        path = LICENSE_ROOT / f"{pack}-License.txt"
        if not path.is_file() or sha256_file(path) != details["license_sha256"]:
            raise SystemExit(f"Missing or changed pinned license: {path}")

    expected_catalog = canonical_json_bytes(catalog_payload())
    if not CATALOG_PATH.is_file() or CATALOG_PATH.read_bytes() != expected_catalog:
        raise SystemExit(f"Generated sound catalog is stale: {CATALOG_PATH}")
    expected_provenance = canonical_json_bytes(provenance_payload())
    if (
        not PROVENANCE_PATH.is_file()
        or PROVENANCE_PATH.read_bytes() != expected_provenance
    ):
        raise SystemExit(f"Generated sound provenance is stale: {PROVENANCE_PATH}")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check", help="Verify checked-in audio and provenance")
    commands.add_parser(
        "refresh-local", help="Refresh manifests after regenerating local CC0 sounds"
    )
    importer = commands.add_parser("import", help="Import the three pinned archives")
    importer.add_argument(
        "--archive-dir",
        required=True,
        help="Directory containing impact.zip, rpg.zip, and interface.zip",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if args.command == "import":
        import_archives(Path(args.archive_dir).expanduser())
        print(f"[OK] imported 119 Kenney CC0 OGG files into {SOUND_ROOT}")
    elif args.command == "refresh-local":
        validate_static_contract()
        CATALOG_PATH.write_bytes(canonical_json_bytes(catalog_payload()))
        PROVENANCE_PATH.write_bytes(canonical_json_bytes(provenance_payload()))
        validate_repo()
        print(
            f"[OK] refreshed manifests for {len(LOCAL_FILES)} "
            "Mclone-original CC0 sounds"
        )
    else:
        validate_repo()
        print("[OK] verified 121 first-party CC0 OGG files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
