#!/usr/bin/env python3
"""Publish only a complete exact-revision build; prune old nightlies afterward."""
from datetime import datetime, timezone, timedelta
import json
import hashlib
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

root = Path(sys.argv[1])
revision = os.environ['GITHUB_SHA']
repository = os.environ['GITHUB_REPOSITORY']
required = ['mclone-' + platform + suffix for platform, suffix in [
    ('linux-x64', '.zip'), ('linux-arm64', '.zip'), ('windows-x64', '.zip'),
    ('macos-arm64', '.zip'), ('web', '.zip'), ('android-arm64', '.apk'), ('quest-arm64', '.apk')]]
for name in required:
    if not (root / name).is_file() or not (root / (name + '.sha256')).is_file():
        raise SystemExit('Refusing incomplete nightly: missing ' + name)
    expected = (root / (name + '.sha256')).read_text().split()[0]
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != expected:
        raise SystemExit('Refusing nightly with a checksum mismatch: ' + name)
    if name.endswith('.zip'):
        with zipfile.ZipFile(root / name) as archive:
            receipt = 'BUILD.json' if name == 'mclone-web.zip' else name[:-4] + '/BUILD.json'
            if json.loads(archive.read(receipt))['revision'] != revision:
                raise SystemExit('Refusing mixed-revision nightly: ' + name)
if json.loads((root / 'android-build.json').read_text())['revision'] != revision:
    raise SystemExit('Refusing mixed-revision Android nightly')
if not (root / 'first-party-provenance.json').is_file():
    raise SystemExit('Refusing nightly without the strict asset provenance receipt')
releases = json.loads(subprocess.check_output(['gh', 'api', '--paginate',
    f'repos/{repository}/releases', '--slurp']))
releases = [r for page in releases for r in page]
if os.environ.get('GITHUB_EVENT_NAME') == 'schedule' and any(
    r['prerelease'] and not r['draft'] and re.fullmatch(r'nightly-\d{8}-\d+', r['tag_name'])
    and r['target_commitish'] == revision for r in releases
):
    print('This revision already has a published nightly; keeping the existing downloads.')
    raise SystemExit(0)
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
    subprocess.run(['gh', 'release', 'create', tag, '--target', revision, '--draft', '--prerelease',
                    '--title', f'Nightly {now:%Y-%m-%d} ({revision[:8]})', '--notes-file', notes.name,
                    *[str(p) for p in sorted(root.iterdir()) if p.is_file()]], check=True)
# Keep partial uploads hidden. Verify GitHub's uploaded asset inventory before
# exposing this release; the previous complete nightly stays available on error.
uploaded = json.loads(subprocess.check_output(['gh', 'api', f'repos/{repository}/releases/tags/{tag}']))
expected_assets = {p.name: p for p in root.iterdir() if p.is_file()}
if {asset['name'] for asset in uploaded['assets']} != set(expected_assets):
    raise SystemExit('Nightly draft retained: uploaded asset inventory differs')
for asset in uploaded['assets']:
    source = expected_assets[asset['name']]
    expected_digest = 'sha256:' + hashlib.sha256(source.read_bytes()).hexdigest()
    if asset['size'] != source.stat().st_size or asset.get('digest') != expected_digest:
        raise SystemExit('Nightly draft retained: uploaded bytes differ for ' + asset['name'])
subprocess.run(['gh', 'release', 'edit', tag, '--draft=false'], check=True)
# Retention only touches our dated prereleases after a replacement succeeded.
for release in releases:
    old_tag = release['tag_name']
    published = datetime.fromisoformat(release['published_at'].replace('Z', '+00:00')) if release['published_at'] else now
    if (release['prerelease'] and not release['draft']
            and re.fullmatch(r'nightly-\d{8}-\d+', old_tag) and old_tag != tag
            and '<!-- keep-nightly -->' not in (release.get('body') or '')
            and published < now - timedelta(days=14)):
        subprocess.run(['gh', 'release', 'delete', old_tag, '--yes', '--cleanup-tag'], check=True)
