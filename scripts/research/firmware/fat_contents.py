from pathlib import Path
import struct,json,collections,re,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import CONTENTS_OUT,EVO_IMAGE,need,out_dir
b=need(EVO_IMAGE).read_bytes();allrows=[]
for vol,base in [('A',0xb2b20),('B',0x48b2b20)]:
 sec=struct.unpack_from('<H',b,base+11)[0];spc=b[base+13];reserved=struct.unpack_from('<H',b,base+14)[0];nf=b[base+16];nr=struct.unpack_from('<H',b,base+17)[0];fs=struct.unpack_from('<H',b,base+22)[0];fat=base+reserved*sec;root=fat+nf*fs*sec;data=root+((nr*32+sec-1)//sec)*sec;cs=sec*spc
 def chain(cluster):
  out=bytearray();seen=set()
  while 2<=cluster<0xfff8 and cluster not in seen:
   seen.add(cluster);out.extend(b[data+(cluster-2)*cs:data+(cluster-1)*cs]);cluster=struct.unpack_from('<H',b,fat+2*cluster)[0]
  return bytes(out)
 def walk(buf,path,depth=0):
  if depth>16:return
  lfn=[]
  for off in range(0,len(buf),32):
   e=buf[off:off+32]
   if len(e)<32 or e[0]==0:break
   if e[0]==0xe5:lfn=[];continue
   if e[11]==15:
    lfn.append((e[0]&31,e[1:11]+e[14:26]+e[28:32]));continue
   name=e[:8].decode('ascii',errors='replace').rstrip();ext=e[8:11].decode('ascii',errors='replace').rstrip();name+=('.'+ext)if ext else ''
   if lfn:name=b''.join(x[1]for x in sorted(lfn)).decode('utf-16le',errors='replace').split('\0')[0].rstrip('\uffff')
   lfn=[]
   if name in ('.','..')or e[11]&8:continue
   cluster=struct.unpack_from('<H',e,26)[0];size=struct.unpack_from('<I',e,28)[0];full=path+'/'+name
   if e[11]&16:walk(chain(cluster),full,depth+1)
   else:
    payload=chain(cluster)[:size];strings=[s.decode(errors='replace')for s in re.findall(rb'[ -~]{12,}',payload)if re.search(rb'ThreadX|FreeRTOS|iCatch|SPCA|GCC|MIPS|Linux|eCos|Nucleus|Sunplus',s,re.I)][:12]
    allrows.append({'volume':vol,'path':full,'bytes':size,'head':payload[:32].hex(),'strings':strings})
 walk(b[root:root+nr*32],vol)
 print(vol,'files',sum(x['volume']==vol for x in allrows))
(out_dir(CONTENTS_OUT)/'mini-evo-fat-files.json').write_text(json.dumps(allrows,indent=2))
print('extensions',collections.Counter(Path(x['path']).suffix.upper()for x in allrows));print('largest',sorted(allrows,key=lambda x:x['bytes'],reverse=True)[:15]);print('samplepaths',[r['path']for r in allrows[:70]])
