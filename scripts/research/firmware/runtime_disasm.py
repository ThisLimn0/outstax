#!/usr/bin/env python3
import sys,struct
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1,need
from capstone import Cs,CS_ARCH_ARM,CS_MODE_THUMB,CS_MODE_LITTLE_ENDIAN
from capstone.arm import ARM_OP_MEM,ARM_REG_PC
BASE=0x08020000;b=need(MINI1/'mini1-0104.bin').read_bytes()
s=int(sys.argv[1],0)&~1;n=int(sys.argv[2],0)if len(sys.argv)>2 else 0x100
m=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);m.detail=True
for i in m.disasm(b[s-BASE:s-BASE+n],s):
 extra=''
 for o in i.operands:
  if o.type==ARM_OP_MEM and o.mem.base==ARM_REG_PC:
   a=((i.address+4)&~3)+o.mem.disp
   if BASE<=a<=BASE+len(b)-4:extra=f' ; [{a:#010x}] = {struct.unpack_from("<I",b,a-BASE)[0]:#010x}'
 print(f'{i.address:08x}: {i.bytes.hex():10} {i.mnemonic:9} {i.op_str}{extra}')
