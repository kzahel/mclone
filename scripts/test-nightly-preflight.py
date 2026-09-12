#!/usr/bin/env python3
"""Exercise the early nightly gate without launching builds or publishing."""
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).with_name('check-nightly-needed.py')
REVISION = 'a' * 40
PUBLISHED = dict(prerelease=True, draft=False,
                 tag_name='nightly-20260912-1', target_commitish=REVISION)


class NightlyPreflightTests(unittest.TestCase):
    def run_gate(self, pages, event='schedule', error=None):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'output'
            with patch.dict(os.environ, {
                'GITHUB_EVENT_NAME': event, 'GITHUB_SHA': REVISION,
                'GITHUB_REPOSITORY': 'fixture/repo', 'GITHUB_OUTPUT': str(output),
            }), patch('subprocess.check_output', return_value=json.dumps(pages).encode(),
                      side_effect=error) as api:
                if error:
                    with self.assertRaises(subprocess.CalledProcessError):
                        runpy.run_path(str(SCRIPT), run_name='__main__')
                    self.assertFalse(output.exists())
                    return
                runpy.run_path(str(SCRIPT), run_name='__main__')
                if event != 'schedule':
                    api.assert_not_called()
                return output.read_text()

    def test_published_revision_skips_even_on_later_page(self):
        self.assertEqual(self.run_gate([[], [PUBLISHED]]), 'should_run=false\n')

    def test_new_revision_builds(self):
        self.assertEqual(self.run_gate([[dict(PUBLISHED, target_commitish='b' * 40)]]),
                         'should_run=true\n')

    def test_first_nightly_builds(self):
        self.assertEqual(self.run_gate([[]]), 'should_run=true\n')

    def test_failed_draft_retries(self):
        self.assertEqual(self.run_gate([[dict(PUBLISHED, draft=True)]]), 'should_run=true\n')

    def test_unrelated_releases_do_not_suppress_build(self):
        for change in [dict(tag_name='project-media'), dict(prerelease=False),
                       dict(tag_name='nightly-manual')]:
            with self.subTest(change=change):
                self.assertEqual(self.run_gate([[dict(PUBLISHED, **change)]]),
                                 'should_run=true\n')

    def test_push_pr_and_manual_runs_do_not_query_or_skip(self):
        for event in ['push', 'pull_request', 'workflow_dispatch']:
            with self.subTest(event=event):
                self.assertEqual(self.run_gate([[PUBLISHED]], event), 'should_run=true\n')

    def test_api_failure_stops_before_build_decision(self):
        self.run_gate([], error=subprocess.CalledProcessError(1, ['gh', 'api']))


if __name__ == '__main__':
    unittest.main()
