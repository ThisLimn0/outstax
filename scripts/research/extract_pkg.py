# Run from any directory; vendor inputs belong under --research-root (default: ../research beside the project).
import argparse
from pathlib import Path
_cli=argparse.ArgumentParser()
_cli.add_argument('--project-root',type=Path,default=Path(__file__).resolve().parents[2])
_cli.add_argument('--research-root',type=Path)
_args=_cli.parse_args()
PROJECT_ROOT=_args.project_root.resolve()
RESEARCH_ROOT=(_args.research_root or PROJECT_ROOT.parent/'research').resolve()  # research workspace beside the project
from pathlib import Path
import struct,zlib,xml.etree.ElementTree as ET,json,hashlib,shutil
import libarchive
out=Path(str(RESEARCH_ROOT)+'/drivers-mac');out.mkdir(exist_ok=True)
items=[]
def save(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
 items.append({'file':str(path.relative_to(out)),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
for pkg in Path(str(RESEARCH_ROOT)+'/acquisition').glob('*.pkg'):
 data=pkg.read_bytes();magic,hlen,version,compressed,length,algo=struct.unpack('>4sHHQQI',data[:28]);toc=ET.fromstring(zlib.decompress(data[hlen:hlen+compressed]));heap=hlen+compressed
 def extract(node,root):
  name=node.findtext('name');p=root/name
  if node.findtext('type')=='directory':
   for child in node.findall('file'):extract(child,p)
  elif node.find('data') is not None:
   info=node.find('data');offset=int(info.findtext('offset'));size=int(info.findtext('size'));body=data[heap+offset:heap+offset+size]
   encoding=info.find('encoding').get('style')
   if encoding=='application/x-gzip':body=zlib.decompress(body)
   save(p,body)
   if name=='Payload':
    with libarchive.memory_reader(body) as ar:
     for ent in ar:
      if ent.isfile:
       dest=p.with_name('files')/ent.pathname.lstrip('./');save(dest,b''.join(ent.get_blocks()))
 for node in toc.findall('toc/file'):extract(node,out/pkg.stem)
windows=json.loads((PROJECT_ROOT/'data/drivers/windows-cab-files.json').read_text())
Path(str(PROJECT_ROOT)+'/data/drivers/driver-files.json').write_text(json.dumps(windows+items,indent=2)+'\n')
print(len(items),'Mac files extracted; inventory',len(windows+items))
for p in out.rglob('*'):
 if p.is_file() and ('MacOS' in str(p) or p.suffix=='.bin'):print(str(p),p.stat().st_size)
