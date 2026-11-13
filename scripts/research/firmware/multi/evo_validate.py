#!/usr/bin/env python3
"""Verify recovered data against source hashes and independent structural totals."""
import hashlib,json,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi, multi_generated, multi_out
R='research/multi/evo-derived/'
def read(n):return json.loads(multi_generated(R+n+'.json').read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
c=read('container');i=read('inspection');f=read('files');j=read('catalogue');v=read('variant-comparison');sig=read('signature-verification');checks={}
checks['source_sha256']=sha(multi(c['input']))=='470c82342b590402a4828636426d468ab726c3339a5891f35d60c54f941be71f'
checks['decoded_sha256']=sha(multi_generated(R+'decoded.bin'))=='8801a0c190c0224ee61d9ab1934b4f120ec16a9d205b99e436455d1f792bff17'
checks['decoded_size']=multi_generated(R+'decoded.bin').stat().st_size==62428448
checks['block_count']=c['block_count']==953
checks['part_hashes']=all(sha(multi_generated(R+f"part-{p['id']:02}.bin"))==p['sha256'] for p in c['parts'])
checks['ec_crc']=i['ubi']['total']['ec_headers']==i['ubi']['total']['ec_crc_valid']==423
checks['vid_crc']=i['ubi']['total']['vid_headers']==i['ubi']['total']['vid_crc_valid']==423
checks['ubifs_crc']=i['ubi']['total']['ubifs_nodes']==i['ubi']['total']['ubifs_crc_valid']==30495
roots={13:multi(R+'rootfs-13/854005228/rootfs'),14:multi(R+'rootfs-14/1720163920/rootfs'),18:multi(R+'rootfs-18/1366069669/rootfs')}
checks['all_file_hashes']=all(sha(roots[x['partition']]/x['path'])==x['sha256'] for x in f if x['type']=='file')
checks['file_total']=sum(x['type']=='file' for x in f)==1542
checks['symlink_total']=sum(x['type']=='symlink' for x in f)==373
checks['arm_elf_count']=j['elf_count']==j['elf_arm_count']==174
checks['kernel_module_count']=j['kernel_modules']==103
checks['variant_prefix_and_trailer']=v['prefix_identical'] and v['trailer_size']==1280 and v['descending_32_to_1']
checks['signature_candidate_failures_recorded']=len(sig['tests'])==6 and all(not x['valid'] for x in sig['tests'])
result=dict(passed=all(checks.values()),checks=checks,regular_files_hashed=sum(x['type']=='file' for x in f));multi_out('report/multi/evo-validation.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));assert result['passed']
