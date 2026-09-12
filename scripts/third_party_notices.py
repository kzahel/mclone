"""Retain dependency-supplied notices without assigning a project license."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def write_notices(destination, target, include_node=False):
    destination.mkdir(parents=True, exist_ok=True)
    metadata = json.loads(subprocess.check_output([
        'cargo', 'metadata', '--locked', '--format-version', '1',
        '--manifest-path', str(ROOT / 'native/Cargo.toml'), '--filter-platform', target,
    ], cwd=ROOT))
    dependencies = []
    sections = ['Third-party dependency notices. Mclone project licensing remains TBD.\n']
    seen = set()

    def add(name, source, license_id=None, repository=None):
        if name in seen:
            return
        seen.add(name)
        dependencies.append({'name': name, 'license': license_id, 'repository': repository})
        sections.append(f'\n===== {name} =====\nDeclared license: {license_id or "see dependency source"}\n')
        for entry in sorted(source.iterdir()):
            if entry.is_file() and entry.name.lower().startswith(('license', 'licence', 'copying', 'notice')):
                sections.append(f'\n--- {entry.name} ---\n' + entry.read_text(errors='replace') + '\n')
    for package in metadata['packages']:
        if package['id'] not in metadata['workspace_members']:
            add(package['name'] + '@' + package['version'], Path(package['manifest_path']).parent,
                package['license'], package['repository'])
    if include_node:
        for project in [ROOT, *sorted((ROOT / 'tools').iterdir())]:
            store = project / 'node_modules/.pnpm'
            if not store.is_dir():
                continue
            for manifest in [*store.glob('*/node_modules/*/package.json'),
                             *store.glob('*/node_modules/@*/*/package.json')]:
                package = json.loads(manifest.read_text())
                add(package['name'] + '@' + package['version'], manifest.parent,
                    package.get('license'), package.get('repository'))
    for path in sorted((ROOT / 'assets/mclone/audio/licenses').rglob('*')):
        if path.is_file():
            sections.append('\n===== Audio: ' + path.name + ' =====\n' + path.read_text() + '\n')
    (destination / 'THIRD-PARTY-NOTICES.txt').write_text(''.join(sections))
    (destination / 'third-party-dependencies.json').write_text(json.dumps(dependencies, indent=2) + '\n')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--target', required=True)
    parser.add_argument('--node', action='store_true')
    args = parser.parse_args()
    write_notices(args.destination, args.target, args.node)
