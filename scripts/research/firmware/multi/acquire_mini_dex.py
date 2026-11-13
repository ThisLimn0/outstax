#!/usr/bin/env python3
"""Acquire only classes.dex from the mini Link APK bundle for model routing proof."""
import sys,zipfile,json,struct,io,hashlib,zlib
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
sys.path.insert(0,str(Path(__file__).resolve().parent))
from _paths import MULTI_MATERIAL, write_input
from acquire_restore import Remote,Slice
source='https://d.apkpure.com/b/XAPK/com.fujifilm.instaxminiLink?version=latest'
r=Remote(source);outer=zipfile.ZipFile(r);manifest=json.loads(outer.read('manifest.json'))
assert str(manifest.get('version_name'))=='7.0.6' and str(manifest.get('version_code'))=='336'
print('entries',outer.namelist(),flush=True)
name='com.fujifilm.instaxminiLink.apk'
entry=outer.getinfo(name)
if entry.compress_type==0:
 r.seek(entry.header_offset);h=r.read(30);fn,extra=struct.unpack_from('<HH',h,26);inner=zipfile.ZipFile(Slice(r,entry.header_offset+30+fn+extra,entry.file_size))
else:inner=zipfile.ZipFile(io.BytesIO(outer.read(entry)))
print('base entries ready',flush=True)
data=inner.read('classes.dex');assert len(data)==6888320
assert data[:4]==b'dex\n' and data[7]==0
assert hashlib.sha1(data[32:]).digest()==data[12:32]
assert zlib.adler32(data[12:])&0xffffffff==struct.unpack_from('<I',data,8)[0]
p=write_input(MULTI_MATERIAL/'research/multi/inputs/mini-app/classes.dex',data)
o={'source':source,'package':'com.fujifilm.instaxminiLink','app_version':'7.0.6','app_code':'336','asset':name+'/classes.dex','path':p.relative_to(MULTI_MATERIAL).as_posix(),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'role':'Application DEX evidence for model-specific firmware routing'}
write_input(p.parent/'input.json',(json.dumps(o,indent=2)+'\n').encode());print(json.dumps(o),flush=True)
