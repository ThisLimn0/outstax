#!/usr/bin/env python3
"""Extract the exact Rich-mode table from a locally acquired INSTAX base APK."""
import argparse
import hashlib
import zipfile
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('apk', type=Path)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
with zipfile.ZipFile(args.apk) as archive:
    data = archive.read('assets/20210701_3DLUT.bin')
if len(data) != 67108864:
    raise SystemExit('Unexpected table size; expected 67108864 bytes')
digest = hashlib.sha256(data).hexdigest()
with args.output.open('xb') as output:
    output.write(data)
print(f'{args.output}: {len(data)} bytes, SHA-256 {digest}')
if digest != '9bc33b49082d9359f677094d8fa10343da54b3c07654c24d88a6f5a039f82092':
    print('This table differs from the three app snapshots analyzed on 2026-10-25.')

