"""Assemble public release attachments from a verified local package.

Only explicit release files and notices are eligible; books and portable data
are never collected. This script performs no network or GitHub operations.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import zipfile

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--name', default='SaeReader-0.3.0-preview.1')
args = parser.parse_args()
if not re.fullmatch(r'[A-Za-z0-9._-]+', args.name):
    raise ValueError('Invalid package name')
dist = root / 'dist'
runtime = dist / args.name
package = json.loads((dist / f'{args.name}-manifest.json').read_text(encoding='utf-8'))

def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda: file.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()

for item in package['files']:
    path = runtime / item['file'] if item['file'].endswith('.exe') else dist / item['file']
    if path.stat().st_size != item['bytes'] or digest(path) != item['sha256']:
        raise RuntimeError(f'Package checksum mismatch: {item["file"]}')

output = dist / f'{args.name}-release'
if output.exists():
    raise RuntimeError('Release attachment folder exists; choose a new package name')
output.mkdir()
for suffix in ('windows-x64.zip', 'source.zip'):
    shutil.copy2(dist / f'{args.name}-{suffix}', output / f'{args.name}-{suffix}')
for source, name in (
    (runtime / 'SaeReader.exe', 'SaeReader.exe'),
    (runtime / 'LICENSE', 'LICENSE.txt'),
    (runtime / 'CHANGELOG.md', 'CHANGELOG.md'),
    (runtime / '使用说明.txt', 'README.txt'),
    (root / 'assets/icon-original.png', 'SaeReader-icon.png'),
    (root / 'assets/app-icon.ico', 'SaeReader.ico'),
):
    shutil.copy2(source, output / name)

license_zip = output / f'{args.name}-licenses.zip'
with zipfile.ZipFile(license_zip, 'w', compression=zipfile.ZIP_DEFLATED,
                     compresslevel=6, strict_timestamps=False) as archive:
    for name in ('LICENSE', 'THIRD-PARTY-NOTICES.txt', 'dependency-inventory.json'):
        archive.write(runtime / name, f'{args.name}-licenses/{name}')
    archive.write(root / 'assets/README.md', f'{args.name}-licenses/ICON-SOURCE.md')
    for file in sorted((runtime / 'licenses').rglob('*')):
        if file.is_file():
            archive.write(file, f'{args.name}-licenses/{file.relative_to(runtime).as_posix()}')
with zipfile.ZipFile(license_zip) as archive:
    if archive.testzip() is not None:
        raise RuntimeError('License ZIP CRC failure')

files = [{'file': p.name, 'bytes': p.stat().st_size, 'sha256': digest(p)}
         for p in sorted(output.iterdir()) if p.is_file()]
manifest = {
    'version': args.name.removeprefix('SaeReader-'),
    'tag': 'v' + args.name.removeprefix('SaeReader-'),
    'repository': 'https://github.com/PoesisQ/Sae-CN',
    'source_commit': package.get('source_commit'),
    'source_tree_clean': package.get('source_tree_clean'),
    'application_license': 'AGPL-3.0-or-later',
    'corresponding_source': f'{args.name}-source.zip',
    'locked_registry_packages': package['locked_registry_packages'],
    'files': files,
}
(output / 'release-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
checksums = files + [{'file': 'release-manifest.json', 'sha256': digest(output / 'release-manifest.json')}]
(output / 'SHA256SUMS.txt').write_text(''.join(f"{f['sha256']}  {f['file']}\n" for f in checksums), encoding='ascii')
print(json.dumps({'attachments': str(output), 'count': len(list(output.iterdir())),
                  'source_commit': manifest['source_commit'],
                  'binary_sha256': next(f['sha256'] for f in files if f['file'] == 'SaeReader.exe')}, indent=2))
