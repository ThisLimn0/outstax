#!/usr/bin/env python3
"""Reproduce raw-LZ4/SUNP resource and signature analysis; never executes firmware."""
from pathlib import Path
import argparse, hashlib, json, re, struct, sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_OUT, multi, multi_out, out_dir

def sha(b): return hashlib.sha256(b).hexdigest()
def raw_lz4(src):
    out=bytearray(); p=0; sequences=0
    while p<len(src):
        token=src[p];p+=1;lit=token>>4
        if lit==15:
            while True:
                x=src[p];p+=1;lit+=x
                if x!=255:break
        assert p+lit<=len(src),'truncated literals'
        out.extend(src[p:p+lit]);p+=lit;sequences+=1
        if p==len(src):break
        assert p+2<=len(src),'truncated offset'
        off=int.from_bytes(src[p:p+2],'little');p+=2
        assert 0<off<=len(out),'invalid match offset'
        n=(token&15)+4
        if (token&15)==15:
            while True:
                x=src[p];p+=1;n+=x
                if x!=255:break
        for _ in range(n):out.append(out[-off])
    return bytes(out),sequences

def fat_volumes(b):
    found=[]
    for match in re.finditer(b'\x55\xaa',b):
        base=match.start()-510
        if base<0:continue
        sec=struct.unpack_from('<H',b,base+11)[0]
        if sec not in (512,1024,2048,4096):continue
        spc=b[base+13];reserved=struct.unpack_from('<H',b,base+14)[0];nf=b[base+16]
        nr=struct.unpack_from('<H',b,base+17)[0];tot=struct.unpack_from('<H',b,base+19)[0] or struct.unpack_from('<I',b,base+32)[0]
        fs=struct.unpack_from('<H',b,base+22)[0]
        if not(spc in (1,2,4,8,16,32,64,128) and reserved and nf in (1,2) and nr and fs and tot and base+tot*sec<=len(b)):continue
        rootsectors=(nr*32+sec-1)//sec;clusters=(tot-reserved-nf*fs-rootsectors)//spc
        if not 0<clusters<65525:continue
        bits=12 if clusters<4085 else 16
        found.append(dict(base=base,base_hex=hex(base),bytes=tot*sec,sector_bytes=sec,sectors_per_cluster=spc,reserved=reserved,fat_copies=nf,root_entries=nr,fat_sectors=fs,cluster_count=clusters,fat_bits=bits))
    return found

