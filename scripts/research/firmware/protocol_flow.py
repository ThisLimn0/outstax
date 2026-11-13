#!/usr/bin/env python3
"""Conservative local control-flow walk from explicit Thumb entrypoints."""
import sys,pathlib,json,struct,collections
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(pathlib.Path(__file__).parent))
from _paths import MINI1,need
from capstone import *
from capstone.arm import *
BASE=0x08020000;b=need(MINI1/'mini1-0104.bin').read_bytes();md=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);md.detail=True
seeds=[int(x,0)&~1 for x in sys.argv[1:]]
seen={};q=collections.deque(seeds)
while q and len(seen)<100000:
 addr=q.popleft()
 while BASE<=addr<BASE+len(b) and addr not in seen:
  ins=next(md.disasm(b[addr-BASE:addr-BASE+4],addr),None)
  if ins is None:break
  seen[addr]=ins
  mnemonic=ins.mnemonic.split('.')[0]
  target=[o.imm&0xffffffff for o in ins.operands if o.type==ARM_OP_IMM]
  if mnemonic in ('bl','blx'):
   pass # do not descend call targets
  elif mnemonic=='b':
   if target:q.append(target[-1]&~1)
   break
  elif mnemonic.startswith('b') and mnemonic not in ('bic','bics','bfi','bfc'):
   if target:q.append(target[-1]&~1)
   if mnemonic=='bx':break
  elif mnemonic.startswith('cb'):
   if target:q.append(target[-1]&~1)
  if mnemonic in ('tbb','tbh') or (mnemonic=='pop' and 'pc' in ins.op_str) or (ins.mnemonic.startswith('ldr') and ins.op_str.startswith('pc,')) :break
  addr+=ins.size
for addr,ins in sorted(seen.items()):
 extra=''
 for op in ins.operands:
  if op.type==ARM_OP_MEM and op.mem.base==ARM_REG_PC:
   loc=((addr+4)&~3)+op.mem.disp
   if BASE<=loc<=BASE+len(b)-4:extra+=f' ; [{loc:#010x}]={struct.unpack_from("<I",b,loc-BASE)[0]:#010x}'
 print(f'{addr:08x}: {ins.bytes.hex():10s} {ins.mnemonic:8s} {ins.op_str}{extra}')
