#!/usr/bin/env python3
"""Generate checked runtime/resource evidence from an already acquired image.
No code execution or interaction with a printer. Analyst labels are provisional.
"""
import sys,struct,json,csv,hashlib,subprocess
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import INPUT_JSON,MINI1,need,out_dir
BASE=0x08020000
b=need(MINI1/'mini1-0104.bin').read_bytes();out=out_dir();out.mkdir(parents=True,exist_ok=True)
def u32(a):return struct.unpack_from('<I',b,a-BASE)[0]
def zstr(a):return b[a-BASE:].split(b'\0',1)[0].decode()
def hx(a):return f'0x{a:08x}'
tasks=[]
for i in range(17):
 nameaddr=u32(0x0802ad2c+4*i);a=0x0802aa14+24*i
 exinf,entry,size,stack,res,pri=struct.unpack_from('<6I',b,a-BASE)
 assert exinf==0 and res==0 and entry&1 and 0x08020000<entry<0x08071908 and 0x20000000<=stack<0x20020000
 assert u32(0x0802ac3c+4*i)==a
 tasks.append({'index':i,'name':zstr(nameaddr),'name_address':hx(nameaddr),'record_address':hx(a),'entry_thumb':hx(entry),'entry_code':hx(entry&~1),'stack_size':size,'stack_base':hx(stack),'priority_candidate':pri,'record_words':[hx(v)for v in (exinf,entry,size,stack,res,pri)],'confidence':'high for parallel table association; priority/stack semantics inferred from layout and runtime initialization'})
# Each three-curve family is 3*256 little-endian unsigned16 entries.
# Each companion family is 3 groups * 3 curves * 256 unsigned16 entries.
resources=[]
for family,kind,start,groups,curves in [('A','gain',0x080fcf80,1,3),('B','gain',0x080fd580,1,3),('A','lookup',0x080fdbf0,3,3),('B','lookup',0x080fedf0,3,3)]:
 size=groups*curves*256*2;data=b[start-BASE:start-BASE+size];assert len(data)==size
 stem=f'runtime-curves-{family.lower()}-{kind}';(out/(stem+'.bin')).write_bytes(data)
 with (out/(stem+'.csv')).open('w',newline='')as f:
  w=csv.writer(f);w.writerow(['family','kind','group_index','curve_index','sample_index','flash_address','value_u16'])
  for g in range(groups):
   for c in range(curves):
    for k in range(256):
     a=start+((g*curves+c)*256+k)*2;w.writerow([family,kind,g,c,k,hx(a),struct.unpack_from('<H',b,a-BASE)[0]])
 resources.append({'family':family,'kind_analyst_label':kind,'start':hx(start),'end_exclusive':hx(start+size),'bytes':size,'groups':groups,'curves_per_group':curves,'samples_per_curve':256,'storage':'little-endian uint16','sha256':hashlib.sha256(data).hexdigest(),'raw_file':stem+'.bin','csv_file':stem+'.csv'})
