#!/usr/bin/env python3
"""Record concrete instruction/data anchors for the shared printer protocol."""
import json,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi_generated, multi_out
from capstone import Cs,CS_ARCH_MIPS,CS_MODE_MIPS32,CS_MODE_LITTLE_ENDIAN
md=Cs(CS_ARCH_MIPS,CS_MODE_MIPS32|CS_MODE_LITTLE_ENDIAN)
def window(b,off,n):
    va=off+0x80000000-0x3b2b20
    return [dict(file_offset=hex(off+i.address-va),va=hex(i.address),bytes=i.bytes.hex(),mnemonic=i.mnemonic,operands=i.op_str) for i in md.disasm(b[off:off+n],va)]
for model in ['square','wide2']:
    b=multi_generated(f'research/multi/sunp-derived/{model}/decoded.bin').read_bytes()
    wide=model=='wide2';magic=0x74d700 if wide else 0x6b1a10
    assert b[magic:magic+8]==b'Ab\0\0aB\0\0'
    regions={'receive_magic_compare':(0x57d714,0x44) if wide else (0x575508,0x44),'response_header_builder':(0x578828,0x64) if wide else (0x570af8,0x64),'download_start_fields_and_reply':(0x579248,0x18c) if wide else (0x570e34,0x134),'image_block_response':(0x579410,0x210) if wide else (0x570f68,0xa8),'stream_receive_sequence':(0x57d8cc,0x25c) if wide else (0x5756ac,0x250),'processing_geometry':(0x55c098,0x24) if wide else (0x54ef74,0x64),'checksum':(0x578408,0x1c0) if wide else (0x5706d8,0x1c0),'be32_helper':(0x5b25b0,0x34) if wide else (0x5a7fb8,0x34),'dimension_allocator':(0x553f50,0x30) if wide else (0x54f1a4,0x38)}
    r=dict(model=model,code_mapping=dict(file_offset='0x3b2b20',va='0x80000000'),magic=dict(file_offset=hex(magic),request_bytes='4162',response_bytes='6142',confidence='confirmed by receive comparison and response two-byte copy'),frame=dict(header_bytes=6,magic_bytes=2,total_length_offset=2,total_length_type='big-endian uint16',group_offset=4,subtype_offset=5,payload_offset=6,checksum_bytes=1,checksum_rule='byte sum of complete packet modulo 256 equals 255'),download_start=dict(command_hex='1000',request_payload_bytes=8,request_total_bytes=15,fields=[dict(payload_offset=0,bytes=1,name='picture type'),dict(payload_offset=1,bytes=1,name='print option'),dict(payload_offset=2,bytes=1,name='print option 2'),dict(payload_offset=3,bytes=1,name='reserved or unassigned; copied only in WIDE2 path'),dict(payload_offset=4,bytes=4,name='image byte count',encoding='big-endian uint32')],success_response_total_bytes=12,success_response_fields=['one-byte result','big-endian uint32 segment size']),image_block=dict(command_hex='1001',sequence_bytes=4,sequence_encoding='big-endian uint32',image_data_packet_offset=10,total_packet_bytes_formula='image chunk bytes + 11',success_response_total_bytes=12,success_response_fields=['one-byte result','big-endian uint32 received sequence'],confidence='direct parser/endian-helper and response code in both; not all sequence-error paths traced'),geometry=dict(width=1260 if wide else 800,height=840 if wide else 800,scope='processing/default target, not universal wire-input validation',confidence='immediate constants in executable instructions'),instruction_regions={name:window(b,*region) for name,region in regions.items()})
    multi_out('report/multi/'+model+'-protocol-evidence.json').write_text(json.dumps(r,indent=2)+'\n')
    print(model,len(r['instruction_regions']),'instruction regions')
