#!/usr/bin/env python3
"""Recompute the cross-project firmware input manifest without network access."""
from pathlib import Path
import hashlib,json,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_MATERIAL, multi, multi_out
ROOT='research/multi/inputs'
entries=[]
for p in sorted(multi(ROOT).glob('*/input.json')):
 o=json.loads(p.read_text());o['metadata_path']=p.relative_to(MULTI_MATERIAL).as_posix();entries.append(o)
mini1=json.loads(multi('research/mini1/input.json').read_text())
mini1.update(path='research/mini1/0104.hex',bytes=mini1['hex_bytes'],sha256=mini1['hex_sha256'],binary_path='research/mini1/mini1-0104.bin',metadata_path='research/mini1/input.json')
entries.append(mini1)
p='research/contents/mini-evo-official.DAT'
entries.append({'path':p,'source':'https://www.instax.com/mini_evo/common/data/FPUPDATE.DAT','bytes':multi(p).stat().st_size,'sha256':hashlib.sha256(multi(p).read_bytes()).hexdigest()})
for p in sorted(multi(ROOT).glob('wide-evo/FPUPDATE-Ver*.json')):
 entries.append(json.loads(p.read_text()))
lines=[]
for o in entries:
 for key,sizekey,hashkey in [('path','bytes','sha256'),('binary_path','binary_bytes','binary_sha256')]:
  if key not in o:continue
  p=o[key];d=multi(p).read_bytes();sha=hashlib.sha256(d).hexdigest()
  assert len(d)==o[sizekey] and sha==o[hashkey],p
  lines.append(sha+'  '+p)
manifest={'description':'All project source firmware artifacts, application routing evidence when acquired, and Intel HEX canonical binary forms; SHA256 recomputed from local bytes. DAT extraction outputs and compiler/build inputs are excluded.','inputs':entries}
multi_out(ROOT+'/manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
multi_out(ROOT+'/SHA256SUMS').write_text('\n'.join(sorted(set(lines)))+'\n')
print(f'Verified {len(lines)} firmware artifacts across {len(entries)} input records')
