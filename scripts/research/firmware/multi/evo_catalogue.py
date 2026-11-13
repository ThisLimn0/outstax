#!/usr/bin/env python3
"""Catalogue extracted firmware without printing PEM contents."""
import os,stat,json,hashlib,collections,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_EXTERNAL, MULTI_MATERIAL, multi, multi_out, need
from elftools.elf.elffile import ELFFile
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa
R='research/multi/evo-derived/';roots={13:multi(R+'rootfs-13/854005228/rootfs'),14:multi(R+'rootfs-14/1720163920/rootfs'),18:multi(R+'rootfs-18/1366069669/rootfs')}
# The Windows copy holds the recovered symlinks as regular files containing their target.
LINKS=json.loads(need(MULTI_EXTERNAL/'SYMLINKS.json').read_text())
def sha(x):return hashlib.sha256(x).hexdigest()
def main():
 allrows=[];sums=[];elfs=[];pems=[]
 for pid,root in roots.items():
  counts=collections.Counter();counts['directory']=1
  for base,dirs,files in os.walk(root,followlinks=False):
   for name in dirs+files:
    p=Path(base)/name;k=p.relative_to(MULTI_MATERIAL).as_posix();s=p.lstat();r=dict(partition=pid,path=p.relative_to(root).as_posix(),size=s.st_size,mode=oct(s.st_mode&0o7777))
    if stat.S_ISLNK(s.st_mode):r.update(type='symlink',target=os.readlink(p))
    elif k in LINKS:r.update(type='symlink',target=LINKS[k],mode='0o777')
    elif stat.S_ISDIR(s.st_mode):r['type']='directory'
    elif stat.S_ISREG(s.st_mode):
     b=p.read_bytes();r.update(type='file',sha256=sha(b))
     if b[:4]==b'\x7fELF':
      with p.open('rb') as f:
       e=ELFFile(f);dy=e.get_section_by_name('.dynsym');sy=e.get_section_by_name('.symtab');rec=dict(partition=pid,path=r['path'],size=len(b),machine=e.header.e_machine,type=e.header.e_type,entry=e.header.e_entry,dynamic_symbols=dy.num_symbols() if dy else 0,static_symbols=sy.num_symbols() if sy else 0)
       if r['path']=='lib/libhost.so':
        symbols=[dict(name=x.name,value=x.entry.st_value,size=x.entry.st_size,info=str(x.entry.st_info)) for x in dy.iter_symbols() if x.name];multi_out(R+'libhost-symbols.json').write_text(json.dumps(symbols,indent=2)+'\n')
       elfs.append(rec)
     if b.startswith(b'-----BEGIN '):
      private=b'PRIVATE KEY' in b[:80]
      key=serialization.load_pem_private_key(b,password=None) if private else serialization.load_pem_public_key(b)
      pub=key.public_key() if private else key;der=pub.public_bytes(serialization.Encoding.DER,serialization.PublicFormat.SubjectPublicKeyInfo)
      pems.append(dict(partition=pid,path=r['path'],private=private,type=type(key).__name__,bits=getattr(key,'key_size',None),size=len(b),file_sha256=sha(b),public_spki_sha256=sha(der)))
    else:r['type']='special'
    counts[r['type']]+=1;allrows.append(r)
  sums.append(dict(partition=pid,**counts))
 j=dict(partitions=sums,elf_count=len(elfs),elf_arm_count=sum(e['machine']=='EM_ARM' for e in elfs),kernel_modules=sum(e['path'].endswith('.ko') for e in elfs),elfs=elfs,pem_metadata=pems,resource_jpeg_count=sum(r['partition']==18 and r['path'].lower().endswith('.jpg') for r in allrows),film_tables=[r for r in allrows if 'Film_' in r['path']],models=[r for r in allrows if 'model' in r['path'] and r['path'].endswith('.bin')]);multi_out(R+'files.json').write_text(json.dumps(allrows,indent=2)+'\n');multi_out(R+'catalogue.json').write_text(json.dumps(j,indent=2)+'\n');print(json.dumps({k:v for k,v in j.items() if k!='elfs'},indent=2))
if __name__=='__main__':main()
