#!/usr/bin/env python3
"""Inspect each complete APK, then publish it with checksums and build facts."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

root = Path(__file__).resolve().parents[1]
output = root / 'dist-release'
output.mkdir(exist_ok=True)
for project, name in [('android', 'mclone-android-arm64'), ('android-xr', 'mclone-quest-arm64')]:
    source = root / project / 'app/build/outputs/apk/release/app-release.apk'
    with tempfile.TemporaryDirectory() as temporary:
        with zipfile.ZipFile(source) as apk:
            apk.extractall(temporary)
        subprocess.run([sys.executable, str(root / 'scripts/check-public-assets.py'), temporary,
                        '--report', str(output / (name + '-files.json'))], check=True)
    destination = output / (name + '.apk')
    shutil.copy2(source, destination)
    (output / (name + '.apk.sha256')).write_text(hashlib.sha256(destination.read_bytes()).hexdigest() + '  ' + destination.name + '\n')
(output / 'android-build.json').write_text(json.dumps({
    'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    'experimental': True, 'project_license': 'TBD',
    'device_tested': False, 'install': 'adb install -r <apk>',
}, indent=2) + '\n')
