#!/usr/bin/env python3
"""Package the complete analysis tree without following recovered symlinks."""
from pathlib import Path
import hashlib
import json
import os
import stat
import sys
import zipfile

sys.dont_write_bytecode = True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MATERIAL, MULTI_EXTERNAL, MULTI_MATERIAL, MULTI_OUT, need, out_dir
ROOT = MULTI_EXTERNAL
os.chdir(ROOT)
PREFIX = 'instax-multi-firmware-analysis/'
OUT = out_dir(MULTI_OUT) / 'instax-multi-firmware-project.zip'
META = out_dir(MULTI_OUT / 'output/package')
# The Windows copy of research/ holds the recovered symlinks as regular files containing their target.
LINKS = json.loads(need(ROOT / 'SYMLINKS.json').read_text())
EXCLUDED_DIRS = {'__pycache__', 'python', 'target', '.toolchain', '.build-tmp', '.git'}
EXCLUDED_FILES = {'RECOVERY.md', 'renesas-checkpoint.md', 'pip.log'}
selected = {}
directories = set()
symlinks = {}

def collect(base, root=ROOT):
    for current, dirs, files in os.walk(root / base, followlinks=False):
        rel = Path(current).relative_to(root)
        dirs[:] = sorted(d for d in dirs if d not in EXCLUDED_DIRS)
        directories.add(rel.as_posix())
        for name in dirs[:]:
            path = Path(current) / name
            if path.is_symlink():
                symlinks[(rel / name).as_posix()] = os.readlink(path)
                dirs.remove(name)
        for name in sorted(files):
            if name in EXCLUDED_FILES or name.endswith(('.pyc', '.zip')):
                continue
            path = Path(current) / name
            key = (rel / name).as_posix()
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                symlinks[key] = os.readlink(path)
            elif key in LINKS:
                symlinks[key] = LINKS[key]
            elif stat.S_ISREG(mode):
                selected[key] = path
            else:
                raise ValueError(f'Unexpected non-regular entry: {path}')

for base in ['research', 'report', 'tools/multi', 'tools/mini1', 'rust',
             'examples', 'third_party', 'output/pdf']:
    collect(base, MULTI_MATERIAL if base == 'research' else ROOT)
# binutils source moved to material/references; keep it beside the GPL rx-elf-objdump.
selected['third_party/binutils-2.44.tar.xz'] = need(MATERIAL / 'references/binutils-2.44/binutils-2.44.tar.xz')
directories.add('third_party')
for name in ['tools/NTKFWinfo.py', 'tools/fat_contents.py']:
    selected[name] = Path(name)
selected['README.md'] = Path('report/multi/PROJECT-README.md')
selected['requirements.txt'] = Path('tools/multi/requirements.txt')
(META / 'SYMLINKS.json').write_text(json.dumps(symlinks, indent=2, sort_keys=True) + '\n')
selected['SYMLINKS.json'] = META / 'SYMLINKS.json'
metadata = {'root_folder': PREFIX.rstrip('/'), 'symlinks_followed': False,
    'regular_files_before_manifests': len(selected), 'symlink_entries': len(symlinks),
    'excluded': ['installed dependencies', 'build caches and Rust target directories',
                 'temporary PDF page renders', 'superseded checkpoint notes',
                 'duplicate prior staging folders and ZIPs', 'upload infrastructure'],
    'scope': 'Complete retained analysis inputs, outputs, reports, tools and prior mini1 Rust source.',
    'hardware_validation': False}
(META / 'PACKAGING.json').write_text(json.dumps(metadata, indent=2) + '\n')
selected['PACKAGING.json'] = META / 'PACKAGING.json'
lines = []
for name, path in sorted(selected.items()):
    assert stat.S_ISREG(path.lstat().st_mode) and not path.is_symlink()
    h = hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()
    lines.append(h + '  ' + name)
(META / 'SHA256SUMS').write_text('\n'.join(lines) + '\n')
selected['SHA256SUMS'] = META / 'SHA256SUMS'
# Opening the existing path preserves its retained Library identity/version xattrs.
with zipfile.ZipFile(OUT, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6,
                     allowZip64=True) as archive:
    archive.writestr(PREFIX, b'')
    for directory in sorted(directories):
        archive.writestr(PREFIX + directory + '/', b'')
    for name, path in sorted(selected.items()):
        archive.write(path, PREFIX + name)
    for name, target in sorted(symlinks.items()):
        info = zipfile.ZipInfo(PREFIX + name)
        info.create_system = 3
        info.external_attr = (stat.S_IFLNK | 0o777) << 16
        archive.writestr(info, target.encode('utf-8', 'surrogateescape'))
with zipfile.ZipFile(OUT) as archive:
    assert archive.testzip() is None
    assert len(archive.namelist()) == len(set(archive.namelist()))
    for name, target in symlinks.items():
        info = archive.getinfo(PREFIX + name)
        assert stat.S_ISLNK(info.external_attr >> 16)
        assert archive.read(info) == target.encode('utf-8', 'surrogateescape')
result = {'zip': str(OUT), 'bytes': OUT.stat().st_size,
    'sha256': hashlib.file_digest(OUT.open('rb'), 'sha256').hexdigest(),
    'regular_files': len(selected), 'symlinks': len(symlinks), 'crc_check': 'passed'}
(META / 'archive-verification.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
