#!/usr/bin/env python3
"""Install pinned official target stdlibs into the local compiler and cargo check.

Optional developer verification. Requires a local Rust installation under the
crate's .toolchain/installed directory, network access, and Python 3.12+.
This type-checks foreign targets; it does not link or execute their binaries.
The instax-mini1 crate is archived in the external package; its facts were
merged into crates/protocol. Results go to research/work/mini1-report.
"""
from pathlib import Path
import concurrent.futures, hashlib, json, os, subprocess, sys, tarfile, urllib.request
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import CRATE, need, out_dir

crate=need(CRATE)
meta=json.loads((crate/'cross-targets.json').read_text())
prefix=crate/'.toolchain/installed'
if not (prefix/'bin/cargo').exists():raise SystemExit('local pinned compiler installation is missing')
cache=crate/'.toolchain/cross';cache.mkdir(parents=True,exist_ok=True)
temporary=out_dir()/'.build-tmp';temporary.mkdir(exist_ok=True)
env=os.environ.copy();env['PATH']=str(prefix/'bin')+os.pathsep+env.get('PATH','');env['TMPDIR']=str(temporary)

def fetch(pair):
 target,item=pair;path=cache/(target+'.tar.xz')
 if not path.exists():
  with urllib.request.urlopen(item['xz_url'],timeout=40)as response,path.open('wb')as out:
   while chunk:=response.read(1024*1024):out.write(chunk)
 if hashlib.sha256(path.read_bytes()).hexdigest()!=item['xz_hash']:
  raise ValueError('target archive SHA-256 mismatch: '+target)
 print('Downloaded and verified '+target,flush=True)
 return target,path

with concurrent.futures.ThreadPoolExecutor(max_workers=3)as pool:
 results=list(pool.map(fetch,meta['targets'].items()))
checks=[]
for target,path in results:
 dest=cache/target;dest.mkdir(exist_ok=True)
 with tarfile.open(path,'r:xz')as archive:
  archive.extractall(dest,filter='data')
 installers=list(dest.glob('*/install.sh'))
 if len(installers)!=1:raise ValueError('unexpected installer layout')
 install=subprocess.run(['sh',str(installers[0]),'--prefix='+str(prefix),'--disable-ldconfig'],
                        env=env,capture_output=True,text=True)
 if install.returncode:raise RuntimeError(install.stderr+install.stdout)
 command=['cargo','check','--offline','--all-targets','--target',target]
 check=subprocess.run(command,cwd=crate,env=env,capture_output=True,text=True)
 checks.append({'target':target,'command':command,'exit_code':check.returncode,
                'stdout':check.stdout,'stderr':check.stderr,'execution':False,'linking':False})
 print(target+': '+('PASS'if check.returncode==0 else'FAIL'),flush=True)
 if check.returncode:raise RuntimeError(check.stderr)
(out_dir()/'cross-check-results.json').write_text(json.dumps({'manifest_url':meta['manifest_url'],
 'date':meta['date'],'checks':checks,'limitation':'Rust type-checks only; no foreign linking, execution or Bluetooth validation.'},indent=2)+'\n')
