import io,zipfile,urllib.request,struct,hashlib,json,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1
def save(name,data):
 p=MINI1/name
 if p.exists() and hashlib.sha256(p.read_bytes()).hexdigest()!=hashlib.sha256(data).hexdigest():
  sys.exit(f'refusing to overwrite {p}: existing file has a different SHA-256 (set OUTSTAX_MATERIAL to use another folder)')
 p.write_bytes(data)
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
MINI1.mkdir(parents=True,exist_ok=True)
url='https://d.apkpure.com/b/XAPK/com.fujifilm.instaxminiLink?version=latest'
r=Remote(url);outer=zipfile.ZipFile(r);manifest=json.loads(outer.read('manifest.json'));entry=outer.getinfo('SharedAssets.apk');print('version',manifest.get('version_name'),'splitcompression',entry.compress_type,flush=True)
if entry.compress_type==0:
 r.seek(entry.header_offset);h=r.read(30);fn,extra=struct.unpack_from('<HH',h,26);inner=zipfile.ZipFile(Slice(r,entry.header_offset+30+fn+extra,entry.file_size))
else:inner=zipfile.ZipFile(io.BytesIO(outer.read(entry)))
asset='assets/fw/BOD/0104.hex';data=inner.read(asset);sha=hashlib.sha256(data).hexdigest();assert sha=='f3d3e5d2b981555c6aa6b059504bc1416d17303dede8e07c2fec057fce91b0f7';save('0104.hex',data)
mem={};upper=0;entrypoint=None
for line in data.decode().splitlines():
 rec=bytes.fromhex(line[1:]);assert sum(rec)&255==0;n=rec[0];addr=int.from_bytes(rec[1:3],'big');typ=rec[3];body=rec[4:4+n]
 if typ==4:upper=int.from_bytes(body,'big')<<16
 elif typ==0:
  for i,b in enumerate(body):mem[upper+addr+i]=b
 elif typ==5:entrypoint=int.from_bytes(body,'big')
base=min(mem);end=max(mem)+1;assert len(mem)==end-base;binary=bytes(mem[a]for a in range(base,end));save('mini1-0104.bin',binary)
meta={'package':'com.fujifilm.instaxminiLink','app_version':manifest.get('version_name'),'app_code':manifest.get('version_code'),'source':url,'asset':asset,'hex_bytes':len(data),'hex_sha256':sha,'binary_bytes':len(binary),'binary_sha256':hashlib.sha256(binary).hexdigest(),'base':hex(base),'end_exclusive':hex(end),'entrypoint':hex(entrypoint)if entrypoint else None}
save('input.json',json.dumps(meta,indent=2).encode());print(json.dumps(meta,indent=2));print(binary[:256].hex())
