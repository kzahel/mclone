#!/usr/bin/env python3
"""Accept only a completed main development run with every build gate green."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

run_id = sys.argv[1]
if not re.fullmatch(r'[0-9]+', run_id):
    raise SystemExit('Expected a numeric Actions run ID')
repository = os.environ['GITHUB_REPOSITORY']


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path]))


run = api(f'repos/{repository}/actions/runs/{run_id}')
if (run['status'] != 'completed' or run['head_branch'] != 'main'
        or run['head_repository']['full_name'] != repository
        or run['path'] != '.github/workflows/development.yml'
        or run['event'] not in ('push', 'schedule', 'workflow_dispatch')
        or not re.fullmatch(r'[0-9a-f]{40}', run['head_sha'])):
    raise SystemExit('Only a completed trusted main development run can be promoted')
pages = json.loads(subprocess.check_output(['gh', 'api', '--paginate',
    f'repos/{repository}/actions/runs/{run_id}/jobs', '--slurp']))
jobs = {job['name']: job['conclusion'] for page in pages for job in page['jobs']}
required = ['checks-and-assets', 'packages / web', 'packages / android']
required += [f'packages / desktop ({runner}, {platform})' for runner, platform in [
    ('ubuntu-24.04', 'linux-x64'), ('ubuntu-24.04-arm', 'linux-arm64'),
    ('windows-2025', 'windows-x64'), ('macos-15', 'macos-arm64')]]
for name in required:
    if jobs.get(name) != 'success':
        raise SystemExit('Required source build gate did not pass: ' + name)
facts = f"revision={run['head_sha']}\nrun_number={run['run_number']}\n"
if 'GITHUB_OUTPUT' in os.environ:
    with Path(os.environ['GITHUB_OUTPUT']).open('a') as output:
        output.write(facts)
print(facts, end='')
