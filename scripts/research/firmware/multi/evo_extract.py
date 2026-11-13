#!/usr/bin/env python3
"""Static WIDE Evo DAT extraction; never executes vendor code.
Offsets in decoded directory are firmware logical offsets, including 128-byte
outer prefix, so subtract 128 to address the decompressed byte string.
"""
import argparse,ctypes,ctypes.util,hashlib,json,struct,zlib,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_OUT, multi, out_dir

def sha(b): return hashlib.sha256(b).hexdigest()
def decode(b):
 lib=ctypes.CDLL(ctypes.util.find_library('lz4')); f=lib.LZ4_decompress_safe_usingDict
 f.argtypes=[ctypes.c_char_p,ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_char_p,ctypes.c_int]; f.restype=ctypes.c_int
 p=128;out=bytearray();blocks=[]
 while True:
  n=struct.unpack_from('<I',b,p)[0];p+=4
  if not n: break
  dst=ctypes.create_string_buffer(65536);d=bytes(out[-65536:]);z=f(b[p:p+n],dst,n,65536,d,len(d))
  if z<0: raise ValueError(f'Invalid LZ4 block {len(blocks)}: {z}')
  blocks.append(dict(input_offset=p,compressed_size=n,output_offset=len(out),output_size=z));out.extend(dst.raw[:z]);p+=n
 return bytes(out),p,blocks

def main():
 ap=argparse.ArgumentParser();ap.add_argument('input');ap.add_argument('--out',default=str(MULTI_OUT/'research/multi/evo-derived'));a=ap.parse_args(); b=multi(a.input).read_bytes();out=out_dir(a.out);d,end,blocks=decode(b);(out/'decoded.bin').write_bytes(d)
 parts=[]
 for i in range(8):
  off,n,pid=struct.unpack_from('<III',d,i*12);p=d[off-128:off-128+n];assert len(p)==n;(out/f'part-{pid:02}.bin').write_bytes(p)
  rec=dict(id=pid,logical_offset=off,decoded_offset=off-128,size=n,sha256=sha(p),magic=p[:16].hex());parts.append(rec)
  u=p.find(b'UBI#')
  if u>=0 and pid in (13,14,18): (out/f'part-{pid:02}.ubi').write_bytes(p[u:]);rec['ubi_offset']=u
 j=dict(input=str(a.input),input_size=len(b),input_sha256=sha(b),prefix_size=128,stream_end=end,trailer_size=len(b)-end,decoded_size=len(d),decoded_sha256=sha(d),block_count=len(blocks),parts=parts);(out/'container.json').write_text(json.dumps(j,indent=2)+'\n');(out/'blocks.json').write_text(json.dumps(blocks,indent=2)+'\n');print(json.dumps(j,indent=2))
if __name__=='__main__':main()
