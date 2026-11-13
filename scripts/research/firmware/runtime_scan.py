#!/usr/bin/env python3
"""Read-only static survey of the mini Link 1 Intel-HEX image."""
import sys,struct,re,json,collections,hashlib
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1,need,out_dir
from capstone import Cs,CS_ARCH_ARM,CS_MODE_THUMB,CS_MODE_LITTLE_ENDIAN
from capstone.arm import ARM_OP_MEM,ARM_REG_PC
P=need(MINI1/'mini1-0104.bin')
b=P.read_bytes();BASE=0x08020000;END=BASE+len(b)
def u32(a):return struct.unpack_from('<I',b,a-BASE)[0]
strings=[]
for m in re.finditer(rb'[\x20-\x7e]{5,}',b):
 strings.append({'addr':hex(BASE+m.start()),'size':len(m[0]),'text':m[0].decode()})
runs=[]
for m in re.finditer(rb'\xff{128,}|\x00{128,}',b):
 runs.append({'start':hex(BASE+m.start()),'end':hex(BASE+m.end()),'size':len(m[0]),'byte':m[0][0]})
md=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);md.detail=True;md.skipdata=True
lits=[];calls=[];prologues=[]
for ins in md.disasm(b,BASE):
 if ins.id==0:continue
 if ins.mnemonic.startswith('push') and 'lr' in ins.op_str:prologues.append(ins.address)
 if ins.mnemonic in ('bl','blx') and ins.op_str.startswith('#'):
  try:calls.append([ins.address,int(ins.op_str[1:],0)])
  except ValueError:pass
 for op in ins.operands:
  if op.type==ARM_OP_MEM and op.mem.base==ARM_REG_PC:
   a=((ins.address+4)&~3)+op.mem.disp
   if BASE<=a<=END-4:lits.append({'insn':hex(ins.address),'asm':ins.mnemonic+' '+ins.op_str,'pool':hex(a),'value':hex(u32(a))})
words=collections.Counter(struct.unpack('<'+'I'*(len(b)//4),b[:len(b)//4*4]))
mmio=[{'value':hex(v),'aligned_words':n}for v,n in words.most_common()if 0x40000000<=v<0x60000000 or 0xe0000000<=v<0xe0100000]
vectors=[hex(u32(BASE+0x200+i*4))for i in range(107)]
out={'base':hex(BASE),'end':hex(END),'size':len(b),'sha256':hashlib.sha256(b).hexdigest(),'strings':strings,'long_constant_runs':runs,'vector_table_address':'0x08020200','vector_words_107':vectors,'survey_caveat':'Linear sweep candidates include data decoded as Thumb; inspect targeted disassembly before using any call, literal or prologue as evidence.','mmio_word_candidates':mmio,'literal_load_candidates':lits,'direct_call_candidates':[[hex(a),hex(t)]for a,t in calls],'prologue_candidates':[hex(x)for x in prologues]}
p=out_dir()/'runtime-survey.json';p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(out,indent=2))
print(json.dumps({'strings':len(strings),'runs':runs,'top_mmio':mmio[:100],'literal_loads':len(lits),'calls':len(calls),'prologues':len(prologues)},indent=2))
print('\nSTRINGS\n'+'\n'.join(f"{s['addr']} {s['text']}"for s in strings if len(s['text'])>7))
