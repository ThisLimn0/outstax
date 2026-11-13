#!/usr/bin/env python3
"""Reproduce RX image integrity, descriptor registries and string evidence (no execution)."""
from pathlib import Path
import hashlib,json,struct,re,argparse,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_OUT, multi, multi_out, out_dir
PROFILES={'mini2':{'label':'BO23 / mini Link 2','hex':'0100.hex','table':0xfff1086c,'source_sha256':'12a678b575f1f6356a594bc7a2a07faa3aeec269e685ce4c8b95cbfa2018f6ed'},'mini3':{'label':'LINK3 / mini Link 3','hex':'0102.hex','table':0xfff1d3ec,'source_sha256':'81de808fe0a0fa58f6b3426365848fda68de8a5e6792da4ec758fca36ad7e724'}}
def ihex(raw):
 mem={}; upper=0;types={};overlaps=0
 for ln,line in enumerate(raw.decode('ascii').splitlines(),1):
  assert line.startswith(':'),(ln,'not Intel HEX')
  r=bytes.fromhex(line[1:]);assert len(r)>=5 and len(r)==r[0]+5,(ln,'length');assert sum(r)%256==0,(ln,'checksum')
  n,addr,typ=r[0],int.from_bytes(r[1:3],'big'),r[3];data=r[4:-1];types[typ]=types.get(typ,0)+1
  if typ==4:assert n==2;upper=int.from_bytes(data,'big')<<16
  elif typ==0:
   for i,b in enumerate(data):
    a=upper+addr+i
    if a in mem:assert mem[a]==b;overlaps+=1
    mem[a]=b
  elif typ==1:assert n==0
  else:assert typ in {2,3,5},(ln,typ)
 base=min(mem);end=max(mem)+1;assert end-base==len(mem),'noncontiguous image'
 return base,bytes(mem[a] for a in range(base,end)),types,overlaps
LABELS={0:'handshake',1:'identity selectors',2:'capability/status selectors',0x1000:'image setup',0x1001:'image block',0x1002:'image finalize',0x1003:'image cancel',0x1080:'print operation',0x2000:'firmware transfer setup',0x2001:'firmware transfer block',0x2002:'firmware transfer finalize',0x2010:'update metadata query',0x2080:'update operation',0x2081:'update operation'}
def extract(name,p):
 source=f'research/multi/inputs/{name}/'+p['hex'];raw=multi(source).read_bytes();assert hashlib.sha256(raw).hexdigest()==p['source_sha256']
 base,b,types,overlaps=ihex(raw);assert base==0xfff00000 and len(b)==917504
 dest=out_dir(MULTI_OUT/'research/multi/renesas-derived'/name);(dest/(name+'.bin')).write_bytes(b)
 off=p['table']-base;records=[]
 for i in range(512):
  chunk=b[off+16*i:off+16*(i+1)];op,length=struct.unpack_from('<HH',chunk);handler,continuation=struct.unpack_from('<II',chunk,8)
  r={'record_va':f'0x{base+off+16*i:08x}','opcode':f'0x{op:04x}','request_frame_bytes':length,'flags':list(chunk[4:8]),'handler':f'0x{handler:08x}','continuation':f'0x{continuation:08x}','raw_hex':chunk.hex(),'interpretation':LABELS.get(op,'unassigned')}
  if op==0xffff:sentinel=r;break
  assert base<=handler<base+len(b),(name,r)
  assert continuation==0 or base<=continuation<base+len(b),(name,r)
  records.append(r)
 else:raise AssertionError('missing table sentinel')
 strings=[]
 for m in re.finditer(rb'[ -~]{4,}',b):
  s=m.group().decode('ascii')
  if any(x in s.upper() for x in ['FUJI','INSTAX','SP-','PRINT','TASK','USBD','I2C','SPI','RENESAS','LINK','B-O']):strings.append({'address':f'0x{base+m.start():08x}','value':s})
 obj={'profile':name,'label':p['label'],'source':source,'source_sha256':hashlib.sha256(raw).hexdigest(),'binary_sha256':hashlib.sha256(b).hexdigest(),'base':hex(base),'end_exclusive':hex(base+len(b)),'binary_bytes':len(b),'record_types':types,'identical_overlaps':overlaps,'first_64_bytes':b[:64].hex(),'last_128_bytes':b[-128:].hex(),'table_va':hex(p['table']),'record_count':len(records),'unique_opcode_count':len({r['opcode'] for r in records}),'descriptors':records,'sentinel':sentinel,'selected_strings':strings}
 multi_out('report/multi/renesas-'+name+'.json').write_text(json.dumps(obj,indent=2)+'\n');(dest/'selected-strings.json').write_text(json.dumps(strings,indent=2)+'\n')
 lines=[f'# {p["label"]} reproducible registry','',f'Binary SHA-256: `{obj["binary_sha256"]}`. {len(records)} records / {obj["unique_opcode_count"]} unique opcodes.','', '| Opcode | Record VA | Frame length | Flags (raw) | Handler | Continuation |','|---|---|---:|---|---|---|']
 for r in records:lines.append('| '+ ' | '.join([r['opcode'],r['record_va'],str(r['request_frame_bytes']),str(r['flags']),r['handler'],r['continuation']])+' |')
 lines+=['',f'Sentinel at {sentinel["record_va"]}: `{sentinel["raw_hex"]}`.','', 'Selected strings:','']
 lines.extend(f'- `{s["address"]}`: `{s["value"]}`' for s in strings)
 multi_out('report/multi/renesas-'+name+'-registry.md').write_text('\n'.join(lines)+'\n')
 print(name,len(b),hashlib.sha256(b).hexdigest(),len(records),obj['unique_opcode_count'])
 return obj
if __name__=='__main__':
 for n,p in PROFILES.items():extract(n,p)
