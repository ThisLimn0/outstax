#!/usr/bin/env python3
"""Recover the mini Link 0104 header, vectors and runtime initialization table.

These are file-backed observations, not a firmware emulator.
Run from the project: python scripts/research/firmware/startup.py
"""
from pathlib import Path
import json
import struct
import subprocess
import sys

sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1, need, out_dir
BASE = 0x08020000
IMAGE = need(MINI1 / 'mini1-0104.bin')
OUT = out_dir()
b = IMAGE.read_bytes()

def u32(address):
    return struct.unpack_from('<I', b, address - BASE)[0]

def hx(x):
    return f'0x{x:08x}'

# First initialization routine clears (length,destination) pairs until length=0.
clears = []
cursor = 0x080718CC
while u32(cursor):
    size, destination = u32(cursor), u32(cursor + 4)
    clears.append({'destination': hx(destination), 'bytes': size,
                   'end_exclusive': hx(destination + size)})
    cursor += 8
assert cursor == 0x080718F4

# Second initialization routine copies words from a relative source pointer.
size = u32(0x080718FC)
relative = struct.unpack_from('<i', b, 0x08071900 - BASE)[0]
source = 0x08071900 + relative
destination = u32(0x08071904)
assert source == 0x08070000 and size == 0x160

vectors = [{'index': i, 'entry_address': hx(0x08020200 + 4*i),
            'value': hx(u32(0x08020200 + 4*i))} for i in range(107)]
result = {
    'basis': 'mini1-0104.bin; addresses are virtual flash addresses',
    'header': {'magic_hex': b[:4].hex(), 'magic_ascii': b[:4].decode('ascii'),
               'version_u16_le': hex(struct.unpack_from('<H', b, 4)[0]),
               'entry_thumb': hx(u32(BASE + 8))},
    'boot_stub': {'entry_even': '0x08020010', 'vector_table': '0x08020200',
                  'psp': '0x1000ffc0', 'msp': '0x1000bfc0',
                  'runtime_entry_thumb': hx(u32(0x08020048)),
                  'vtor_mmio': hx(u32(0x0802003C))},
    'vector_reset_word': hx(u32(0x08020204)),
    'vector_reset_note': '0x08000000 is outside this update payload; not treated as a valid standalone Cortex-M boot image',
    'c_runtime': {'entry_even': '0x080718bc', 'fpu_enable': '0x0806ff8c',
                  'initialization_walker': '0x0806ffc8', 'zero_routine': '0x0802e33e',
                  'copy_routine': '0x0802e3d6', 'application_main': '0x08052098',
                  'rtos_initialization_call': '0x0805218e'},
    'zero_regions': clears,
    'copy_regions': [{'source': hx(source), 'destination': hx(destination), 'bytes': size}],
    'vectors': vectors,
}
OUT.mkdir(parents=True, exist_ok=True)
(OUT / 'startup.json').write_text(json.dumps(result, indent=2) + '\n')
(OUT / 'startup-data.bin').write_bytes(b[source-BASE:source-BASE+size])
entries = ['0x08020010','0x080718bc','0x0806ff8c','0x0806ffa8',
           '0x0806ffc8','0x0802e33e','0x0802e3d6','0x08052098']
with (OUT / 'startup-disassembly.txt').open('w') as out:
    subprocess.run([sys.executable, str(Path(__file__).parent/'protocol_flow.py'), *entries],
                   stdout=out, check=True)
print(json.dumps({k:v for k,v in result.items() if k != 'vectors'}, indent=2))
