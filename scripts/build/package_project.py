#!/usr/bin/env python3
"""Archive the entire clean project directory, including hidden files and empty directories."""
import argparse, hashlib, json, zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--project',type=Path,default=Path(__file__).resolve().parents[2])
p.add_argument('--output',type=Path,required=True)
a=p.parse_args();root=a.project.resolve();output=a.output.resolve()
if output==root or root in output.parents:raise SystemExit('ZIP output must be outside the project folder')
if (root/'target').exists():raise SystemExit('Move the build cache outside the project folder before packaging; no project files will be silently excluded')
def listing():  # project content only, not git metadata
    return sorted(x for x in root.rglob('*') if '.git' not in x.relative_to(root).parts)
paths=listing()
if any(x.is_symlink() for x in paths):raise SystemExit('Resolve project symlinks explicitly before packaging')
manifest=root/'SHA256SUMS.txt'
files=[f for f in paths if f.is_file() and f!=manifest]
manifest.write_text(''.join(hashlib.sha256(f.read_bytes()).hexdigest()+'  '+f.relative_to(root).as_posix()+'\n' for f in files),newline='\n')
paths=listing();output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
    z.write(root,root.name+'/')
    for path in paths:z.write(path,root.name+'/'+path.relative_to(root).as_posix()+('/' if path.is_dir() else ''))
with zipfile.ZipFile(output) as z:
    bad=z.testzip()
    if bad:raise RuntimeError('ZIP CRC failure: '+bad)
    expected={root.name+'/'}|{root.name+'/'+x.relative_to(root).as_posix()+('/' if x.is_dir() else '') for x in paths}
    if set(z.namelist())!=expected:raise RuntimeError('Archive file inventory differs from project folder')
print(json.dumps({'archive':str(output),'project_folder':root.name,'files':sum(x.is_file() for x in paths),'directories':sum(x.is_dir() for x in paths)+1,'bytes':output.stat().st_size,'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'all_folder_entries_included':True,'crc_verified':True},indent=2))
