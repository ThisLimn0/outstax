#!/usr/bin/env python3
"""Reproduce remaining mini Link 0104 control/readback command evidence."""
from pathlib import Path
import hashlib, json, struct, sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import MINI1,need,out_dir
from capstone import Cs,CS_ARCH_ARM,CS_MODE_THUMB,CS_MODE_LITTLE_ENDIAN
from capstone.arm import ARM_OP_MEM,ARM_REG_PC
BASE=0x08020000
b=need(MINI1/'mini1-0104.bin').read_bytes()
sha=hashlib.sha256(b).hexdigest()
if sha!='b04e3490f57f93f562edc868dc34cc06ce670dd41476bf91782e9519aefcda4c':raise ValueError('wrong image')
md=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);md.detail=True
def loc(a):return {'va':f'0x{a:08x}','file_offset':hex(a-BASE)}
commands=[
 {'opcode':'0x0010','label':'opaque device data query','request_frame_bytes':8,'payload':'selector:u8 (0..3)','response_payload':'selector:u8,length:u8,data[length]','notes':'Runtime/inhibit-dependent data. Selector2 formats numeric parameter words; selector3 reverses six lower-layer bytes on successful read. Not all fields are text.'},
 {'opcode':'0x0100','label':'power transition / shutdown request','request_frame_bytes':8,'payload':'mode:u8 (0 or1)','response_payload':'none','notes':'Updates power flags through0x0804c88a. Ifoperation is0 or0300, sends response then continuation posts1/2 basedCCMbit10. Withotheroperation it records deferred intent and returnsordinarysuccess. Exact physical variants unresolved.'},
 {'opcode':'0x0101','label':'software reboot request','request_frame_bytes':8,'payload':'mode:u8 (0 or1)','response_payload':'none','notes':'Mode1 maps tointernal4; rejects activeoperations except0/0300. Waitsresponsecompletion thenpostsevent3; BGcleanupcallsAIRCRSYSRESETREQ. Responseacceptance precedes reset.'},
 {'opcode':'0x0102','label':'four-word runtime/persistent parameter access','request_frame_bytes':19,'payload':'selector:u8,reserved[3],a:BE16,b:BE16,c:BE16,d:BE16','response_payload':'a:BE16,b:BE16,c:BE16,d:BE16 (no selector)','notes':'Selectors0..5. Eachreservedbyte isread into thesame scratchbyte, notvalidated. Validationappliesto suppliedwords evenforqueries. Physicalunitsunresolved.'},
 {'opcode':'0x0103','label':'four-word Bluetooth control tuple','request_frame_bytes':15,'payload':'a:BE16,b:BE16,c:BE16,d:BE16','response_payload':'none','notes':'6<=a<=b<=3200,c<500,10<=d<=3200. Calls0x080507ca; helpercreateslowerBTrequestwords0x22,0x3a with8-byte LEtuple. Handlerignoreshelperreturn. Unitsunresolved.'},
 {'opcode':'0x1010','label':'image readback setup','request_frame_bytes':7,'payload':'none','response_payload':'format:u8,flags:u8,reserved:BE16=0,size:BE32,block_size:BE32=900','notes':'Requires nonzero savedformat elseB3; protocolphase0/3/4. Clears first/lastmarkers, computeslastsequence fromsavedsize. DoesnotguaranteeJPEG: postdecodeformat10 contains1440000rawbytes.'},
 {'opcode':'0x1011','label':'image readback block','request_frame_bytes':11,'payload':'sequence:BE32','response_payload':'sequence:BE32,data[900] (zero-padded tail)','notes':'Requiresphase3/4, nonzerocounter,seq<=last. CopiesfromC06A0004+900*seq. First/lastmarkerif/else sameoneblocknuanceasupload.'},
 {'opcode':'0x1012','label':'image readback finalize','request_frame_bytes':7,'payload':'none','response_payload':'none','notes':'Requiresphase4,nonzerocounter,bothmarkers;clearsthosemarkers. Genericresponsepathclearsprotocolphase.'},
 {'opcode':'0x1081','label':'film reject/eject operation','request_frame_bytes':7,'payload':'none','response_payload':'none','notes':'Sharedguardcategory2 plusfilm/operationchecks. Submitsoperation1101 through0x08049dd8, returnsinternal10. ContinuationlogsRejectEnd> andupdatesglobalstatus; nosecondcompletionframe.'}
]
ranges={
 'query0010':(0x0805b090,0xa8),'query0010_selectors':(0x0805b13a,0xee),
 'numeric_parameter_formatter':(0x0805b244,0x1f4),
 'power0100':(0x0805b43c,0xbc),'power0100_continue':(0x0805b4fa,0x30),
 'reboot0101':(0x0805b534,0xce),'reboot0101_continue':(0x0805b608,0x20),
 'parameters0102':(0x0805b628,0x18e),'parameters0102_validation':(0x0805b7d0,0x76),
 'parameters_helper':(0x0804c980,0x72),'power_flags':(0x0804c88a,0x50),
 'bt0103':(0x08070174,0xa0),'bt0103_validation':(0x08070214,0x6a),
 'bt_tuple_request':(0x080507ca,0x38),
 'readback_setup':(0x0805bc74,0x13a),'readback_block':(0x0805bdb4,0x16e),
 'readback_finalize':(0x08070308,0xac),'readback_source':(0x0804c616,0x10),
 'zero_response_padding':(0x08058c1a,0x2e),
 'eject1081':(0x0805c1a0,0x160),'eject1081_continue':(0x0805c302,0x90),
 'reboot_event_write':(0x0804d26a,0x8),'periodic_event_relay':(0x0804ce96,0x20),
 'cleanup_schedule':(0x08052534,0x14),'cleanup_worker':(0x08052452,0xe2),
 'aircr_reset':(0x08052078,0x20),
 'readback_response_phases':(0x080543fc,0x40),
}
evidence={}
for name,(a,size)in ranges.items():
 lines=[]
 for i in md.disasm(b[a-BASE:a-BASE+size],a):
  item={**loc(i.address),'bytes':i.bytes.hex(),'instruction':i.mnemonic+' '+i.op_str}
  for o in i.operands:
   if o.type==ARM_OP_MEM and o.mem.base==ARM_REG_PC:
    p=((i.address+4)&~3)+o.mem.disp
    if BASE<=p<=BASE+len(b)-4:item['literal']={**loc(p),'value':hex(struct.unpack_from('<I',b,p-BASE)[0])}
  lines.append(item)
 evidence[name]={'start':loc(a),'byte_length':size,'instructions':lines}
