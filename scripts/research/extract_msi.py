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
import json,hashlib,gzip,lzma,bz2
import libarchive,olefile
out=Path(str(RESEARCH_ROOT)+'/drivers');out.mkdir(exist_ok=True)
entries=[]
def unpack(data,dest,depth=0):
 if depth>3:return
 try:
  with libarchive.memory_reader(data) as archive:
   for ent in archive:
    if not ent.isfile:continue
    name=ent.pathname.replace('\\','/').lstrip('./')
    if '..' in Path(name).parts:continue
    d=b''.join(ent.get_blocks());p=dest/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(d)
    entries.append({'file':str(p.relative_to(out)),'bytes':len(d),'sha256':hashlib.sha256(d).hexdigest()})
    if name.endswith('Payload') or d.startswith((b'\x1f\x8b',b'070701',b'070702',b'070707',b'\xfd7zXZ',b'MSCF')):
     unpack(d,p.with_name(p.name+'-files'),depth+1)
 except Exception as e:print('unpack',dest,str(e)[:180])
for p in Path(str(RESEARCH_ROOT)+'/acquisition').iterdir():
 if p.suffix=='.msi':
  with olefile.OleFileIO(p) as ole:
   for stream in ole.listdir():
    data=ole.openstream(stream).read()
    if data.startswith(b'MSCF'):unpack(data,out/p.stem)
Path(str(PROJECT_ROOT)+'/data/drivers/windows-cab-files.json').write_text(json.dumps(entries,indent=2)+'\n')
print(len(entries),'files')
