#!/usr/bin/env python3
"""Check that published instruction bytes and descriptors match the input image."""
from pathlib import Path
import json
import struct
import sys

sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1, need, report as report_path
BASE = 0x08020000
b = need(MINI1/'mini1-0104.bin').read_bytes()
checked = 0
literal_count = 0
for name in ['protocol','status','update','settings','image']:
    report = json.loads(report_path(f'{name}.json').read_text())
    for section in report['evidence'].values():
        instructions = section if isinstance(section,list) else section['instructions']
        for item in instructions:
            address = int(item.get('va',item.get('address')),0)
            off = int(item.get('file_offset',hex(address-BASE)),0)
            raw = bytes.fromhex(item['bytes'])
            if address != BASE+off or b[off:off+len(raw)] != raw:
                raise ValueError(f'instruction evidence differs in {name} at {address:#x}')
            if 'literal' in item:
                lit = item['literal']; litoff = int(lit.get('va',lit.get('address')),0)-BASE
                if struct.unpack_from('<I',b,litoff)[0] != int(lit['value'],0):
                    raise ValueError('literal value mismatch')
                literal_count += 1
            checked += 1
j = json.loads(report_path('protocol.json').read_text())
for item in j['descriptors'] + [j['sentinel']]:
    off = int(item['file_offset'], 0)
    raw = bytes.fromhex(item['raw_hex'])
    if len(raw) != 16 or b[off:off+16] != raw:
        raise ValueError(f'descriptor evidence differs at {item["va"]}')
    if struct.unpack_from('<H', raw)[0] != int(item['opcode'], 0):
        raise ValueError('opcode encoding mismatch')
print(f'{checked} instruction records, {literal_count} literals and {len(j["descriptors"])+1} descriptor records match the image.')
