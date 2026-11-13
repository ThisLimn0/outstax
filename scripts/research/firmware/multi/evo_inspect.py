#!/usr/bin/env python3
"""Read-only structural validation, kernel and initramfs recovery."""
import ctypes,hashlib,json,struct,zlib,collections,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_OUT, multi_generated, out_dir
ROOT=out_dir(MULTI_OUT/'research/multi/evo-derived')
def src(n):return multi_generated('research/multi/evo-derived/'+n)
def sha(b): return hashlib.sha256(b).hexdigest()
def dtb(b):
 magic,total,so,sto,ro,ver,compat,cpu,ss,sz=struct.unpack_from('>10I',b);assert magic==0xd00dfeed
 names=b[sto:sto+ss];p=so;stack=[];props=[]
 while p<so+sz:
  tok=struct.unpack_from('>I',b,p)[0];p+=4
  if tok==1:
   end=b.index(0,p);stack.append(b[p:end].decode(errors='replace'));p=(end+4)&~3
  elif tok==2:stack.pop()
  elif tok==3:
   n,no=struct.unpack_from('>II',b,p);p+=8;name=names[no:names.index(0,no)].decode();v=b[p:p+n];p=(p+n+3)&~3
   props.append(dict(path='/'.join(stack),name=name,value_hex=v.hex(),text=v.rstrip(b'\0').decode('ascii',errors='replace') if all(c==0 or 32<=c<127 for c in v) else None))
  elif tok==9:break
  elif tok!=4:raise ValueError(tok)
 return dict(size=total,version=ver,properties=props)
def cpio(b,out):
 p=0;records=[];out.mkdir(exist_ok=True)
 while b[p:p+6] in [b'070701',b'070702']:
  h=[int(b[p+6+i*8:p+14+i*8],16) for i in range(13)];namesz=h[11];size=h[6];name=b[p+110:p+110+namesz-1].decode();p=(p+110+namesz+3)&~3;d=b[p:p+size];p=(p+size+3)&~3
  if name=='TRAILER!!!': return records,p
  mode=h[1];records.append(dict(path=name,mode=oct(mode),size=size,sha256=sha(d)))
  # Do not create device nodes or follow symlinks, and constrain archive paths.
  if mode&0xf000==0x8000 and '..' not in Path(name).parts and not name.startswith('/') and not Path(name).anchor:
   dst=out/name;dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes(d)
 raise ValueError('missing cpio trailer')
def main():
 j={};j['dtb']=dtb(src('part-01.bin').read_bytes());j['fastboot_dtb']=dtb(src('part-08.bin').read_bytes())
 k=zlib.decompress(src('part-12.bin').read_bytes()[16:]);(ROOT/'kernel.bin').write_bytes(k);j['kernel']=dict(size=len(k),sha256=sha(k),bcl1_header=src('part-12.bin').read_bytes()[:16].hex())
 initoff=k.find(b'\x02\x21\x4c\x18');assert initoff>=0;pos=initoff+4;chunks=[];lib=ctypes.CDLL('liblz4.so.1');f=lib.LZ4_decompress_safe;f.argtypes=[ctypes.c_char_p,ctypes.c_void_p,ctypes.c_int,ctypes.c_int];f.restype=ctypes.c_int
 while True:
  n=struct.unpack_from('<I',k,pos)[0];pos+=4
  if not n:break
  dst=ctypes.create_string_buffer(8*1024*1024);z=f(k[pos:pos+n],dst,n,len(dst));assert z>0;chunks.append(dst.raw[:z]);pos+=n
 c=b''.join(chunks);(ROOT/'initramfs.cpio').write_bytes(c);rec,end=cpio(c,ROOT/'initramfs');j['initramfs']=dict(kernel_offset=initoff,size=len(c),sha256=sha(c),records=rec,cpio_end=end)
 counters=collections.Counter();per=[]
 for pid in [13,14,18]:
  b=src(f'part-{pid:02}.ubi').read_bytes();stats=collections.Counter()
  for off in range(0,len(b)-64,131072):
   ec=b[off:off+64]
   if ec[:4]!=b'UBI#':continue
   stats['ec_headers']+=1;stats['ec_crc_valid']+=(zlib.crc32(ec[:60])^0xffffffff)==int.from_bytes(ec[60:64],'big')
   vo=int.from_bytes(ec[16:20],'big');v=b[off+vo:off+vo+64]
   if v[:4]==b'UBI!':
    stats['vid_headers']+=1;stats['vid_crc_valid']+=(zlib.crc32(v[:60])^0xffffffff)==int.from_bytes(v[60:64],'big')
  start=0
  while True:
   p=b.find(b'\x31\x18\x10\x06',start)
   if p<0:break
   start=p+4
   if p+24>len(b):continue
   crc,sq,n,typ,group=struct.unpack_from('<IQIBB',b,p+4)
   if n<24 or n>126976 or p+n>len(b):continue
   stats['ubifs_nodes']+=1;stats['ubifs_crc_valid']+=(zlib.crc32(b[p+8:p+n])^0xffffffff)==crc;stats[f'node_type_{typ}']+=1
   if typ==1 and n>=48: stats[f'data_compression_{struct.unpack_from("<H",b,p+44)[0]}']+=1
  counters.update(stats);per.append(dict(id=pid,**stats))
 j['ubi']=dict(eraseblock_size=131072,parts=per,total=dict(counters));(ROOT/'inspection.json').write_text(json.dumps(j,indent=2)+'\n');print(json.dumps({k:v for k,v in j.items() if k in ['kernel','ubi']},indent=2));print('initramfs',initoff,len(c),len(rec))
if __name__=='__main__':main()
