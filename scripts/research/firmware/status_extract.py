#!/usr/bin/env python3
"""Reproduce mini Link 0104 status/identity/result evidence without device I/O."""
from pathlib import Path
import hashlib,json,struct,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1,need,out_dir
from capstone import Cs,CS_ARCH_ARM,CS_MODE_THUMB,CS_MODE_LITTLE_ENDIAN
from capstone.arm import ARM_OP_MEM,ARM_REG_PC
B=0x08020000
b=need(MINI1/'mini1-0104.bin').read_bytes()
digest=hashlib.sha256(b).hexdigest()
if digest!='b04e3490f57f93f562edc868dc34cc06ce670dd41476bf91782e9519aefcda4c':
    raise ValueError('different input snapshot')
def loc(a):return {'va':f'0x{a:08x}','file_offset':f'0x{a-B:x}'}
def byte(a):return b[a-B]
def word(a):return struct.unpack_from('<H',b,a-B)[0]
md=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);md.detail=True
ranges={
 'dispatch_results':(0x08053f18,0x1a6),
 'response_internal_result_modes':(0x0805428c,0xf4),
 'global_result_override':(0x0805420a,0x22),
 'response_transfer_state_changes':(0x08054380,0x12e),
 'payload_consumed_check':(0x080588d4,0xa6),
 'identity_handler':(0x080597fc,0xaa),
 'identity_selector_helper':(0x080598a6,0xba),
 'version_hex_ascii':(0x0805999c,0x4e),
 'status_handler':(0x0805ad5c,0x320),
 'status_counter_accessor':(0x0804c746,0x16),
 'counter_update':(0x0804df8a,0xdc),
 'status_aux_accessor':(0x080491e8,0xe),
 'time_estimate_accessor':(0x080482b0,0x46),
 'guard':(0x08059126,0x160),
 'guard_device_error_mapping':(0x08059286,0x60),
 'raw_fault_mask':(0x08059022,0x104),
 'transfer_expiry':(0x080540c0,0x8e),
 'ten_ms_task_calls_expiry':(0x0806fe50,0x42),
 'upload_marker_reset':(0x0805b94c,0x18),
 'upload_marker_updates':(0x0805baa6,0x3a),
 'upload_finalize_marker_requirement':(0x0805bbbc,0x26),
}
evidence={}
for name,(start,size) in ranges.items():
    ins=[]
    for i in md.disasm(b[start-B:start-B+size],start):
        row={**loc(i.address),'bytes':i.bytes.hex(),'instruction':i.mnemonic+' '+i.op_str}
        for op in i.operands:
            if op.type==ARM_OP_MEM and op.mem.base==ARM_REG_PC:
                a=((i.address+4)&~3)+op.mem.disp
                if B<=a<=B+len(b)-4:
                    row['literal']={**loc(a),'value':f'0x{struct.unpack_from("<I",b,a-B)[0]:08x}'}
        ins.append(row)
    evidence[name]={'start':loc(start),'byte_length':size,'instructions':ins}
