#!/usr/bin/env python3
"""Independently validate the complete Intel HEX and reconstruct its flat image."""
from pathlib import Path
import hashlib
import json

import sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1, need
path = need(MINI1 / '0104.hex')
expected_hex = 'f3d3e5d2b981555c6aa6b059504bc1416d17303dede8e07c2fec057fce91b0f7'
expected_bin = 'b04e3490f57f93f562edc868dc34cc06ce670dd41476bf91782e9519aefcda4c'
raw = path.read_bytes()
if hashlib.sha256(raw).hexdigest() != expected_hex:
    raise ValueError('HEX SHA-256 does not match the analyzed snapshot')
mem = {}
upper = 0
entry = None
eof = False
counts = {}
for number, line in enumerate(raw.decode('ascii').splitlines(), 1):
    if eof:
        raise ValueError(f'record after EOF at line {number}')
    if not line.startswith(':'):
        raise ValueError(f'not an Intel HEX record at line {number}')
    record = bytes.fromhex(line[1:])
    if len(record) < 5 or len(record) != record[0] + 5 or sum(record) & 255:
        raise ValueError(f'invalid record length/checksum at line {number}')
    count, kind = record[0], record[3]
    address = int.from_bytes(record[1:3], 'big')
    data = record[4:4+count]
    counts[kind] = counts.get(kind, 0) + 1
    if kind == 0:
        for offset, value in enumerate(data):
            absolute = upper + address + offset
            if absolute in mem:
                raise ValueError(f'overlap at {absolute:#x}')
            mem[absolute] = value
    elif kind == 1:
        if count != 0 or address != 0:
            raise ValueError('malformed EOF')
        eof = True
    elif kind == 4:
        if count != 2 or address != 0:
            raise ValueError('malformed extended linear address')
        upper = int.from_bytes(data, 'big') << 16
    elif kind == 5:
        if count != 4 or address != 0 or entry is not None:
            raise ValueError('malformed/repeated start linear address')
        entry = int.from_bytes(data, 'big')
    else:
        raise ValueError(f'unsupported record type {kind:#x}')
if not eof or not mem:
    raise ValueError('missing EOF or empty image')
base, end = min(mem), max(mem)+1
if len(mem) != end-base:
    raise ValueError('image has gaps')
binary = bytes(mem[a] for a in range(base, end))
if hashlib.sha256(binary).hexdigest() != expected_bin:
    raise ValueError('reconstructed binary SHA-256 mismatch')
saved = MINI1 / 'mini1-0104.bin'
if saved.exists() and saved.read_bytes() != binary:
    raise ValueError('existing BIN differs from reconstructed image')
if not saved.exists():
    saved.write_bytes(binary)
result = {'validated_records': counts, 'base': hex(base), 'end_exclusive': hex(end),
          'bytes': len(binary), 'hex_entrypoint': hex(entry),
          'hex_sha256': expected_hex, 'binary_sha256': expected_bin}
print(json.dumps(result, indent=2))
