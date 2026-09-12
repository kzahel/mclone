#!/usr/bin/env python3
"""Publish only a complete exact-revision build; prune old nightlies afterward."""
from datetime import datetime, timezone, timedelta
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
required = ['mclone-' + platform + suffix for platform, suffix in [
    ('linux-x64', '.zip'), ('linux-arm64', '.zip'), ('windows-x64', '.zip'),
    ('macos-arm64', '.zip'), ('web', '.zip'), ('android-arm64', '.apk'), ('quest-arm64', '.apk')]]
for name in required:
    if not (root / name).is_file() or not (root / (name + '.sha256')).is_file():
        raise SystemExit('Refusing incomplete nightly: missing ' + name)
revision = os.environ['GITHUB_SHA']
now = datetime.now(timezone.utc)
tag = f'nightly-{now:%Y%m%d}-{os.environ["GITHUB_RUN_NUMBER"]}'
with tempfile.NamedTemporaryFile(mode='w', suffix='.md') as notes:
    notes.write(f'''Experimental Mclone build from `{revision}`.

WIP voxel game with limited gameplay. Worlds and file formats may change.
Try the browser game at https://mclone.kzahel.com/play/ or download a package.
Desktop bundles include OpenXR support and are not publisher-signed/notarized.
Android and Quest APKs share a stable nightly signer; install with `adb install -r`.
These are CI-built artifacts; device validation is recorded separately in the
public development guide. Project licensing remains TBD.

Checksums are supplied beside each download. Keep complete desktop folders
intact. Extract the web ZIP and serve it with cross-origin isolation headers.
''')
    notes.flush()
    subprocess.run(['gh', 'release', 'create', tag, '--target', revision, '--prerelease',
                    '--title', f'Nightly {now:%Y-%m-%d} ({revision[:8]})', '--notes-file', notes.name,
                    *[str(p) for p in sorted(root.iterdir()) if p.is_file()]], check=True)
# Retention only touches our dated prereleases after a replacement succeeded.
releases = json.loads(subprocess.check_output(['gh', 'api', '--paginate',
    f'repos/{os.environ["GITHUB_REPOSITORY"]}/releases', '--slurp']))
for release in [r for page in releases for r in page]:
    old_tag = release['tag_name']
    published = datetime.fromisoformat(release['published_at'].replace('Z', '+00:00')) if release['published_at'] else now
    if release['prerelease'] and old_tag.startswith('nightly-') and old_tag != tag and published < now - timedelta(days=14):
        subprocess.run(['gh', 'release', 'delete', old_tag, '--yes', '--cleanup-tag'], check=True)
