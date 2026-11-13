#!/usr/bin/env python3
"""Static ARM driver disassembly; retain relocation targets without executing code."""
import sys,io,json,struct
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi, multi_out
from capstone import Cs,CS_ARCH_ARM,CS_MODE_ARM,CS_MODE_LITTLE_ENDIAN
from elftools.elf.elffile import ELFFile
R='research/multi/evo-derived/';p=multi(R+'rootfs-13/854005228/rootfs/lib/modules/4.19.91/extra/ab_printer/ab_printer.ko');e=ELFFile(io.BytesIO(p.read_bytes()));sy=e.get_section_by_name('.symtab');syms={x.name:x for x in sy.iter_symbols() if x.name};md=Cs(CS_ARCH_ARM,CS_MODE_ARM|CS_MODE_LITTLE_ENDIAN);relocs={}
for sec in e.iter_sections():
 if sec['sh_type'] not in ('SHT_REL','SHT_RELA'):continue
 tab=e.get_section(sec['sh_link']);dst=sec['sh_info']
 for r in sec.iter_relocations():relocs[(dst,r['r_offset'])]=tab.get_symbol(r['r_info_sym']).name
names=['ab_printer_ioctl','ab_printer_write','printer_send_line','ab_printer_send_line','printer_exposure_inline','printer_set_gain','ab_printer_exposure_start','printer_calculatePOD','ab_printer_mmap'];result={}
for name in names:
 s=syms[name];sec=e.get_section(s['st_shndx']);data=sec.data()[s['st_value']:s['st_value']+s['st_size']];instructions=[]
 for ins in md.disasm(data,s['st_value']):
  if name=='ab_printer_ioctl' and 0x10cc<=ins.address<0x125c:continue
  row=dict(address=ins.address,mnemonic=ins.mnemonic,operands=ins.op_str)
  if (s['st_shndx'],ins.address) in relocs:row['relocation_symbol']=relocs[(s['st_shndx'],ins.address)]
  instructions.append(row)
 result[name]=dict(section=sec.name,address=s['st_value'],size=s['st_size'],instructions=instructions)
table=e.get_section(syms['ab_printer_ioctl']['st_shndx']).data();dispatch=[dict(command_low_byte=i,destination=struct.unpack_from('<I',table,0x10cc+4*i)[0]) for i in range(100)];result['ioctl_dispatch']=dict(mask='low 8 bits (uxtb r1,r1)',max_index=99,default_destination=0x12ec,table=dispatch)
multi_out(R+'printer-code.json').write_text(json.dumps(result,indent=2)+'\n')
print('Wrote selected driver disassembly and 100-entry ioctl dispatch table')
