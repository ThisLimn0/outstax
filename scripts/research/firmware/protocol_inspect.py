#!/usr/bin/env python3
"""Read-only Thumb inspection helper; all addresses are mapped at 0x08020000."""
import argparse, json, pathlib, struct, sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(pathlib.Path(__file__).parent))
from _paths import MINI1,need
from capstone import *
from capstone.arm import *
BASE=0x08020000
p=argparse.ArgumentParser(); p.add_argument('mode',choices=['dis','strings','refs','imm','table']);p.add_argument('values',nargs='*');a=p.parse_args()
b=need(MINI1/'mini1-0104.bin').read_bytes()
md=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);md.detail=True
u32=lambda off:struct.unpack_from('<I',b,off)[0]
num=lambda x:int(x,0)
if a.mode=='dis':
 start=num(a.values[0])&~1;size=num(a.values[1]) if len(a.values)>1 else 0x100
 for i in md.disasm(b[start-BASE:start-BASE+size],start):
  extra=''
  for op in i.operands:
   if op.type==ARM_OP_MEM and op.mem.base==ARM_REG_PC:
    addr=((i.address+4)&~3)+op.mem.disp
    if BASE<=addr<=BASE+len(b)-4:extra+=f' ; [{addr:#010x}]={u32(addr-BASE):#010x}'
  print(f'{i.address:08x}: {i.bytes.hex():10s} {i.mnemonic:8s} {i.op_str}{extra}')
elif a.mode=='strings':
 import re
 for m in re.finditer(rb'[ -~]{5,}',b):
  if not a.values or any(v.lower() in m.group().decode().lower() for v in a.values):print(f'{BASE+m.start():08x} {m.group().decode()}')
elif a.mode=='refs':
 vals={num(x)&~1 for x in a.values}
 for off in range(0,len(b)-3,2):
  if u32(off)&~1 in vals: print(f'data {BASE+off:08x} -> {u32(off):08x}')
 for off in range(0,len(b)-4,2):
  i=next(md.disasm(b[off:off+4],BASE+off),None)
  if i and i.mnemonic.startswith(('b','cb')):
   for op in i.operands:
    if op.type==ARM_OP_IMM and op.imm&~1 in vals:print(f'code {i.address:08x} {i.mnemonic} {i.op_str}')
elif a.mode=='imm':
 vals={num(x) for x in a.values}
 # decode each halfword to avoid losing alignment at embedded pools
 for off in range(0,len(b)-4,2):
  ins=next(md.disasm(b[off:off+4],BASE+off),None)
  if ins and any(o.type==ARM_OP_IMM and o.imm in vals for o in ins.operands):print(f'{ins.address:08x} {ins.mnemonic:8s} {ins.op_str}')
elif a.mode=='table':
 start=num(a.values[0]);size=num(a.values[1]) if len(a.values)>1 else 0x80
 for off in range(start-BASE,start-BASE+size,4):print(f'{BASE+off:08x} {u32(off):08x} {b[off:off+4].hex()}')