def extract_fat(b,v,outdir,label):
    base=v['base'];sec=v['sector_bytes'];cs=sec*v['sectors_per_cluster'];fat=base+v['reserved']*sec
    root=fat+v['fat_copies']*v['fat_sectors']*sec;data=root+((v['root_entries']*32+sec-1)//sec)*sec
    rows=[]
    def chain(c):
        buf=bytearray();seen=set()
        while 2<=c<(0xff8 if v['fat_bits']==12 else 0xfff8):
            assert c not in seen,'FAT chain loop';seen.add(c)
            start=data+(c-2)*cs;assert base<=start<base+v['bytes'];buf.extend(b[start:start+cs])
            if v['fat_bits']==12:
                n=struct.unpack_from('<H',b,fat+c+c//2)[0];c=(n>>4 if c&1 else n&0xfff)
            else:c=struct.unpack_from('<H',b,fat+2*c)[0]
        return bytes(buf),sorted(seen)
    def walk(buf,path):
        lfn=[]
        for pos in range(0,len(buf),32):
            e=buf[pos:pos+32]
            if len(e)<32 or e[0]==0:break
            if e[0]==0xe5:lfn=[];continue
            if e[11]==15:lfn.append((e[0]&31,e[1:11]+e[14:26]+e[28:32]));continue
            name=e[:8].decode('ascii',errors='replace').rstrip();ext=e[8:11].decode('ascii',errors='replace').rstrip();name+=('.'+ext) if ext else ''
            if lfn:name=b''.join(x[1] for x in sorted(lfn)).decode('utf-16le',errors='replace').split('\0')[0].rstrip('\uffff')
            lfn=[]
            if name in ('.','..') or e[11]&8:continue
            c=struct.unpack_from('<H',e,26)[0];n=struct.unpack_from('<I',e,28)[0];payload,clusters=chain(c);full=path/name
            if e[11]&16:walk(payload,full)
            else:
                payload=payload[:n];assert len(payload)==n;target=outdir/full;assert target.resolve().is_relative_to(outdir.resolve()),full;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(payload)
                row=dict(volume=label,path=full.as_posix(),bytes=n,sha256=sha(payload),head=payload[:32].hex(),cluster=c,cluster_chain=clusters)
                if n in (514,1024,1260):
                    row['sum_le16_mod65536']=sum(struct.unpack('<'+'H'*(n//2),payload))&65535
                rows.append(row)
    walk(b[root:root+v['root_entries']*32],Path(label));v['files']=len(rows);return rows

def main():
    ap=argparse.ArgumentParser();ap.add_argument('model',choices=['square','wide2']);ap.add_argument('input',type=Path);a=ap.parse_args()
    src=multi(a.input).read_bytes();b,seq=raw_lz4(src);out=out_dir(MULTI_OUT/'research/multi/sunp-derived'/a.model);(out/'decoded.bin').write_bytes(b)
    vols=fat_volumes(b);files=[]
    for i,v in enumerate(vols):files+=extract_fat(b,v,out/'files',chr(65+i))
    assert len(b)%4==0
    tail_start=len(b)-1284;signature=b[tail_start:tail_start+256];metadata=b[-1028:-4]
    signed=b[:tail_start]+metadata;verify=[]
    for row in files:
        payload=(out/'files'/row['path']).read_bytes()
        if b'-----BEGIN PUBLIC KEY-----' in payload or b'-----BEGIN RSA PUBLIC KEY-----' in payload:
            from cryptography.hazmat.primitives.serialization import load_pem_public_key
            from cryptography.hazmat.primitives.asymmetric import padding
            from cryptography.hazmat.primitives import hashes
            key=load_pem_public_key(payload);nums=key.public_numbers();ok=True
            try:key.verify(signature,signed,padding.PKCS1v15(),hashes.SHA256())
            except Exception:ok=False
            verify.append(dict(path=row['path'],bits=key.key_size,e=nums.e,signature_valid=ok))
    nested=[];gammas=[];jpeg=[]
    for row in files:
        payload=(out/'files'/row['path']).read_bytes()
        if payload[8:12]==b'FIF0':
            count=struct.unpack_from('<I',payload,12)[0];cursor=(16+count*16+15)&~15;children=[]
            for i in range(count):
                e=payload[16+16*i:32+16*i];name=e[:14].split(b'\0')[0].decode('ascii');size=int.from_bytes(e[14:16],'little');child=payload[cursor:cursor+size];assert len(child)==size
                path=out/'nested'/row['path']/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(child)
                children.append(dict(name=name,offset=cursor,bytes=size,sha256=sha(child)));cursor=(cursor+size+15)&~15
            assert cursor==len(payload),(row['path'],cursor,len(payload))
            nested.append(dict(path=row['path'],magic_offset=8,table_offset=16,entry_bytes=16,name_bytes=14,length_endian='little',data_alignment=16,children=children))
        if len(payload)==514:
            values=struct.unpack('<257H',payload)
            gammas.append(dict(path=row['path'],entries=256,element_type='little-endian uint16',minimum=min(values[:256]),maximum=max(values[:256]),nondecreasing=all(x<=y for x,y in zip(values[:255],values[1:256])),checksum=values[-1],sum_le16_mod65536=sum(values)&65535,values=list(values[:256])))
        if payload.startswith(b'\xff\xd8'):
            from PIL import Image
            import io
            im=Image.open(io.BytesIO(payload));im.load();jpeg.append(dict(path=row['path'],format=im.format,width=im.width,height=im.height,mode=im.mode))
    header=dict(magic=b[:16].rstrip(b'\0').decode('ascii'),declared_end=struct.unpack_from('<I',b,16)[0],hdr1_offset=struct.unpack_from('<I',b,20)[0],hdr2_offset=struct.unpack_from('<I',b,24)[0],code_offset=struct.unpack_from('<I',b,28)[0],hdr3_offset=struct.unpack_from('<I',b,36)[0])
    trailer=None;start=header['declared_end']
    if start<tail_start:
        magic,total=struct.unpack_from('<II',b,start);assert magic==0xabb707ab;parts=[];end=0
        for i in range(11):
            rel,n=struct.unpack_from('<II',b,start+24+i*8);assert rel==end;end=rel+n;off=start+256+rel;payload=b[off:off+n];assert len(payload)==n
            name=f'{i:02d}.bin';(out/name).write_bytes(payload)
            tags=[x.decode('ascii') for x in re.findall(rb'RTL[0-9A-Z]+',payload[:512])]
            parts.append(dict(index=i,offset=off,offset_hex=hex(off),relative_payload_offset=rel,bytes=n,sha256=sha(payload),head=payload[:64].hex(),component_id=hex(int.from_bytes(payload[3:5],'little')) if i else None,tags=tags))
        assert end==total and start+256+total==tail_start
        trailer=dict(offset=start,offset_hex=hex(start),magic=hex(magic),header_bytes=256,payload_bytes=total,parts=parts)
    strings=[]
    for m in re.finditer(rb'[ -~]{8,}',b):
        if re.search(rb'SUNP|ThreadX|FIF0|FFProtocol|Realtek|RTL87|MIPS|GCC|SPCA|Sunplus',m.group(),re.I):strings.append(dict(offset=hex(m.start()),text=m.group().decode('ascii')[:240]))
    r=dict(model=a.model,input=a.input.as_posix(),compressed_bytes=len(src),compressed_sha256=sha(src),compression='single raw LZ4 block from offset 0',lz4_sequences=seq,decoded_bytes=len(b),decoded_sha256=sha(b),sunp_header=header,volumes=vols,files=files,nested_containers=nested,gamma_tables=gammas,jpeg_images=jpeg,bluetooth_trailer=trailer,signature=dict(offset=tail_start,offset_hex=hex(tail_start),bytes=256,metadata_offset=len(b)-1028,metadata_bytes=1024,checksum_offset=len(b)-4,signed_bytes=len(signed),signed_sha256=sha(signed),verification=verify),sum_le32_mod2_32=sum(struct.unpack('<'+'I'*(len(b)//4),b))&0xffffffff,strings=strings)
    report=multi_out('report/multi/'+a.model+'-evidence.json');report.write_text(json.dumps(r,indent=2)+'\n')
    print(json.dumps({k:v for k,v in r.items() if k not in ('files','strings','nested_containers','gamma_tables','bluetooth_trailer')},indent=2))
if __name__=='__main__':main()