segments=[('Header and early stub',0x08020000,0x08020200),('Vector table (107 words)',0x08020200,0x080203ac),('Mixed code, constants and initialized data',0x080203ac,0x08071908),('All-zero fill',0x08071908,0x080fc8fc),('Actively referenced parameters and lookup tables',0x080fc8fc,0x080ffff0),('Zero footer reserved bytes',0x080ffff0,0x080ffffa),('Version and magic footer',0x080ffffa,0x08100000)]
assert set(b[0x08071908-BASE:0x080fc8fc-BASE])=={0}
assert b[0x080ffffa-BASE:]==bytes.fromhex('0401422d6f42')
funcs=[
 ('0x0802d762','JPEG decoder initialization wrapper','Calls error manager initializer0x0802dc0c; calls CreateDecompress-compatible0x0802dc5c with version90,size0x1e8; header parser0x0802df08 then start-decompress0x0802e164.','high'),
 ('0x0802d948','JPEG scanline conversion wrapper','Calls read-scanlines-compatible0x0802e2bc with maxlines1; stores byte channels in planes offset0/600/1200 with row stride1800.','high'),
 ('0x0802dc0c','IJG jpeg_std_error-compatible initializer','Fills standard error manager callbacks and points to embedded JPEG message table0x08029fb4.','high'),
 ('0x0802dc5c','IJG jpeg_CreateDecompress-compatible initializer','Checks version90 and structsize0x1e8, sets decompressor flag and installs decompressor components; direct app caller0x0802d790.','high'),
 ('0x080322bc','object-runtime initializer','Reads config0x0802ae8c, iterates typed object attributes and configuration/state tables; called from main0x0805218e.','high'),
 ('0x080435cc','ID_SPI_TASK entry','Parallel task-name/config association; waits for mailbox/queue requests then calls0x0804368a.','high'),
 ('0x0804368a','SPI task request dispatcher','Request op0->0x0804579a,op1->0x08045818,op2->0x080458d2; op3/4 erase then program.','high'),
 ('0x08045564','SPI2 handle setup fragment','Stores0x40003800 to handle0x2000d7c8 then calls0x080405b8.','high'),
 ('0x080455d4','serial-flash command transmit','Clears/sets PB12 through0x40020414 bit0x1000 around0x08040640 transport.','high'),
 ('0x080456c4','read serial-flash status register1','Writes opcode05 then reads response byte via0x08045612.','high'),
 ('0x080456ea','read serial-flash status register2','Writes opcode35 then reads response byte.','high'),
 ('0x08045710','read serial-flash status register3','Writes opcode15 then reads response byte.','high'),
 ('0x08045736','serial-flash 4KiB erase command','Opcode20 plus address encoded in three big-endian bytes.','high'),
 ('0x08045768','serial-flash 64KiB erase command','OpcodeD8 plus24-bit address.','high'),
 ('0x0804579a','serial-flash read','Opcode03 plus24-bit address; transport call0x08045656.','high'),
 ('0x08045818','serial-flash page program','Opcode02; splits on256-byte page boundaries.','high'),
 ('0x080458d2','serial-flash range erase','Validates4KiB alignment and nonzero length; rejects end above0x200000, uses4KiB/64KiB commands and busy polling.','high'),
 ('0x08045a10','serial-flash deep power-down command','OpcodeB9 via command transport.','high'),
 ('0x08045a22','serial-flash release power-down/read ID command','OpcodeAB; reads fifth exchange byte. Does not prove manufacturer/chip.','high'),
 ('0x08045c4e','USART6 handle setup fragment','Stores0x40011400 and115200 baud in handle0x2000dc08; called UART setup0x08041014.','high'),
 ('0x08045db2','Bluetooth-linked UART ring-buffer transmit','Command builder0x080599f6->0x0805a4b0->this routine->UART DMA transmit0x08043bae using USART6 handle.','high'),
 ('0x0804c53c','resource-copy function fragment','Signed nonzero selector chooses B else A; copies3*512-byte gain tables and9*512-byte lookup tables to RAM.','high'),
 ('0x0804c646','gain table pointer accessor','Returns RAM0x2000e7f0+channel*0x200; consumers use LDRH and scale product right by15.','high'),
 ('0x0804c654','lookup table pointer accessor','Returns0xc0017900+group*0x600+curve*0x200.','high'),
 ('0x0804c676','lookup table pointer accessor duplicate','Same arithmetic as0x0804c654; consumers at0x08048690/96 use unsigned LDRH and interpolation.','high')]
interrupts=[]
for n in range(107):
 val=u32(0x08020200+n*4)
 if val in [0x0803b399,0x0803b39b]:continue
 interrupts.append({'vector_index':n,'external_irq_index':n-16 if n>=16 else None,'vector_address':hx(0x08020200+4*n),'value':hx(val),'note':'raw vector value, no unverified IRQ symbolic assignment'})
