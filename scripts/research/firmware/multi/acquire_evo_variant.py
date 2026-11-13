#!/usr/bin/env python3
"""Acquire and compare the official WIDE Evo source-version updater variant."""
from pathlib import Path
import urllib.request,hashlib,json,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_MATERIAL, multi, write_input
source='https://www.instax.com/wide_evo/assets/firmware/Ver101/FPUPDATE.DAT'
path=MULTI_MATERIAL/'research/multi/inputs/wide-evo/FPUPDATE-Ver101.DAT'
with urllib.request.urlopen(urllib.request.Request(source,headers={'User-Agent':'Mozilla/5.0'}),timeout=90) as response:
 data=response.read()
sha=hashlib.sha256(data).hexdigest()
assert len(data)==51115071 and sha=='61f48235177db3fcc2e6a799363fcdebed64c35f6969ad146a3e69621d0905e8'
base=multi('research/contents/wide-evo.DAT').read_bytes()
assert data[:len(base)]==base and len(data)-len(base)==1280
path=write_input(path,data)
meta={'source':source,'path':path.relative_to(MULTI_MATERIAL).as_posix(),'bytes':len(data),'sha256':sha,'base_path':'research/contents/wide-evo.DAT','base_exact_prefix':True,'trailer_bytes':len(data)-len(base),'trailer_sha256':hashlib.sha256(data[len(base):]).hexdigest(),'note':'Ver101 identifies update source version path; it does not identify target version 1.01.'}
write_input(path.with_suffix('.json'),(json.dumps(meta,indent=2)+'\n').encode())
print(json.dumps(meta,indent=2))
