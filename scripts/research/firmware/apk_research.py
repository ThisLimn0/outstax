import io,zipfile,json,re,hashlib,struct,urllib.request
from pathlib import Path
class Remote(io.RawIOBase):
 def __init__(self,url):
  self.url=url;self.pos=0
  with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'Mozilla/5.0','Range':'bytes=0-0'}),timeout=60) as r:
   self.size=int(r.headers['Content-Range'].split('/')[-1]);self.url=r.url;r.read()
 def readable(self):return True
 def seekable(self):return True
 def tell(self):return self.pos
 def seek(self,n,w=0):self.pos=n if w==0 else self.pos+n if w==1 else self.size+n;return self.pos
 def read(self,n=-1):
  n=self.size-self.pos if n<0 else min(n,self.size-self.pos)
  if n<=0:return b''
  start=self.pos;end=start+n-1
  for attempt in range(3):
   try:
    with urllib.request.urlopen(urllib.request.Request(self.url,headers={'User-Agent':'Mozilla/5.0','Range':f'bytes={start}-{end}'}),timeout=120) as r:
     b=r.read()
     if r.status!=206 or len(b)!=n or not r.headers.get('Content-Range','').startswith(f'bytes {start}-{end}/'):raise ValueError('range mismatch')
     self.pos+=len(b);return b
   except Exception:
    if attempt==2:raise

def dex_strings(data):
 count,offset=struct.unpack_from('<II',data,56)
 for i in range(count):
  p=struct.unpack_from('<I',data,offset+4*i)[0]
  while data[p]&128:p+=1
  p+=1;e=data.find(b'\0',p)
  yield data[p:e].decode('utf-8',errors='replace')
def acquire(package,out):
 out=Path(out);out.mkdir(parents=True,exist_ok=True);url='https://d.apkpure.com/b/XAPK/'+package+'?version=latest';remote=Remote(url);z=zipfile.ZipFile(remote)
 manifest=json.loads(z.read('manifest.json'));(out/'manifest.json').write_text(json.dumps(manifest,indent=2))
 inventory=[];fw=[];strings=[]
 for entry in z.infolist():
  if not entry.filename.endswith('.apk') or not (entry.filename==package+'.apk' or entry.filename=='base.apk' or 'SharedAssets' in entry.filename or 'arm64' in entry.filename):continue
  data=z.read(entry);print(package,entry.filename,len(data),flush=True);(out/entry.filename).write_bytes(data);az=zipfile.ZipFile(io.BytesIO(data))
  inventory.append({'file':entry.filename,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'entries':az.namelist()})
  for item in az.infolist():
   if item.filename.endswith('.dex'):strings.extend(dex_strings(az.read(item)))
   if re.search(r'firmware|(?:^|/)fw(?:/|_)|\.hex$|FW[_0-9]|BO\d.*\.DAT',item.filename,re.I):
    payload=az.read(item);p=out/'firmware'/item.filename;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(payload);fw.append({'apk':entry.filename,'asset':item.filename,'bytes':len(payload),'sha256':hashlib.sha256(payload).hexdigest(),'saved':str(p)})
 (out/'apk-inventory.json').write_text(json.dumps(inventory,indent=2));(out/'firmware-assets.json').write_text(json.dumps(fw,indent=2))
 strings=sorted(set(strings));(out/'dex-strings.txt').write_text('\n'.join(strings));refs=[s for s in strings if re.search(r'firmware|https?://|FW_.*DAT|\.hex\b|firm.*(?:url|download|version)|update.*program|btfw',s,re.I)];(out/'references.txt').write_text('\n'.join(refs))
 return manifest,fw
if __name__=='__main__':
 import sys;acquire(sys.argv[1],sys.argv[2])