identity=[
 {'selector':0,'length':byte(0x08039bae),'source':'0x0802b6cc','meaning':'manufacturer','snapshot_data_hex':b[0xb6cc:0xb6cc+byte(0x08039bae)].hex()},
 {'selector':1,'length':byte(0x0802e46b),'source':'0x0802b6b0','meaning':'model identifier','snapshot_data_hex':b[0xb6b0:0xb6b0+byte(0x0802e46b)].hex()},
 {'selector':2,'length':8,'source':'0x2000fb23','meaning':'opaque runtime bytes; serial/MAC meaning not established'},
 {'selector':3,'length':4,'source':'u16le[0x1000ffc4]','encoding':'four uppercase hexadecimal ASCII digits','meaning':'runtime version-like word; exact component unassigned'},
 {'selector':4,'length':4,'source':'u16le[0x08020004]','encoding':'four uppercase hexadecimal ASCII digits','snapshot_ascii':f'{word(0x08020004):04X}','meaning':'application header version'},
 {'selector':5,'length':4,'source':'u16le[0x080ffffa]','encoding':'four uppercase hexadecimal ASCII digits','snapshot_ascii':f'{word(0x080ffffa):04X}','meaning':'application footer version'},
 {'selector':6,'length':byte(0x08039baf),'source':'0x0803281b','meaning':'empty in this snapshot'},
 {'selector':7,'length':byte(0x08039c32),'source':'0x08032912','meaning':'empty in this snapshot'},
 {'selector':8,'length':byte(0x08039c33),'source':'0x08032913','meaning':'empty in this snapshot'},
]
results=[
 {'handler_result':0,'wire_before_override':0,'emit':'yes','retain_request':False},
 {'handler_result':16,'wire_before_override':0,'emit':'yes','retain_request':True,'role':'asynchronous accepted'},
 {'handler_result':18,'wire_before_override':None,'emit':'no','retain_request':True,'role':'continuation not complete'},
 {'handler_result':19,'wire_before_override':None,'emit':'no','retain_request':False,'role':'discard/finish without a response'},
 {'handler_result':127,'wire_before_override':127,'emit':'initial response or pending notification condition','retain_request':True},
 {'handler_result':126,'wire_before_override':127,'emit':'yes','retain_request':False},
]
schemas=[
 {'selector':0,'payload_bytes':11,'frame_bytes_on_success':19,'fields':['selector:u8','width:BE16=600','height:BE16=800','type:u8=2','reserved:u8=0','max_image:BE32=153600']},
 {'selector':1,'payload_bytes':5,'frame_bytes_on_success':13,'fields':['selector:u8','state_flags:u8=min(state,4)|(device_flag?8:0)','aux:u8=[0x20000998+8]>>8','raw_c:u8=[0x2000f244+0xc]','raw_d_flags:u8=[0x2000f244+0xd]|(([+0x18]&0x20)?0x10:0)|(([+0x18]&0x10)?0x20:0)']},
 {'selector':2,'payload_bytes':9,'frame_bytes_on_success':17,'fields':['selector:u8','counter_state_flags:u8','operation_flags:u8','status_code:u8=[0x20010504+1]','time_estimate:u8=min(255,(accessor0x080482b0()+([0x2000f2c8+0x8b4]==2?30:0))/10)','fault_mask:BE32=accessor0x08059022()']},
 {'selector':3,'payload_bytes':9,'frame_bytes_on_success':17,'fields':['selector:u8','counter_sum:BE32=u16[0x2000f2c8+0x8a4]+u16[+0x8a6]','counter:BE32=u16[0x2000f2c8+0x8a8]']},
]
guard_errors=[
 {'result':'0x90','condition':'u8[0x2000f21c+0x1a]==0'},
 {'result':'0x91','condition':'u8[0x2000f21c+0x1a]==5'},
 {'result':'0x92','condition':'s8[0x2000dd28+3]!=0'},
 {'result':'0x93','condition':'u8[0x2000dd28+4]|u8[+5] nonzero; or (u8[0x2000f244+0x18]|u8[+0x19])&0x58 nonzero'},
 {'result':'0xf0','condition':'s8[0x2000dd28+6]!=0; skipped in guard mode3'},
 {'result':'0xf1','condition':'s8[0x2000dd28+7]!=0; skipped in guard mode3'},
 {'result':'0xf2','condition':'s8[0x2000dd28+8]!=0; skipped in guard mode3'},
 {'result':'0x85','condition':'s8[0x2000dd28+9]!=0; skipped in guard mode3'},
 {'result':'0x01','condition':'operation/pending rejection; several branches including per-operation pending flags'},
]
out={
 'input_sha256':digest,'method':'Anchored ARM Thumb disassembly and literal data extraction; no firmware execution or printer communication.',
 'identity':{'opcode':'0x0001','request_payload':'selector:u8 (0..8)','response_payload':'selector:u8,length:u8,data[length]','invalid_selector_handler_result':'0x81','selectors':identity},
 'status':{'opcode':'0x0002','request_payload':'selector:u8 (0..3)','invalid_selector_handler_result':'0x81','response_schemas':schemas,
 'selector2_counter_state_flags':'low nibble counter from[0x2000f2c8+0x881], or0xf when s8[0x2000f288]!=0; OR min(state,4)<<4; OR0x80 when s8[0x2000f244+6]!=0',
 'selector2_counter_semantics':'Likely remaining film: producer resets10, decrements if nonzero, clears0. Not hardware-validated.0xf is an unknown/unready sentinel, not15 shots.',
 'selector2_operation_flags':'bit0 when[0x2000f21c+0x19]==1; ifu16[+0x1e]==0x0300 OR0xf4; else ifnonzero OR2 and additionally0x10 for highbyte0x10,0x20 forhighbyte0x11/0x12; else OR0xf4 if(raw_fault_mask&~0x40)!=0',
 'label_limits':'Battery, cover, temperature and exact physical fault names are not assigned solely from bit positions. Raw fields must be preserved.'},
 'handler_result_dispositions':results,
 'result_override':{'function':'0x0805420a','source':'u8[0x2000f21c+0x1a]','state0':'0x90','state5':'0x91','other_states':'preserve result after internal-mode normalization','note':'Internal0x12/0x13 never enter response builder, so override does not create responses for them.'},
 'guard':{'function':'0x08059126','mode_argument':'r1; observed call modes0..3','errors_in_precedence_order':guard_errors,'device_error_helper':'0x08059286; maps operation errorcode from[0x2000f21c+0x20] when currentoperation==0x0300','mode3':'Skips several error/operation checks but still checks pending flags and earlier state/device checks.'},
 'callback0':{'function':'0x080588d4','role':'request payload consumption check against receive buffer positions;0 means input consumed/ready','nonzero_handler_result':'0x13, no response emitted','limit':'Do not label this transport check as a physical printer fault.'},
 'transfer_timeout':{'counter':'u16[0x2001031c+0x22]','init_channel0_ticks':word(0x080fc902),'init_other_channel_ticks':word(0x080fc904),'initialized_by':'successful0x1000 response path','block_refresh':False,'decrement_function':'0x080540c0','caller':'0x0806fe58 inside ID_T10MS_TASK','expiry':'states1..4->state0,expiredflag[+0x20]=1; subsequent guard may return85','nominal_seconds_channel0':word(0x080fc902)/100,'nominal_seconds_other':word(0x080fc904)/100,'timing_confidence':'10ms inferred from named task; elapsed wall clock not measured. Counter is not refreshed for successful0x1001, so it is a transfer budget, not per-block inactivity.'},
 'single_block_transfer':{'observations':['0x1000 clears ctx+0 andctx+1','0x1001 uses if(!first_seen) first_seen=(seq==0); else if(!last_seen) last_seen=(seq==last)','0x1002 requires both flags'], 'conclusion':'For size<=900 (lastsequence0), two successfulseq0 submissions appear necessary before finalize along this isolated path.','limits':'Static path reconstruction; no hardware test or exhaustive alias-write proof. Not a verified device workaround.'},
 'evidence':evidence,
}
path=out_dir()/'status.json';path.write_text(json.dumps(out,indent=2)+'\n')
print(f'Wrote {path.name}: {sum(len(v["instructions"])for v in evidence.values())} instructions,9 identity selectors,4 status schemas.')
