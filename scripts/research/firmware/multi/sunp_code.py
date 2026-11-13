#!/usr/bin/env python3
"""Disassemble explicit file-offset windows as MIPS32 little-endian.

Address correspondence is code file offset 0x3b2b20 -> VA 0x80000000.
These are evidence windows, not a recursive whole-program disassembler.
"""
import argparse,json,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi_generated, multi_out
from capstone import Cs,CS_ARCH_MIPS,CS_MODE_MIPS32,CS_MODE_LITTLE_ENDIAN

def main():
    p=argparse.ArgumentParser();p.add_argument('model');p.add_argument('windows',nargs='*');a=p.parse_args()
    b=multi_generated(f'research/multi/sunp-derived/{a.model}/decoded.bin').read_bytes()
    windows=a.windows or (['0x3b2b20:0x180','0x52d788:0x250','0x57c568:0x250','0x57d598:0x650','0x578408:0x1c0','0x58528c:0x500','0x578ee4:0x750','0x57961c:0x550','0x5b2344:0x30','0x5b25b0:0x34','0x578828:0x64','0x577f08:0x94','0x578084:0x450','0x55c098:0x90','0x556ac4:0x28','0x553f50:0x30'] if a.model=='wide2' else ['0x3b2b20:0x200','0x57538c:0x650','0x57425c:0x350','0x5710d0:0x650','0x570af8:0x670','0x54ef50:0xb0','0x54f180:0x70','0x5706d8:0x1c0','0x5a7fb8:0x34'])
    md=Cs(CS_ARCH_MIPS,CS_MODE_MIPS32|CS_MODE_LITTLE_ENDIAN);rows=[];lines=[]
    for win in windows:
        off,n=(int(x,0) for x in win.split(':'));va=0x80000000+off-0x3b2b20
        lines.append(f'\nFILE {off:#x} VA {va:#x} length {n:#x}')
        for ins in md.disasm(b[off:off+n],va):
            f=off+ins.address-va
            rows.append(dict(file_offset=hex(f),va=hex(ins.address),bytes=ins.bytes.hex(),mnemonic=ins.mnemonic,operands=ins.op_str))
            lines.append(f'{f:08x} {ins.address:08x} {ins.bytes.hex()} {ins.mnemonic:10s} {ins.op_str}')
    target=multi_out('report/multi/'+a.model+'-code-windows');target.with_suffix('.json').write_text(json.dumps(rows,indent=2)+'\n');target.with_suffix('.txt').write_text('\n'.join(lines)+'\n');print(target)
if __name__=='__main__':main()
