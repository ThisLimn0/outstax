#!/usr/bin/env python3
"""Read-only static update/auxiliary-command extraction. No device I/O."""
import sys,struct,json,hashlib
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1,need,out_dir
from capstone import Cs,CS_ARCH_ARM,CS_MODE_THUMB,CS_MODE_LITTLE_ENDIAN
from capstone.arm import ARM_OP_MEM,ARM_REG_PC
BASE=0x08020000;b=need(MINI1/'mini1-0104.bin').read_bytes();SHA='b04e3490f57f93f562edc868dc34cc06ce670dd41476bf91782e9519aefcda4c';assert hashlib.sha256(b).hexdigest()==SHA
out=out_dir();md=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);md.detail=True
hx=lambda x:f'0x{x:08x}'
def u32(a):return struct.unpack_from('<I',b,a-BASE)[0]
def dis(a,n):
 ret=[]
 for i in md.disasm(b[a-BASE:a-BASE+n],a):
  r={'address':hx(i.address),'bytes':i.bytes.hex(),'instruction':i.mnemonic+' '+i.op_str}
  for op in i.operands:
   if op.type==ARM_OP_MEM and op.mem.base==ARM_REG_PC:
    p=((i.address+4)&~3)+op.mem.disp
    if BASE<=p<=BASE+len(b)-4:r['literal']={'address':hx(p),'value':hx(u32(p))}
  ret.append(r)
 return ret
ops=[]
for k in range(8):
 a=0x08023d74+16*k;op,addr,ptr,n=struct.unpack_from('<4I',b,a-BASE)
 ops.append({'record_address':hx(a),'operation_raw':hx(op),'operation':{0:'read',1:'page program',2:'erase'}.get(op,'sentinel'),'flash_address':hx(addr),'buffer':hx(ptr),'byte_count':n,'raw_hex':b[a-BASE:a-BASE+16].hex()})
