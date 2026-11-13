#!/usr/bin/env python3
"""Generate function-anchored RX evidence windows using retained GNU objdump."""
from pathlib import Path
import subprocess,json,os,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_EXTERNAL, MULTI_MATERIAL, MULTI_OUT, multi, out_dir
TOOL=Path(os.environ.get('RX_OBJDUMP') or MULTI_EXTERNAL/'tools/multi/rx-objdump/rx-elf-objdump')
WINDOWS={
 'mini2':{'payload-accessors':(0xfff66fa9,0xfff673c0),'initialize-receive':(0xfff55bd8,0xfff55e9b),'parser-dispatch':(0xfff55e9b,0xfff56caf),'identity-status':(0xfff56caf,0xfff5725f),'image-setup':(0xfff57afd,0xfff57d09),'image-block-finalize':(0xfff57d09,0xfff5806e),'print':(0xfff5845b,0xfff58983),'standard-update':(0xfff58983,0xfff592b3),'settings':(0xfff592b3,0xfff59680)},
 'mini3':{'payload-accessors':(0xfff79485,0xfff7989c),'initialize-receive':(0xfff68e7b,0xfff6913e),'parser-dispatch':(0xfff6913e,0xfff6a01a),'identity-status':(0xfff6a01a,0xfff6a6ea),'image-setup':(0xfff6af88,0xfff6b203),'image-block-finalize':(0xfff6b203,0xfff6b568),'print':(0xfff6b955,0xfff6be7d),'standard-update':(0xfff6be7d,0xfff6c802),'additional-update':(0xfff95aa7,0xfff968b0),'settings':(0xfff6c802,0xfff6cc00)}}
for name,windows in WINDOWS.items():
 source=multi(f'research/multi/inputs/{name}/{name}.bin');dest=out_dir(MULTI_OUT/'research/multi/renesas-derived'/name)
 for label,(start,end) in windows.items():
  args=[str(TOOL),'-D','-b','binary','-m','rx','--adjust-vma=0xfff00000',f'--start-address={start:#x}',f'--stop-address={end:#x}',source.relative_to(MULTI_MATERIAL).as_posix()]
  p=subprocess.run(args,cwd=MULTI_MATERIAL,check=True,stdout=subprocess.PIPE)
  (dest/(label+'.txt')).write_bytes(p.stdout)
 (dest/'windows.json').write_text(json.dumps({n:{'start':hex(a),'end_exclusive':hex(z)} for n,(a,z) in windows.items()},indent=2)+'\n')
 print(name,len(windows),'evidence windows')
