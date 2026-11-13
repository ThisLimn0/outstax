#!/usr/bin/env python3
"""Static ARM symbols/disassembly; imported decoder handles host data only."""
import sys,json,io
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi, multi_out
from capstone import Cs,CS_ARCH_ARM,CS_MODE_ARM,CS_MODE_LITTLE_ENDIAN
from elftools.elf.elffile import ELFFile
R='research/multi/evo-derived/';p=multi(R+'rootfs-13/854005228/rootfs/lib/libhost.so');b=p.read_bytes();e=ELFFile(io.BytesIO(b));s=e.get_section_by_name('.dynsym');syms={x.name:x for x in s.iter_symbols() if x.name};md=Cs(CS_ARCH_ARM,CS_MODE_ARM|CS_MODE_LITTLE_ENDIAN);md.detail=True
byaddr={x.entry.st_value:x.name for x in syms.values() if x.entry.st_value};plt=e.get_section_by_name('.plt');rel=e.get_section_by_name('.rel.plt');dyn=e.get_section(rel['sh_link']);pltmap={plt['sh_addr']+20+12*i:dyn.get_symbol(x['r_info_sym']).name for i,x in enumerate(rel.iter_relocations())}
def bytes_at(v,n):
 for z in e.iter_segments():
  if z['p_type']=='PT_LOAD' and z['p_vaddr']<=v<v+n<=z['p_vaddr']+z['p_filesz']:return b[z['p_offset']+v-z['p_vaddr']:z['p_offset']+v-z['p_vaddr']+n]
 raise ValueError(hex(v))
names=['CheckFirmwareUpgradeCheckSum','mwCardUpdateFW','ab_fw_signature_verify','Jpg2PrintRgb','instax_3D_Exec3DLUT','Yuv2PrintRGB','FF_PrintDeviceNodeDeinit','FF_PrintInit','FF_PrintSetGpio17V','FF_PrintSetKernelInfo','FF_PrintGetKernelInfo','FF_PrintMotorStart','FF_PrintMotorStartFixDuty','FF_PrintSetPod','FF_Printer_Stop'];result={}
for name in names:
 x=syms[name];v=x['st_value'];instructions=[];calls=[]
 for ins in md.disasm(bytes_at(v,x['st_size']),v):
  r=dict(address=ins.address,mnemonic=ins.mnemonic,operands=ins.op_str)
  if ins.mnemonic in ['bl','blx'] and ins.op_str.startswith('#'):
   dest=int(ins.op_str[1:],0);r['target']=byaddr.get(dest,pltmap.get(dest,hex(dest)));calls.append(dict(address=ins.address,target=r['target']))
  instructions.append(r)
 result[name]=dict(address=v,size=x['st_size'],bytes=bytes_at(v,x['st_size']).hex() if x['st_size']<=16 else None,calls=calls,instructions=instructions)
multi_out(R+'code.json').write_text(json.dumps(result,indent=2)+'\n')
# Module has a full symtab. Preserve its names/addresses, not executable code.
mod=multi(R+'rootfs-13/854005228/rootfs/lib/modules/4.19.91/extra/ab_printer/ab_printer.ko')
with mod.open('rb') as f:
 me=ELFFile(f);rows=[dict(name=x.name,value=x['st_value'],size=x['st_size'],type=x['st_info']['type'],section=x['st_shndx']) for x in me.get_section_by_name('.symtab').iter_symbols() if x.name]
multi_out(R+'printer-symbols.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Wrote disassembly for', ', '.join(result))
