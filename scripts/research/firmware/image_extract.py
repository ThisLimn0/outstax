#!/usr/bin/env python3
"""Reproduce mini Link 0104 JPEG wrapper evidence, without running firmware."""
from pathlib import Path
import hashlib
import json
import struct
import sys

sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import IJG, MINI1, need, out_dir
from capstone import Cs, CS_ARCH_ARM, CS_MODE_THUMB, CS_MODE_LITTLE_ENDIAN
from capstone.arm import ARM_OP_MEM, ARM_REG_PC

BASE = 0x08020000
b = need(MINI1/'mini1-0104.bin').read_bytes()
expected = 'b04e3490f57f93f562edc868dc34cc06ce670dd41476bf91782e9519aefcda4c'
if hashlib.sha256(b).hexdigest() != expected:
    raise ValueError('wrong firmware image')
md = Cs(CS_ARCH_ARM, CS_MODE_THUMB | CS_MODE_LITTLE_ENDIAN)
md.detail = True

def location(a):
    return {'va': f'0x{a:08x}', 'file_offset': hex(a-BASE)}

modes = []
for i in range(3):
    a = 0x080203AC + 20*i
    raw = b[a-BASE:a-BASE+20]
    mode, width, height, outw, outh, reserved, num, den = struct.unpack('<6H2I', raw)
    modes.append({**location(a), 'raw_hex': raw.hex(), 'internal_mode': mode,
                  'input_width': width, 'input_height': height,
                  'decoder_output_width': outw, 'decoder_output_height': outh,
                  'scale_numerator': num, 'scale_denominator': den,
                  'crop_left': max(0, (outw-600)//2),
                  'crop_top': max(0, (outh-800)//2),
                  'pad_left': max(0, (600-outw)//2),
                  'pad_top': max(0, (800-outh)//2)})

ranges = {
    'jpeg_header_constraints_and_mode_selection': (0x0802D7AC, 0xD8),
    'vertical_crop_and_decoder_start': (0x0802D8B0, 0x7E),
    'scanline_copy_and_vertical_reversal': (0x0802D99C, 0xA0),
    'actual_decode_wrapper': (0x0802DA62, 0xAA),
    'print_state_registers_background_decode': (0x08068156, 0x1A),
    'background_task_invokes_function': (0x08057D7C, 0x44),
    'background_work_registration': (0x08057DC2, 0x78),
}
evidence = {}
for label, (start, size) in ranges.items():
    instructions = []
    for ins in md.disasm(b[start-BASE:start-BASE+size], start):
        item = {**location(ins.address), 'bytes': ins.bytes.hex(),
                'instruction': ins.mnemonic+' '+ins.op_str}
        for operand in ins.operands:
            if operand.type == ARM_OP_MEM and operand.mem.base == ARM_REG_PC:
                a = ((ins.address+4)&~3)+operand.mem.disp
                if BASE <= a <= BASE+len(b)-4:
                    item['literal'] = {**location(a), 'value': hex(struct.unpack_from('<I', b, a-BASE)[0])}
        instructions.append(item)
    evidence[label] = {'start': location(start), 'byte_length': size,
                       'instructions': instructions}

obj = {
    'input_sha256': expected,
    'method': 'Static Thumb disassembly from manually identified functions; no emulation or device action.',
    'decode_wrapper_entry': '0x0802da62',
    'entry_correction': '0x0802da56 is the return instruction of the scanline wrapper, not the image decoder entry.',
    'jpeg_constraints': {
        'start_bytes': 'ffd8', 'end_bytes_at_declared_length_minus_2': 'ffd9',
        'components': 3,
        'jpeg_color_space': {'struct_offset': '0x28', 'required_value': 3, 'ijg9a_meaning': 'JCS_YCbCr'},
        'output_color_space': {'struct_offset': '0x29', 'value': 2, 'ijg9a_meaning': 'JCS_RGB'},
        'progressive_policy': 'Not resolved. Do not equate a conservative client baseline-only policy with a proven firmware rejection.',
    },
    'dimension_modes': modes,
    'selection': 'width and height determine the table record; its mode is internal, not an extra caller-selected upload field.',
    'output': {
        'base': '0xc06a0004', 'bytes': 600*800*3,
        'row_stride': 1800, 'channel_plane_offsets_within_row': [0, 600, 1200],
        'prefill_byte': 255,
        'destination_row_formula': '799 - source_row + crop_top - pad_top',
        'destination_column_formula': 'pad_left + i; source column = crop_left + i',
        'channel_order': 'RGB software samples after IJG JCS_RGB conversion; physical printhead/dye mapping not inferred.',
        'postdecode_saved_format': '0x10',
    },
    'call_chain': [
        {'site': '0x08068166', 'behavior': 'load Thumb callback0x0802da63 from0x08068894'},
        {'site': '0x0806816c', 'behavior': 'register callback using0x08057dc2 with work priority3'},
        {'site': '0x08057e20', 'behavior': 'save supplied function in work record0x20000234+4'},
        {'site': '0x08057d8c', 'behavior': 'BG task loads the saved function and executes BLX'},
        {'site': '0x0802daca', 'behavior': 'JPEG wrapper calls decoder initialization0x0802d762'},
        {'site': '0x0802dad6', 'behavior': 'decode in batches of100 scanlines through0x0802d948'},
    ],
    'primary_reference': json.loads(need(IJG/'ijg-source.json').read_text()),
    'evidence': evidence,
}
out = out_dir()/'image.json'
out.write_text(json.dumps(obj, indent=2)+'\n')
print(f'Wrote {out.name}: {len(modes)} dimension records; '
      f'{sum(len(s["instructions"]) for s in evidence.values())} instruction records.')
