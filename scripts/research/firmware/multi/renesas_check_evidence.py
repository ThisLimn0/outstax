#!/usr/bin/env python3
"""Check critical claims as exact bytes against hash-pinned firmware images."""
from pathlib import Path
import hashlib,json,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi, multi_generated, multi_out
CHECKS={
'mini2':{'sha':'a9f40e948b3c4a05b06bc9f37bbe98ef68ca585d96d10f4cd47684debae22490','checks':{
'handshake payload':(0xfff0d5b9,'0100010000000000'),'model identifier':(0xfff06307,'464930323300'),'header':(0xfff00000,'422d6f4201000000'),'version footer':(0xfffdfff0,'422d6f4200010000'),'parser prologue':(0xfff55e9b,'7100c4'),'Ab comparison':(0xfff55fd1,'76016241'),'registry immediate':(0xfff55ff9,'fb126c08f1ff'),'primary +8 / jsr':(0xfff56555,'ad92a8a27f12'),'continuation +12 / jsr':(0xfff565b8,'ad92a8aa7f12'),'image maximum':(0xfff57cd9,'7701005802'),'image block size':(0xfff57bf1,'f90a068403'),'updater block size':(0xfff58b27,'f90a030004')}},
'mini3':{'sha':'7c384d92a7e420b5f74707974ab0af9d147ba47bc04551d4f8b5c14058fcb84a','checks':{
'handshake payload':(0xfff0f32f,'0100020000000000'),'model identifier':(0xfff08707,'464930333300'),'header':(0xfff00000,'422d6f4206000000'),'version footer':(0xfffdfff0,'422d6f4202010000'),'parser prologue':(0xfff6913e,'7100c4'),'Ab comparison':(0xfff69274,'76016241'),'registry immediate':(0xfff6929c,'fb12ecd3f1ff'),'primary +8 / jsr':(0xfff697f8,'ad92a8a27f12'),'continuation +12 / jsr':(0xfff6985b,'ad92a8aa7f12'),'image byte2 nonzero rejection':(0xfff6b14c,'a881b1916101173c060138a300'),'ordinary image maximum':(0xfff6b18c,'7701005802'),'later larger branch literal':(0xfff6b1d3,'770100b004'),'image block size':(0xfff6b076,'f90a078403'),'updater block size':(0xfff6c021,'f90a038403')}}}
results=[]
for name,p in CHECKS.items():
 b=multi(f'research/multi/inputs/{name}/{name}.bin').read_bytes();assert hashlib.sha256(b).hexdigest()==p['sha']
 for label,(va,expected) in p['checks'].items():
  wanted=bytes.fromhex(expected);got=b[va-0xfff00000:va-0xfff00000+len(wanted)];assert got==wanted,(name,label,hex(va),got.hex(),expected)
  results.append({'profile':name,'claim':label,'address':hex(va),'bytes':expected,'passed':True})
for name,rows,unique,block in [('mini2',34,32,1035),('mini3',46,43,911)]:
 o=json.loads(multi_generated('report/multi/renesas-'+name+'.json').read_text());assert o['record_count']==rows and o['unique_opcode_count']==unique
 for r in o['descriptors']:
  if r['opcode']=='0x1001':assert r['request_frame_bytes']==911
  if r['opcode']=='0x2001':assert r['request_frame_bytes']==block
 out={}
 for r in o['descriptors']:out.setdefault(r['opcode'],[]).append(r)
 for op,rs in out.items():assert len({r['raw_hex'] for r in rs})==1,(name,op,'nonidentical duplicate')
multi_out('report/multi/renesas-evidence-checks.json').write_text(json.dumps({'checks':results,'exact_byte_checks':len(results),'registry_checks_passed':True},indent=2)+'\n')
print('Passed',len(results),'hash-pinned exact-byte checks and both registry invariants.')
