import io,zipfile,urllib.request,struct,hashlib,json,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_MATERIAL, write_input
class Remote(io.RawIOBase):
 def __init__(self,url):
  self.pos=0
  with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'Mozilla/5.0','Range':'bytes=0-0'}),timeout=60)as r:
   self.url=r.url;self.size=int(r.headers['Content-Range'].split('/')[-1]);r.read()
 def readable(self):return True
 def seekable(self):return True
 def tell(self):return self.pos
 def seek(self,n,w=0):self.pos=n if w==0 else self.pos+n if w==1 else self.size+n;return self.pos
 def read(self,n=-1):
  n=self.size-self.pos if n<0 else min(n,self.size-self.pos)
  if n<=0:return b''
  start=self.pos;end=start+n-1
  with urllib.request.urlopen(urllib.request.Request(self.url,headers={'User-Agent':'Mozilla/5.0','Range':f'bytes={start}-{end}'}),timeout=90)as r:
   data=r.read();assert r.status==206 and len(data)==n and r.headers['Content-Range'].startswith(f'bytes {start}-{end}/')
  self.pos+=len(data);return data
class Slice(io.RawIOBase):
 def __init__(self,parent,base,size):self.parent=parent;self.base=base;self.size=size;self.pos=0
 def readable(self):return True
 def seekable(self):return True
 def tell(self):return self.pos
 def seek(self,n,w=0):self.pos=n if w==0 else self.pos+n if w==1 else self.size+n;return self.pos
 def read(self,n=-1):
  n=self.size-self.pos if n<0 else min(n,self.size-self.pos)
  if n<=0:return b''
  self.parent.seek(self.base+self.pos);b=self.parent.read(n);self.pos+=len(b);return b

ROOT=MULTI_MATERIAL/'research/multi/inputs'
TARGETS={
 'mini':('com.fujifilm.instaxminiLink','7.0.6','336',[
 ('mini2','assets/fw/BO23/RenesasRX/0100.hex','12a678b575f1f6356a594bc7a2a07faa3aeec269e685ce4c8b95cbfa2018f6ed'),
 ('mini3','assets/fw/LINK3/RenesasRX/0102.hex','81de808fe0a0fa58f6b3426365848fda68de8a5e6792da4ec758fca36ad7e724')]),
 'wide':('com.fujifilm.instaxLinkWide','2.0.0','204',[
 ('wide','assets/Firmware/BO49/F100.DAT','16187d3f6434e3157b7ca42e919ddb505dc4bdba3527d21ad408f43291519bb4')]),
 'square':('com.fujifilm.instaxSquareLink','1.6.2','60',[
 ('square','assets/Firmware/FW_0102.DAT','9f6e1315e7d822bb8810e34ae691ffa93138043d8d6240ce378bd20d43ddac94')])}
def acquire(key):
 package,version,code,assets=TARGETS[key]
 url=f'https://d.apkpure.com/b/XAPK/{package}?version=latest'
 r=Remote(url);outer=zipfile.ZipFile(r);manifest=json.loads(outer.read('manifest.json'))
 print(key,manifest.get('version_name'),manifest.get('version_code'),flush=True)
 assert str(manifest.get('version_name'))==version and str(manifest.get('version_code'))==code
 entry=outer.getinfo('SharedAssets.apk')
 if entry.compress_type==0:
  r.seek(entry.header_offset);h=r.read(30);fn,extra=struct.unpack_from('<HH',h,26);inner=zipfile.ZipFile(Slice(r,entry.header_offset+30+fn+extra,entry.file_size))
 else:inner=zipfile.ZipFile(io.BytesIO(outer.read(entry)))
 results=[]
 for name,asset,expected in assets:
  data=inner.read(asset);sha=hashlib.sha256(data).hexdigest();assert sha==expected
  target=ROOT/name;path=write_input(target/Path(asset).name,data)
  meta={'package':package,'app_version':version,'app_code':code,'source':url,'asset':asset,'bytes':len(data),'sha256':sha,'path':path.relative_to(MULTI_MATERIAL).as_posix()}
  if asset.endswith('.hex'):
   mem={};upper=0;types={}
   for line in data.decode().splitlines():
    rec=bytes.fromhex(line[1:]);assert sum(rec)&255==0
    n=rec[0];addr=int.from_bytes(rec[1:3],'big');typ=rec[3];body=rec[4:4+n];assert len(rec)==n+5;types[typ]=types.get(typ,0)+1
    if typ==4:upper=int.from_bytes(body,'big')<<16
    elif typ==0:
     for i,b in enumerate(body):
      a=upper+addr+i;assert a not in mem or mem[a]==b;mem[a]=b
   base=min(mem);end=max(mem)+1;assert len(mem)==end-base
   binary=bytes(mem[a] for a in range(base,end));bp=write_input(target/(name+'.bin'),binary)
   meta.update(binary_path=bp.relative_to(MULTI_MATERIAL).as_posix(),binary_bytes=len(binary),binary_sha256=hashlib.sha256(binary).hexdigest(),base=hex(base),end_exclusive=hex(end),record_types=types)
  write_input(target/'input.json',(json.dumps(meta,indent=2)+'\n').encode());results.append(meta);print(json.dumps(meta),flush=True)
 return results
if __name__=='__main__':
 import sys
 results=[]
 for key in sys.argv[1:] or TARGETS:results.extend(acquire(key))
 write_input(ROOT/('manifest-'+('-'.join(sys.argv[1:]) or 'all')+'.json'),(json.dumps(results,indent=2)+'\n').encode())
