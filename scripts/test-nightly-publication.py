#!/usr/bin/env python3
"""Exercise publication failures without touching GitHub or real releases."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SCRIPT = Path(__file__).with_name('publish-nightly.py')
REVISION = 'a' * 40


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for platform in ['linux-x64', 'linux-arm64', 'windows-x64', 'macos-arm64', 'web']:
            name = 'mclone-' + platform
            with zipfile.ZipFile(self.root / (name + '.zip'), 'w') as bundle:
                receipt = 'BUILD.json' if platform == 'web' else name + '/BUILD.json'
                bundle.writestr(receipt, json.dumps({'revision': REVISION}))
        for platform in ['android', 'quest']:
            (self.root / ('mclone-' + platform + '-arm64.apk')).write_bytes(b'fixture APK')
        for artifact in list(self.root.iterdir()):
            self.checksum(artifact)
        (self.root / 'android-build.json').write_text(json.dumps({'revision': REVISION}))
        (self.root / 'first-party-provenance.json').write_text('{}')

    def checksum(self, artifact):
        artifact.with_name(artifact.name + '.sha256').write_text(
            hashlib.sha256(artifact.read_bytes()).hexdigest() + '  ' + artifact.name + '\n')

    def publish(self, *, bad_upload=False, releases=None):
        assets = [{'name': p.name, 'size': p.stat().st_size,
                   'digest': 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest()}
                  for p in self.root.iterdir()]
        if bad_upload:
            assets[0]['digest'] = 'sha256:' + '0' * 64
        with patch.dict(os.environ, {'GITHUB_SHA': REVISION, 'GITHUB_REPOSITORY': 'fixture/repo',
                                    'GITHUB_RUN_NUMBER': '1', 'GITHUB_EVENT_NAME': 'workflow_dispatch'}), \
             patch('sys.argv', [str(SCRIPT), str(self.root)]), \
             patch('subprocess.check_output', side_effect=[json.dumps([releases or []]).encode(), json.dumps({'assets': assets}).encode()]), \
             patch('subprocess.run') as calls:
            self.calls = calls
            runpy.run_path(str(SCRIPT), run_name='__main__')

    def test_tampered_package_never_reaches_github(self):
        (self.root / 'mclone-quest-arm64.apk').write_bytes(b'tampered')
        with self.assertRaisesRegex(SystemExit, 'checksum mismatch'):
            self.publish()
        self.calls.assert_not_called()

    def test_valid_checksum_cannot_hide_mixed_revision(self):
        artifact = self.root / 'mclone-web.zip'
        with zipfile.ZipFile(artifact, 'w') as bundle:
            bundle.writestr('BUILD.json', json.dumps({'revision': 'b' * 40}))
        self.checksum(artifact)
        with self.assertRaisesRegex(SystemExit, 'mixed-revision'):
            self.publish()
        self.calls.assert_not_called()

    def test_missing_platform_never_reaches_github(self):
        (self.root / 'mclone-windows-x64.zip').unlink()
        with self.assertRaisesRegex(SystemExit, 'incomplete nightly'):
            self.publish()
        self.calls.assert_not_called()

    def test_bad_remote_upload_remains_draft(self):
        with self.assertRaisesRegex(SystemExit, 'uploaded bytes differ'):
            self.publish(bad_upload=True)
        self.assertEqual(len(self.calls.call_args_list), 1)
        self.assertIn('--draft', self.calls.call_args.args[0])

    def test_complete_verified_upload_is_published(self):
        self.publish()
        self.assertIn('--draft', self.calls.call_args_list[0].args[0])
        self.assertEqual(self.calls.call_args_list[1].args[0][-1], '--draft=false')

    def test_retention_preserves_pinned_and_unrelated_releases(self):
        base = {'prerelease': True, 'draft': False, 'published_at': '2020-01-01T00:00:00Z'}
        self.publish(releases=[
            dict(base, tag_name='nightly-20200101-1', body='<!-- keep-nightly -->'),
            dict(base, tag_name='nightly-manually-curated', body=''),
            dict(base, tag_name='project-media', body=''),
        ])
        self.assertEqual(len(self.calls.call_args_list), 2)


if __name__ == '__main__':
    unittest.main()
