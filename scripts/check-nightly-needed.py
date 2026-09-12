#!/usr/bin/env python3
"""Skip scheduled work when this revision already has a published nightly."""
import json
import os
from pathlib import Path
import re
import subprocess


def main():
    should_run = True
    if os.environ['GITHUB_EVENT_NAME'] == 'schedule':
        repository = os.environ['GITHUB_REPOSITORY']
        revision = os.environ['GITHUB_SHA']
        pages = json.loads(subprocess.check_output([
            'gh', 'api', '--paginate', f'repos/{repository}/releases', '--slurp',
        ]))
        should_run = not any(
            release['prerelease'] and not release['draft']
            and re.fullmatch(r'nightly-\d{8}-\d+', release['tag_name'])
            and release['target_commitish'] == revision
            for page in pages for release in page
        )
    decision = f'should_run={str(should_run).lower()}\n'
    with Path(os.environ['GITHUB_OUTPUT']).open('a') as output:
        output.write(decision)
    print(decision, end='')
    if not should_run:
        print('This revision has a published nightly; skipping checks, assets, and packages.')


if __name__ == '__main__':
    main()