obj={'input':json.loads(need(INPUT_JSON).read_text()),'method':'Read-only byte parsing and Capstone5.0.9 ARM Thumb little-endian disassembly; no firmware execution. All semantic function names are analyst labels.', 'rtos':{'classification':'ITRON-style typed object kernel; exact vendor/product unproven','confidence':'high for multitasking kernel and typed object model, medium for ITRON family','evidence':['17 task names plusFLG/DTQ/MBX/MPF names','parallel name table0x0802ad2c and24-byte config descriptors0x0802aa14','object initializer0x080322bc uses global config0x0802ae8c','SVC and interrupt-entry/exit support0x080399a4/0x080399ec'],'metadata_bytes':{'0x0802ae8c+0x1c':'4601050201000f00','0x0802d44c':'00004601035405020000000000000000'},'unverified':'Do not identify FreeRTOS,TOPPERS,NORTi,μC3 or exact kernel version from names alone.'},'tasks':tasks,'jpeg_dimension_modes':[{'record':hx(0x080203ac+20*i),'mode':struct.unpack_from('<H',b,0x3ac+20*i)[0],'input_width':struct.unpack_from('<H',b,0x3ae+20*i)[0],'input_height':struct.unpack_from('<H',b,0x3b0+20*i)[0],'output_width':struct.unpack_from('<H',b,0x3b2+20*i)[0],'output_height':struct.unpack_from('<H',b,0x3b4+20*i)[0],'scale_numerator':u32(0x080203b8+20*i),'scale_denominator':u32(0x080203bc+20*i)}for i in range(3)],'task_text_span':{'start':'0x0802a7bc','end_exclusive':'0x0802a9d6','note':'All17 task names followed by22FLG/DTQ/MBX/MPF object names, NUL-terminated and padded.'},'libraries':[{'name':'Independent JPEG Group JPEG9a','confidence':'high:embedded self-identification','evidence':{'0x08029780':'Copyright (C) 2014, Thomas G. Lane, Guido Vollbeding','0x080297b8':'9a  19-Jan-2014'},'active_call_linkage':['0x0802daca->0x0802d762 initialization','0x0802d790->0x0802dc5c(version90,size0x1e8)','0x0802d7a2->0x0802df08 header parser','0x0802d91c->0x0802e164 start decompress','0x0802d976->0x0802e2bc scanlines(max1)'],'limitation':'Function names are semantic matches inferred from IJG API layouts, version checks and state transitions; no recovered symbols. Does not show every codec feature is compiled or reachable.'}],'compiler':{'result':'Exact compiler/vendor/version unresolved','reason':'No unambiguous compiler version string found. Thumb-2/FPU startup/runtime shape alone is insufficient.'},'hardware':{'platform':'ARM Cortex-M with STM32F4-compatible peripheral map; exact MCU not proven by runtime analysis','spi':'SPI2 base0x40003800, PB12 active-low chip select, serial NOR command set with2MiB erase limit; exact flash chip unresolved','bluetooth':'Named BT tasks lead to USART6 handle base0x40011400, initial115200 baud and DMA/ring buffers; exact Bluetooth chip/module unresolved','usb':'Embedded product/manufacturer andCDC descriptors; USBD task exists, no external device interaction'},'functions':[{'address':a,'analyst_label':name,'evidence':e,'confidence':c}for a,name,e,c in funcs],'segments':[{'label':label,'start':hx(s),'end_exclusive':hx(e),'size':e-s}for label,s,e in segments],'resources':resources,'resource_selection':{'source':'0x0804b68c loads signed byte at0x200064d0','copied_selector':'0x200137cc','test':'0x0804c56a LDRSB then CMP0; zero=A, nonzero=B','gain_usage':'0x0805d5ce LDRH then MUL then LSR15','lookup_usage':'0x08048690/0x08048696 LDRH; subsequent signed difference/interpolation','semantic_limit':'Channel/group numbers are indices. Color names, head vendor names, electrical units and exact calibration meaning remain unproven.'},'interrupt_nondefault_values':interrupts,'interrupt_chains':[{'raw_irq_index':35,'chain':['0x0805f09c','0x0803b340','0x08040dce'],'handle':'0x2000d7c8','warning':'Raw slot35 normally corresponds SPI1 in common STM32F4 tables while handle is initialized toSPI2; preserve discrepancy, do not relabel asSPI2_IRQn.'},{'raw_irq_index':71,'chain':['0x0805f198','0x0803b362','0x08041156'],'handle':'0x2000dc08','label':'USART6-compatible chain'}],'embedded_container_magic_scan':{'JPEG_start_marker':0,'PNG_signature':0,'GIF87a':0,'GIF89a':0,'RIFF':0,'limitation':'No recognized signatures is not proof no compressed/raw images or fonts exist.'}}
(out/'runtime.json').write_text(json.dumps(obj,indent=2))
# Anchored assembly excerpts; start addresses deliberately chosen from manually checked paths.
spans=[(0x0802d762,0x48),(0x0802d7d4,0x4e),(0x0802dc0c,0x38),(0x0802dc5c,0x7e),(0x0802da80,0x54),(0x0802d970,0xc8),(0x080322bc,0xa0),(0x080435cc,0x7e),(0x0804368a,0x8c),(0x08045564,0x70),(0x080455d4,0xda),(0x080456ae,0x88),(0x08045736,0x64),(0x0804579a,0x144),(0x080458d2,0x140),(0x08045c4e,0x6c),(0x08045db2,0x9e),(0x08045e34,0x1e),(0x080599f6,0x106),(0x0805a4b0,0x5c),(0x0805f09c,0x1c),(0x0803b340,0x10),(0x0805f198,0x1c),(0x0803b362,0xe),(0x0804c560,0x88),(0x0804c646,0x50),(0x0805d5bc,0x1c),(0x0804868e,0x3a),(0x08044ef6,0x36),(0x08055d18,0x16)]
with (out/'runtime-evidence.txt').open('w')as f:
 for addr,size in spans:
  f.write(f'\n=== {hx(addr)} + {size:#x} ===\n')
  subprocess.run([sys.executable,str(Path(__file__).parent/'runtime_disasm.py'),hex(addr),hex(size)],stdout=f,check=True)
print(json.dumps({'tasks':len(tasks),'resource_files':len(resources)*2,'report_json':str(out/'runtime.json'),'evidence':str(out/'runtime-evidence.txt')},indent=2))