obj={'input_sha256':sha,'method':'Manually anchored static Thumb disassembly; no execution or hardware commands.',
 'commands':commands,'parameter_default_words':list(struct.unpack_from('<4H',b,0x080fcbbc-BASE)),
 'parameter_validation_literal':'selector<6; each ofa/b/c is0 or>=1500; fourth test is d!=0 && a<1500 (notd<1500).',
 'parameter_selectors':{'0':'readpersistentinRAM0x2000fde4+4..+10','1':'readruntime0x200100d4+4..+10','2':'readflashdefault0x080fcbb8+4..+10','3':'refreshflagonly','4':'copy16Binputstructtoruntime;refresh','5':'copytoruntime;copysuppliedwordstopersistentRAM;conditionalredundantNORwrites0x7000/0x8000;refresh'},
 'readback':{'block_bytes':900,'source':'0xc06a0004','maximum_known_decoded_image_bytes':1440000,
 'single_block_note':'Localmarkerlogicrequiresfirstandlast inseparatecalls;repeatingsequence0appearsneededforoneblock,staticallyonly.',
 'timeout_note':'Readbackphases3/4useexistingcounter;unlike1000,1010doesnotreloadit ingenericresponsepath. Clientmustnotassumethissetupestablishesafreshdeadline.'},
 'evidence':evidence}
(out_dir()/'settings.json').write_text(json.dumps(obj,indent=2)+'\n')
print(f'{len(commands)} command schemas; {sum(len(x["instructions"])for x in evidence.values())} instruction records')
