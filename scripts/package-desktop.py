#!/usr/bin/env python3
"""Bundle an already-built desktop client and the approved CI asset stage."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--platform", required=True, choices=["linux-x64", "linux-arm64", "windows-x64", "macos-arm64"])
parser.add_argument("--profile", default="release")
parser.add_argument("--output", type=Path, default=ROOT / "dist-release")
args = parser.parse_args()
name = "mclone-" + args.platform
destination = args.output / name
if destination.exists():
    shutil.rmtree(destination)
destination.mkdir(parents=True)
resources = destination
binary_root = destination
executable = "mclone-native-client" + (".exe" if args.platform.startswith("windows") else "")
if args.platform.startswith("macos"):
    contents = destination / "Mclone.app/Contents"
    binary_root = contents / "MacOS"
    resources = contents / "Resources"
    contents.mkdir(parents=True)
    (contents / "Info.plist").write_bytes(plistlib.dumps({
        "CFBundleName": "Mclone", "CFBundleDisplayName": "Mclone",
        "CFBundleIdentifier": "com.kzahel.mclone", "CFBundleExecutable": executable,
        "CFBundlePackageType": "APPL", "CFBundleVersion": "1",
        "CFBundleShortVersionString": "0.0.0", "NSHighResolutionCapable": True,
    }))
binary_root.mkdir(parents=True, exist_ok=True)
shutil.copy2(ROOT / "native/target" / args.profile / executable, binary_root / executable)
shutil.copytree(ROOT / "generated-assets/first-party-stage/first-party-packs", resources / "assets/packs")
subprocess.run([sys.executable, str(ROOT / "scripts/check-public-assets.py"), str(destination),
                "--report", str(destination / "public-files.json")], check=True)
revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
(destination / "BUILD.json").write_text(json.dumps({"revision": revision, "platform": args.platform,
    "profile": args.profile, "experimental": True, "project_license": "TBD"}, indent=2) + "\n")
(destination / "START-HERE.txt").write_text(
    "Mclone is a WIP voxel game experiment. Worlds and formats may change.\n"
    "Run Mclone.app on macOS or mclone-native-client on Linux/Windows.\n"
    "Keep the complete extracted folder together. --xr uses an installed OpenXR runtime.\n"
    "Desktop packages are not publisher-signed or notarized.\n"
    "Project licensing is TBD. Third-party components retain their own licenses.\n"
    "https://github.com/kzahel/mclone\n")
notices = destination / "THIRD-PARTY-NOTICES"
shutil.copytree(ROOT / "assets/mclone/audio/licenses", notices / "audio")
# Include the actual license texts distributed with resolved Rust dependencies.
target = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True).split("host: ")[1].splitlines()[0]
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1",
    "--manifest-path", str(ROOT / "native/Cargo.toml"), "--filter-platform", target], cwd=ROOT))
dependencies = []
for package in metadata["packages"]:
    if package["id"] in metadata["workspace_members"]:
        continue
    identifier = package["name"] + "-" + package["version"]
    dependencies.append({"name": identifier, "license": package["license"], "repository": package["repository"]})
    source = Path(package["manifest_path"]).parent
    for entry in source.iterdir():
        if entry.is_file() and entry.name.lower().startswith(("license", "licence", "copying", "notice")):
            output = notices / "rust" / identifier / entry.name
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(entry, output)
(notices / "rust-dependencies.json").write_text(json.dumps(dependencies, indent=2) + "\n")
archive = args.output / (name + ".zip")
with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, strict_timestamps=False) as bundle:
    for path in sorted(destination.rglob("*")):
        if path.is_file():
            bundle.write(path, path.relative_to(args.output))
(archive.with_suffix(".zip.sha256")).write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + "  " + archive.name + "\n")
print(archive)
