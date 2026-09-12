#!/usr/bin/env python3
"""Inspect each complete APK, then publish it with checksums and build facts."""
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile
from third_party_notices import write_notices

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

write_notices(output, "aarch64-linux-android")

ndk_version = re.search(r'ndkVersion\s*=\s*"([^"]+)"', (root / 'android/app/build.gradle.kts').read_text()).group(1)
sdk_roots = [Path(value) for value in (os.environ.get('ANDROID_HOME'), os.environ.get('ANDROID_SDK_ROOT')) if value]
sdk_roots += [Path.home() / 'Android/Sdk', Path.home() / 'Library/Android/sdk']
notice = next((sdk / 'ndk' / ndk_version / 'NOTICE' for sdk in sdk_roots if (sdk / 'ndk' / ndk_version / 'NOTICE').is_file()), None)
if notice is None:
    raise SystemExit('Set ANDROID_HOME to the SDK used by the build scripts so the bundled libc++ notice can be copied')
shutil.copy2(notice, output / 'ANDROID-NDK-NOTICE.txt')
