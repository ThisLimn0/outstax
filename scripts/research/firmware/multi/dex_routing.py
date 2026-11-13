#!/usr/bin/env python3
"""Bounded DEX039 method/reference evidence for printer firmware selection.

Only explicitly identified method bodies are decoded; this is not a Java
decompiler. Code offsets are checked against encoded class method definitions.
"""
from pathlib import Path
import hashlib,json,struct,zlib,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi, multi_out
p=multi('research/multi/inputs/mini-app/classes.dex');b=p.read_bytes()
if hashlib.sha256(b).hexdigest()!='58a4a55cbe9feb7165e6c90dc35006abfaefd8838cb9d2080e4f47d8e69c8937':raise ValueError('unexpected DEX')
assert hashlib.sha1(b[32:]).digest()==b[12:32] and zlib.adler32(b[12:])&0xffffffff==struct.unpack_from('<I',b,8)[0]
def u32(a):return struct.unpack_from('<I',b,a)[0]
def uleb(a):
 v=0;s=0
 while True:
  x=b[a];a+=1;v|=(x&127)<<s
  if x<128:return v,a
  s+=7
strings=[]
for i in range(u32(56)):
 a=u32(u32(60)+4*i);_,a=uleb(a);strings.append(b[a:b.index(0,a)].decode('utf8','replace'))
types=[strings[u32(u32(68)+4*i)]for i in range(u32(64))]
fields=[]
for i in range(u32(80)):
 c,t,n=struct.unpack_from('<HHI',b,u32(84)+8*i);fields.append(types[c]+'.'+strings[n])
methods=[]
for i in range(u32(88)):
 c,proto,n=struct.unpack_from('<HHI',b,u32(92)+8*i);methods.append(types[c]+'.'+strings[n])
definitions={}
for i in range(u32(96)):
 a=u32(u32(100)+32*i+24)
 if not a:continue
 ns,a=uleb(a);ni,a=uleb(a);nd,a=uleb(a);nv,a=uleb(a)
 for _ in range(ns+ni):_,a=uleb(a);_,a=uleb(a)
 for count in [nd,nv]:
  idx=0
  for _ in range(count):
   diff,a=uleb(a);idx+=diff;access,a=uleb(a);code,a=uleb(a)
   if code:definitions[code]={'method':methods[idx],'method_id':idx,'access':access}
width=[1]*256
for op in [2,5,8,0x13,0x15,0x16,0x19,0x1a,0x1c,0x1f,0x20,0x22,0x23,0x29,*range(0x2d,0x3e),*range(0x44,0x6e),*range(0x90,0xb0),*range(0xd0,0xe3),0xfe,0xff]:width[op]=2
for op in [3,6,9,0x14,0x17,0x1b,0x24,0x25,0x26,0x2a,0x2b,0x2c,*range(0x6e,0x73),*range(0x74,0x79),0xfc,0xfd]:width[op]=3
width[0x18]=5;width[0xfa]=width[0xfb]=4
names={0x1a:'const-string',0x1b:'const-string/jumbo',0x22:'new-instance',0x2b:'packed-switch',0x2c:'sparse-switch',0x32:'if-eq',0x33:'if-ne',0x38:'if-eqz',0x39:'if-nez',0x62:'sget-object',0x69:'sput-object',0x71:'invoke-static',0x70:'invoke-direct',0x6e:'invoke-virtual'}
offsets=[0x2e8b80,0x3c64f0,0x3c8bbc,0x3c7af8,0x3c7d0c,0x1e7ff0,0x3c7a50]
out=[]
for code in offsets:
 if code not in definitions:raise ValueError(f'code offset{code:x} not an encoded method')
 count=u32(code+12);units=list(struct.unpack_from('<'+'H'*count,b,code+16));pc=0;ins=[]
 while pc<count:
  word=units[pc];op=word&255;n=width[op]
  item={'pc_code_units':pc,'file_offset':hex(code+16+2*pc),'opcode':hex(op),'name':names.get(op,'op_'+hex(op))}
  if op==0 and word>>8:
   kind=word>>8;size=units[pc+1]
   if kind==1:
    n=4+2*size;first=struct.unpack('<i',struct.pack('<2H',*units[pc+2:pc+4]))[0]
    item['packed_switch_payload']={'first_key':first,'targets_relative_to_switch':[struct.unpack('<i',struct.pack('<2H',*units[pc+4+2*i:pc+6+2*i]))[0]for i in range(size)]}
   elif kind==2:n=2+4*size
   elif kind==3:n=4+(units[pc+1]*(units[pc+2]|units[pc+3]<<16)+1)//2
   else:raise ValueError('unknown payload')
  elif op==0x1a:item['string']=strings[units[pc+1]]
  elif op==0x1b:item['string']=strings[units[pc+1]|units[pc+2]<<16]
  elif 0x52<=op<=0x6d:item['field']=fields[units[pc+1]]
  elif op==0x22:item['type']=types[units[pc+1]]
  elif 0x6e<=op<=0x72 or 0x74<=op<=0x78:item['method_reference']=methods[units[pc+1]]
  if 0x32<=op<=0x3d:
   delta=struct.unpack('<h',struct.pack('<H',units[pc+1]))[0];item['branch_target_pc']=pc+delta
  if op in [0x2b,0x2c]:item['switch_payload_pc']=pc+struct.unpack('<i',struct.pack('<2H',*units[pc+1:pc+3]))[0]
  item['raw_code_units']=[f'{x:04x}'for x in units[pc:pc+n]];ins.append(item);pc+=n
 if pc!=count:raise ValueError('instruction width mismatch')
 out.append({'code_offset':hex(code),**definitions[code],'instructions':ins})
result={'input_sha256':hashlib.sha256(b).hexdigest(),'method':'Explicit method DEX instruction/reference decoding, validated method definitions and checksums; no app execution.','bodies':out,
 'routing_conclusion':'Model enum Link2 maps through ui/t factory to callback t$h, selecting BO23; Link3 maps to t$i selecting LINK3. CPU enum RENESAS versus ST_MICRO selects RenesasRX versus STMicro directory.',
 'limitation':'This establishes the bundled host routing; it does not prove exact physical MCU part or Bluetooth behavior.'}
multi_out('report/multi/dex-routing.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified DEX checksums and decoded',len(out),'referenced method bodies')