assert [(int(x['operation_raw'],16),int(x['flash_address'],16),int(x['buffer'],16),x['byte_count'])for x in ops]==[(2,0x40000,0,0x100000),(1,0x40000,0xc00e0000,0x100000),(0,0x40000,0xc01e0000,0x100000),(2,0x9000,0,0x1000),(1,0x9000,0x2000082c,32),(2,0xa000,0,0x1000),(1,0xa000,0x2000082c,32),(0xffffffff,0,0,0)]
cmds=[
 {'opcode':'0x2000','request_frame_bytes':15,'payload':'size_BE32,target_BE32','reply_payload':'1024_BE32','handler':'0x0805c408','checks':['guard(request,3)','target>=0x08000000','uint32(target+size-1)<0x08100000','target%256==0','1<=size<=0x100000','transfer_state in {0,1}','protocol_phase in {0,5,6}'],'mutations':['transfer_state=1;target/size saved;ready=verified=0','blocksize=1024;last_sequence=(size-1)//1024;first_seen=last_seen=0','generic successful response sets protocol_phase=5'],'limits':'Target addition is32-bit; there is no independent target<0x08100000 comparison here. It does not clear the entire1MiB staging buffer on this traced setup path.'},
 {'opcode':'0x2001','request_frame_bytes':1035,'payload':'sequence_BE32,data[1024]','reply_payload':'sequence_BE32','handler':'0x0805c592','checks':['guard(request,3)','sequence<=last_sequence','transfer_state==1','protocol_phase in {5,6}'],'copy':'min(1024,size-sequence*1024) bytes to0xc00e0000+sequence*1024','mutations':['if first_seen==0:first_seen=(sequence==0);else if last_seen==0:last_seen=(sequence==last_sequence)','generic successful response sets protocol_phase=6'],'limits':'No complete received-block bitmap appears in this handler. A single-block transfer needs two sequence0 calls to set both markers under the literal if/else logic.'},
 {'opcode':'0x2002','request_frame_bytes':7,'payload':'none','reply_payload':'none','handler':'0x0805c6f4','checks':['guard(request,3)','transfer_state==1','protocol_phase==6','first_seen && last_seen'],'mutations':['first_seen=last_seen=0;transfer_state=0;ready=1;verified=0','generic successful response sets protocol_phase=0']},
 {'opcode':'0x2010','request_frame_bytes':7,'payload':'none','reply_payload':'u16_BE from0x2000fdcc','handler':'0x0805c3a8','limits':'Exact schema known; semantic name only generation/counter candidate. Not established as the application header version.'},
 {'opcode':'0x2080','request_frame_bytes':7,'payload':'none','initial_reply_payload':'estimated_time_u8(helper0x08048304 argument0)','handler':'0x0805c7c4','continuation':'0x0805ca5c','checks':['guard(request,3)','async_state[7]==0','transfer_state==0','verified==0','ready!=0 else0xa0','global_status!=0x7f else1','0x08047e04 and0x08047e2a succeed else0xa2'],'mutations':['transfer_state=3;operation_pointer=0x08023d74','submit firstNOR operation;global_status=0x7f;return internal0x10 (wire success, keep continuation)','build32-byte metadata;advance operation_pointer16 after each submit','after successful callback with sentinel next:compare0x40000 words(1MiB) staging/readback;set verified=1 only if identical','release resource lock,transfer_state=0,reset async state;global_status=0 on success or0xf0 on failure'],'limits':'The traced integrity check is a wordwise readback comparison, not an image checksum/signature. Operation lengths are fixed1MiB independent of declared transfer size. No final success frame is generated when continuation returns internal0x13; generic dispatcher frees request.'},
 {'opcode':'0x2081','request_frame_bytes':7,'payload':'none','initial_reply_payload':'BE16(uint16(current0x2000fdcc+1)),estimated_time_u8(helper0x08048304 argument1)','handler':'0x0805c97e','continuation':'0x0805cb88','checks':['guard(request,3)','async_state[7]==0','transfer_state==0','verified!=0','global_status!=0x7f'],'mutations':['request[0x26]=1 to attach transmit completion callback;return internal0x10(wire success)','transmit callback0x080545e8 sets request[0x27]=1','continuation waits request[0x27],delay100,save persistent state via0x08047a70,call0x0805cbb0','0x0805cbb0 changes task priority to6,watchdog feeds,0x08050304,CPSIDi,deinit0x0803fada/0x0803faee,BLX pointer loaded from0x1000ffd0'],'limits':'Target pointer contents/source are outside this statically recovered path. Actual bootloader/internal-flash programming or hardware reset is not present in this supplied application span.'},
 {'opcode':'0x3000','request_frame_bytes':7,'payload':'none','reply_payload':'word0_BE16,word1_BE16,word2_BE16,flag_bool_u8','handler':'0x080703b6','sources':['six bytes copied from0x2000e186 by0x080583a4','read-and-clear signed byte flag0x2000022d through0x0805783a,normalized to0/1'],'limits':'Sensor/axis names and signed units unresolved; query flag has read-to-clear side effect.'},
 {'opcode':'0x3001','request_frame_bytes':'11+3*N','payload':'mode_u8,N_u8,timing_u8,repeat_u8,triples[N][3]','reply_payload':'none','handler':'0x0805cca8','checks':['frame_length==11+3*N','0<=N<=100','mode<4','1<=timing<=100','globaloperation&0xff00 !=0x1000 else1'],'mutations':['readmode,N,timing before checking length;readrepeat ifvalid','ifmode==3:flag=1 and repeat forced0;elseflag=0','wire triples[x,y,z] stored internally[z,y,x] at0x2000085c','submit12-byte structure0x20000850 to0x0804a5e0','patternconsumer0x0804ab2e calls0x0804b154 which transforms3bytes usingthree256-entry u16 LUTs at0x080240ac and writesTIM8CCR4,CCR2,CCR3'],'limits':'Exact color wiring and time units unknown. Three-channel PWM pattern is established;RGB LED is probable only. N==0 is accepted and lower layer substitutes default pattern.'},
 {'opcode':'0x3002','request_frame_bytes':9,'payload':'selector_u8,value_u8','reply_payload':'selected_value_u8','handler':'0x0805ce2c','checks':['selector<3','ifselector==2:value&0xfc==0'],'behavior':{'0':'replyRAMbyte0x200100e8','1':'replyconstantbyte0x080fcbcc','2':'RAM0x200100e8=constant&value;replyunmaskedvalue'},'limits':'Meaning of the two bits unresolved.'},
 {'opcode':'0x3080','request_frame_bytes':7,'payload':'none','reply_payload_bytes':13,'reply_payload_fields':[{'offset':0,'type':'u8','source':'signedhardwareselector0x200064d0,returnedbyte'},{'offset':1,'type':'u8','source':'0x2000fb2c'},{'offset':2,'type':'u8','source':'constant0'},{'offset':3,'type':'BE16','source':'0x2000fb2e'},{'offset':5,'type':'u8','source':'0x2000fb30'},{'offset':6,'type':'u8','source':'0x2000fb31'},{'offset':7,'type':'BE16','source':'0x2000facc'},{'offset':9,'type':'BE16','source':'0x2000face'},{'offset':11,'type':'BE16','source':'0x2000fad0'}],'handler':'0x0805cf0c','limits':'Raw schema is exact; settings/calibration semantic labels unresolved.'},
 {'opcode':'0x3081','request_frame_bytes':13,'payload':'word0_BE16,word1_BE16,word2_BE16','reply_payload':'none','handler':'0x0805d010','checks':['ifhardwareselector0:eachword<64 else0x81','ifselector!=0:valuesareignoredwithsuccess','ifselector0andCCMflags0x1000ffc8bit11clear:commit;otherwiseignoredwithsuccess'],'mutations':['16-byte structure at0x2000fdb4 =B-oB,u16word0,u16word1,u16word2,u16zero,B-oB','0x08048054 usesop3(erase4KiBthenprogram) to persist16bytes atNOR0x30000 and0x31000 ifruntime0x2000f2ce!=0'],'limits':'What the three words control is not established.'}
]
spans={'query2010':(0x0805c3a8,0x5c),'setup2000':(0x0805c408,0x186),'block2001':(0x0805c592,0x15e),'finalize2002':(0x0805c6f4,0xcc),'stage2080':(0x0805c7c4,0x1ba),'activate2081':(0x0805c97e,0xda),'stage_continue':(0x0805ca5c,0xf8),'activate_continue_handoff':(0x0805cb88,0x58),'resource_lock':(0x08047e04,0x7e),'async_callbacks':(0x08058c48,0xce),'spi_operation_submit':(0x0804375e,0x60),'spi_dispatch':(0x0804368a,0xcc),'response_internal_results':(0x0805400c,0x50),'response_async_normalize':(0x080542b6,0x26),'generic_transfer_states':(0x0805443c,0x64),'response_completion_callback':(0x08054518,0x2c),'response_completed':(0x080545e8,0x20),'query3000':(0x080703b6,0xc0),'pattern3001':(0x0805cca8,0x174),'bits3002':(0x0805ce2c,0xd4),'query3080':(0x0805cf0c,0xfc),'persist3081':(0x0805d010,0xf4),'query3000_words':(0x080583a4,0x14),'query3000_flag':(0x0805783a,0x2c),'pattern_submit':(0x0804a5e0,0x1e8),'pattern_consumer':(0x0804ab28,0x3a),'pwm_outputs':(0x0804b154,0x32),'persistent3words':(0x08048054,0x30),'guard3_special_case':(0x0805925c,0x2a)}
evidence={k:dis(a,n)for k,(a,n)in spans.items()}
obj={'image_sha256':SHA,'base':hx(BASE),'method':'Read-only Capstone5.0.9 Thumb static disassembly; no execution,emulation,flashing or command transmission. Names are analyst labels.','scope':'200x/208x update family plus300x/308x auxiliary commands','commands':cmds,'nor_stage_operation_table':ops,'transfer_fields':{'base':'0x20000818','+0':'u8 transfer_state(0idle,1receiving,3staging)','+4':'u32 target internal address metadata','+8':'u32 declared total size','+0x10':'u8 ready(after2002)','+0x11':'u8 staged_and_readback_verified'},'protocol_fields':{'base':'0x2001031c','+0':'first_seen_u8','+1':'last_seen_u8','+4':'u32phase(0idle,5setup,6data)','+8':'u32blocksize','+0xc':'u32last_sequence','+0x88':'callbacktable'},'stage_metadata':{'ram':'0x2000082c','bytes':32,'nor_copies':['0x00009000','0x0000a000'],'fields':[{ 'offset':0,'bytes':16,'meaning':'copycurrentmetadata0x2000fdc4,thenbyteoffsets7and10cleared'},{'offset':16,'type':'u32le','value':'0x426f2d42(B-oB)'},{'offset':20,'type':'u32le','meaning':'targetfrom2000'},{'offset':24,'type':'u32le','meaning':'sizefrom2000'},{'offset':28,'type':'u32le','value':'0x426f2d42(B-oB)'}]},'state_machine':['2000setup->phase5,transfer1','2001blocks->phase6;first/lastmarkers','2002->phase0,transfer0,ready1,verified0','2080->transfer3,NORoperations,readbackcompare->transfer0,verified1','2081->initialsuccessreply->txcompletionflag->priorstagehandoffpointer'],'limitations':['Prior-stage image0x08000000..0x08020000 absent; cannot reconstruct actual internal flash installer.','NORreadbackverify establishesstoredbytesmatchRAM,notauthenticity/semanticvalidity.','Full1MiB staging/write/readback goes beyond declared size when size<1MiB;bytesbeyond declaredsize notclearedin2000path.','No runtimebehaviorvalidated;typedschemasandfunction labels are static deductions.'],'evidence':evidence}
import csv
pwm_start=0x080240ac
pwm_bytes=b[pwm_start-BASE:pwm_start-BASE+3*256*2]
(out/'pwm-curves.bin').write_bytes(pwm_bytes)
with(out/'pwm-curves.csv').open('w',newline='')as f:
 writer=csv.writer(f);writer.writerow(['channel_index','sample_index','flash_address','value_u16'])
 for channel in range(3):
  for sample in range(256):
   address=pwm_start+(channel*256+sample)*2
   writer.writerow([channel,sample,hx(address),struct.unpack_from('<H',b,address-BASE)[0]])
obj['pwm_curve_resource']={'start':hx(pwm_start),'bytes':len(pwm_bytes),'channels':3,
 'samples_per_channel':256,'storage':'little-endian uint16','raw_file':'pwm-curves.bin',
 'csv_file':'pwm-curves.csv','sha256':hashlib.sha256(pwm_bytes).hexdigest(),
 'channel_targets':['0x4001043c','0x40010434','0x40010438']}
(out/'update.json').write_text(json.dumps(obj,indent=2)+'\n')
with(out/'update-disassembly.txt').open('w')as f:
 for name,items in evidence.items():
  f.write('\n=== '+name+' ===\n')
  for x in items:
   ext=''
   if'literal'in x:ext=' ; ['+x['literal']['address']+']='+x['literal']['value']
   f.write(x['address']+': '+x['bytes'].ljust(10)+' '+x['instruction']+ext+'\n')
print('Extracted',len(cmds),'command schemas,',len(ops),'NOR records,',sum(map(len,evidence.values())),'evidence instructions.')
