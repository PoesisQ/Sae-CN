"""Validate delivered ZIP CRCs, checksums, source coverage and native icon entries.

Run after package.py. Reads files only; does not extract or execute the archives.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import struct
import zipfile

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--name', default='SaeReader-0.3.0-preview.1')
args = parser.parse_args()
dist = root / 'dist'
manifest = json.loads((dist / (args.name + '-manifest.json')).read_text(encoding='utf-8'))
for item in manifest['files']:
    file = dist / args.name / item['file'] if item['file'].endswith('.exe') else dist / item['file']
    with file.open('rb') as stream:
        digest = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    assert file.stat().st_size == item['bytes'] and digest.hexdigest() == item['sha256'], file

source_prefix = args.name + '-source/'
checked = {}
for kind in ('source', 'windows-x64'):
    with zipfile.ZipFile(dist / f'{args.name}-{kind}.zip') as archive:
        names = archive.namelist()
        assert len(names) == len(set(names)), 'duplicate entries'
        for name in names:
            parts = PurePosixPath(name).parts
            assert '\\' not in name and '..' not in parts and not name.startswith('/'), name
            assert not set(parts) & {'.git', '.tools', '.aws', '__pycache__'}, name
            assert parts[1] not in {'target', 'data', 'artifacts'}, name
            if len(parts)>1 and parts[1] not in {'dependencies', 'vendor'}:
                assert not name.endswith('/state.json'), name
            if name.endswith(('.epub', '.pdf', '.txt', '.md', '.azw3', '.mobi', '.fb2')) and '/tests/fixtures/' in name:
                assert (root / 'tests/fixtures' / PurePosixPath(name).name).is_file(), name
            assert '/vendor/libmobi/tests/' not in name, 'upstream ebook payloads are local opt-in inputs'
        assert archive.testzip() is None, 'CRC failure'
        checked[kind] = {'entries': len(names), 'crc': 'all passed'}
        if kind == 'source':
            required = ['Cargo.toml', 'Cargo.lock', 'build.rs', 'README.md', 'CHANGELOG.md', 'src/main.rs', 'ui/reader.slint',
                        'assets/icon-original.png', 'assets/window-icon.png', 'assets/app-icon.ico',
                        'assets/sae-reader.rc', 'scripts/generate-icon.py', 'scripts/verify-package.py',
                        'src/reading.rs', 'src/rendering.rs', 'src/text_adapter.rs', 'src/epub_compat.rs',
                        'src/kindle_adapter.rs', 'src/fb2_adapter.rs', 'src/zoom.rs',
                        'native/kindle.c', 'native/kindle_alloc.c', 'native/kindle_alloc.h',
                        'vendor/libmobi/src/mobi.h', 'vendor/libmobi/COPYING', 'vendor/libmobi/UPSTREAM.txt',
                        'src/fonts.rs', 'assets/icons/dial.svg', 'assets/icons/fullscreen.svg',
                        'vendor/mupdf-sys-0.8.0/mupdf/source/fitz/context.c',
                        'vendor/mupdf-sys-0.8.0/mupdf/source/html/html-doc.c',
                        'vendor/mupdf-sys-0.8.0/wrapper/document.c',
                        'vendor/mupdf-sys-0.8.0/wrapper/font.c', 'vendor/mupdf-sys-0.8.0/wrapper/page.c',
                        'legal/third-party/sources.json']
            for rel in required:
                assert archive.read(source_prefix + rel) == (root / rel).read_bytes(), rel
            dependency_dirs = {PurePosixPath(n).parts[2] for n in names
                               if n.startswith(source_prefix + 'dependencies/')}
            assert len(dependency_dirs) == manifest['locked_registry_packages'], len(dependency_dirs)
            assert 'directory = "dependencies"' in archive.read(source_prefix + '.cargo/config.toml').decode()
            assert 'vendor/mupdf-sys-0.8.0/mupdf/' in '\n'.join(names)
            checked[kind]['registry_source_directories'] = len(dependency_dirs)
        else:
            assert archive.read(args.name + '/SaeReader.exe') == (dist / args.name / 'SaeReader.exe').read_bytes()
            assert not any('/docs/evidence/' in n for n in names), 'raw evidence belongs in the full source archive'
            assert archive.read(args.name+'/docs/README.txt')
            inventory=json.loads(archive.read(args.name+'/dependency-inventory.json'))
            for component in inventory['native_components']:
                for notice in component['notices']: assert archive.read(args.name+'/'+notice), notice
            assert any(c['name']=='libmobi' for c in inventory['native_components'])
            for p in inventory['packages']:
                if p['default_windows_build']:
                    assert p['notices'] or p['supplemental_notices'], p['name']
            supplements=json.loads(archive.read(args.name+'/licenses/supplemental-sources.json'))
            for record in supplements['records']:
                data=archive.read(args.name+'/licenses/'+record['file'])
                assert hashlib.sha256(data).hexdigest()==record['sha256'], record['file']
            checked[kind]['supplemented_license_packages']=len(supplements['selected_branches'])

if os.name == 'nt':
    from ctypes import wintypes
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
    kernel.LoadLibraryExW.restype = wintypes.HANDLE
    kernel.FindResourceW.argtypes = [wintypes.HANDLE, ctypes.c_void_p, ctypes.c_void_p]
    kernel.FindResourceW.restype = wintypes.HANDLE
    kernel.SizeofResource.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel.LoadResource.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel.LoadResource.restype = wintypes.HANDLE
    kernel.LockResource.argtypes = [wintypes.HANDLE]
    kernel.LockResource.restype = ctypes.c_void_p
    kernel.FreeLibrary.argtypes = [wintypes.HANDLE]
    # Data/image resources only: target executable code is never run.
    module = kernel.LoadLibraryExW(str(dist / args.name / 'SaeReader.exe'), None, 0x22)
    assert module, ctypes.get_last_error()
    try:
        def resource(identifier, kind):
            entry = kernel.FindResourceW(module, identifier, kind)
            assert entry, (identifier, kind, ctypes.get_last_error())
            size = kernel.SizeofResource(module, entry)
            data = kernel.LockResource(kernel.LoadResource(module, entry))
            assert data and size
            return ctypes.string_at(data, size)
        group = resource(1, 14)
        reserved, kind, count = struct.unpack_from('<HHH', group)
        assert (reserved, kind, count) == (0, 1, 8)
        sizes = []
        for offset in range(6, len(group), 14):
            width, height, colors, reserved, planes, bits, length, identifier = struct.unpack_from('<BBBBHHIH', group, offset)
            assert width == height
            sizes.append(width or 256)
            pixels = resource(identifier, 3)
            assert len(pixels) == length and pixels.startswith(b'\x89PNG\r\n\x1a\n')
        assert sorted(sizes) == [16, 20, 24, 32, 48, 64, 128, 256], sizes
        checked['executable_icon_sizes'] = sizes
    finally:
        kernel.FreeLibrary(module)

print(json.dumps(checked, indent=2))
