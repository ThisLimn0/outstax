#!/usr/bin/env python3
"""Independent end-to-end checks of the compiled offline packet tool.
Uses the included synthetic JPEG; it never opens a device or network socket.
The instax-mini1 crate is archived in the external package; its facts were
merged into crates/protocol. Build it first: cargo build --release --offline
--manifest-path <crate>/Cargo.toml (honours CARGO_TARGET_DIR).
"""
from pathlib import Path
import json, os, subprocess, sys, tempfile
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import CRATE, FIXTURES, need, out_dir
crate=need(CRATE)
binary=Path(os.environ.get('CARGO_TARGET_DIR') or crate/'target')/'release/instax-mini1'
fixture=need(FIXTURES/'gradient-600x800.jpg')
raw=fixture.read_bytes()
temporary=out_dir()/'.build-tmp';temporary.mkdir(exist_ok=True)
def call(*args):
 r=subprocess.run([str(binary),*map(str,args)],capture_output=True,text=True,check=True)
 return r.stdout
def request(path):
 data=path.read_bytes()
 if data[:2]!=b'Ab' or int.from_bytes(data[2:4],'big')!=len(data) or sum(data)%256!=255:
  raise ValueError('invalid exported frame')
 return int.from_bytes(data[4:6],'big'),data[6:-1]
if call('encode','request','0000').strip()!='41620007000055':raise ValueError('handshake vector mismatch')
if 'frames=1 rejected=0 discarded=0 pending=0'not in call('inspect','request','41620007000055'):
 raise ValueError('inspect failed')
with tempfile.TemporaryDirectory(dir=temporary)as name:
 out=Path(name)/'plan'
 call('plan-jpeg',fixture,out)
 files=sorted(out.glob('*.bin'))
 opcode,setup=request(files[0]);assert opcode==0x1000 and setup[:4]==bytes([2,0,0,0])
 assert int.from_bytes(setup[4:],'big')==len(raw)
 rebuilt=bytearray()
 for sequence,path in enumerate(files[1:-1]):
  opcode,payload=request(path)
  assert opcode==0x1001 and len(payload)==904
  assert int.from_bytes(payload[:4],'big')==sequence
  rebuilt.extend(payload[4:])
 assert rebuilt[:len(raw)]==raw and not any(rebuilt[len(raw):])
 assert request(files[-1])==(0x1002,b'')
 assert len(files)==(len(raw)+899)//900+2
result={'fixture':fixture.name,'fixture_bytes':len(raw),'unique_blocks':(len(raw)+899)//900,
 'checks':['hand-computed handshake vector','CLI inspect result','setup BE32 byte count',
 'all exported frame lengths and independent sumFF','consecutive BE32 sequence numbers',
 'full JPEG reconstruction from blocks','zero final padding','finalize opcode and empty payload'],
 'hardware_used':False,'outcome':'passed'}
(out_dir()/'rust-cli-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
