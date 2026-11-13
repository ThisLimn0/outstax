#!/usr/bin/env python3
"""Reproduce static protocol table and selected disassembly evidence (no execution)."""
import hashlib,json,pathlib,struct,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(pathlib.Path(__file__).parent))
from _paths import MINI1,need,out_dir
from capstone import *
from capstone.arm import *
BASE=0x08020000
B=need(MINI1/'mini1-0104.bin').read_bytes()
assert hashlib.sha256(B).hexdigest()=='b04e3490f57f93f562edc868dc34cc06ce670dd41476bf91782e9519aefcda4c'
md=Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_LITTLE_ENDIAN);md.detail=True
labels={0:'handshake with fixed eight-byte response payload',1:'identity query with selector',2:'capabilities/status query with selector',0x1000:'image upload setup',0x1001:'image upload block',0x1002:'image upload finalize',0x1003:'image upload cancel',0x1080:'print initiation',0x2000:'firmware transfer setup',0x2001:'firmware transfer block',0x2002:'firmware transfer finalize',0x2080:'firmware validation/copy sequence (partial interpretation)',0x2081:'firmware apply sequence (partial interpretation)'}
def loc(a):return {'va':f'0x{a:08x}','file_offset':f'0x{a-BASE:x}'}
records=[]
a=0x0802cb14
while True:
 raw=B[a-BASE:a-BASE+16];cmd,length,*flags,handler,cont=struct.unpack('<HH4BII',raw)
 rec={**loc(a),'raw_hex':raw.hex(),'opcode':f'0x{cmd:04x}','declared_frame_length':length,'raw_flags':flags,'handler_thumb':f'0x{handler:08x}','handler_code':f'0x{handler&~1:08x}' if handler else None,'continuation_thumb':f'0x{cont:08x}','continuation_code':f'0x{cont&~1:08x}' if cont else None,'interpretation':labels.get(cmd,'unassigned semantic label'),'confidence':'high: table structure and indirect dispatch; semantic label varies'}
 if cmd==0xffff:sentinel=rec;break
 assert BASE<=handler&~1<BASE+len(B) and handler&1
 records.append(rec);a+=16
assert len(records)==30
assert records[9]['raw_hex']==records[10]['raw_hex']
ranges={
 'set_up_task_calls_protocol_init':(0x8052198,0x20),'protocol_init':(0x8053754,0x174),
 'parser_magic_length_opcode_table':(0x8053aa4,0xa0),'parser_descriptor_assignment_checksum':(0x8053c34,0x8a),
 'dispatch_primary_and_continuation':(0x8053fae,0x5c),'response_header':(0x80544ae,0x32),'response_finish':(0x80544e8,0x32),'response_checksum':(0x80541cc,0x3e),
 'read_be32':(0x8058a00,0x66),'write_be32':(0x8058b8e,0x5c),'upload_setup':(0x805b848,0x13a),'upload_setup_validation':(0x805b988,0x62),
 'upload_block':(0x805b9ec,0x14e),'upload_finalize':(0x805bb44,0x12a),'upload_cancel':(0x807027e,0x8a),
 'print_initiate':(0x805bf2c,0x1c6),'print_continue':(0x805c0f2,0x98),'print_submit':(0x8049dd8,0x40),'print_event':(0x804e95c,0xc),
 'firmware_setup':(0x805c408,0x18a),'firmware_block':(0x805c592,0x162),'firmware_finalize':(0x805c6f4,0xd0),
 'protocol_event_task':(0x80592f8,0x46),'protocol_reschedule':(0x8059340,0x3c)
}
evidence={}
for name,(start,size) in ranges.items():
 insns=[]
 for i in md.disasm(B[start-BASE:start-BASE+size],start):
  x={**loc(i.address),'bytes':i.bytes.hex(),'instruction':i.mnemonic+' '+i.op_str}
  for op in i.operands:
   if op.type==ARM_OP_MEM and op.mem.base==ARM_REG_PC:
    p=((i.address+4)&~3)+op.mem.disp
    if BASE<=p<=BASE+len(B)-4:x['literal']={**loc(p),'value':f'0x{struct.unpack_from("<I",B,p-BASE)[0]:08x}'}
  insns.append(x)
 evidence[name]={'start':loc(start),'byte_length':size,'instructions':insns}
out={'input_sha256':hashlib.sha256(B).hexdigest(),'base':f'0x{BASE:08x}','method':'Read-only ARM Thumb static disassembly; literal cross references and actual parser-to-table-to-BLX data flow. No firmware execution, emulation, radio contact, or flashing.','table_layout':{'record_size':16,'opcode_u16le':0,'frame_length_u16le':2,'length_mode_u8':4,'flags_unassigned_u8':[5,6,7],'primary_handler_thumb_u32le':8,'continuation_thumb_u32le':12},'descriptor_count':len(records),'unique_opcode_count':len({r['opcode'] for r in records}),'descriptors':records,'sentinel':sentinel,'known_duplicate_records':[[9,10],[19,20]],'important_addresses':{'state':'0x2001031c','callback_vtable':'0x200104bc','parser':'0x08053984','dispatcher':'0x08053f18','protocol_task':'0x080592f8','protocol_task_pointer':loc(0x0802aa90),'response_builder':'0x0805428c'},'wire_format':{'request':'41 62 | total_length_BE16 | opcode_BE16 | payload | checksum_u8','response':'61 42 | total_length_BE16 | opcode_BE16 | result_u8 | payload | checksum_u8','checksum':'sum(all_frame_bytes) modulo 256 == 255','length_includes':'entire frame including checksum','parser_length_upper_exclusive':20800},'image_upload':{'max_total_bytes':153600,'block_bytes':900,'setup_frame_bytes':15,'block_frame_bytes':911,'setup_payload':'type_u8(2), flags_u8, reserved_u8, reserved_u8, size_BE32','setup_reply_payload':'900_BE32','block_payload':'sequence_BE32, block_data[900]','block_reply_payload':'sequence_BE32','staging_address':'0xc00e0000','final_address':'0xc06a0004','observed_completion_markers':'first sequence 0 seen; final sequence seen after first. No complete missing-block bitmap established.'},'firmware_transfer':{'setup_opcode':'0x2000','data_opcode':'0x2001','finalize_opcode':'0x2002','setup_payload':'total_size_BE32, target_address_BE32','block_bytes':1024,'max_total_bytes':1048576,'setup_literal_address_check':'target>=0x08000000; uint32(target+size-1)<0x08100000; target%256==0; size!=0; size<=0x100000','bounds_note':'Literal unsigned arithmetic checks recorded; not a proof against arithmetic wraparound or a complete updater security audit.'},'evidence':evidence}
(out_dir()/'protocol.json').write_text(json.dumps(out,indent=2)+'\n')
print(f'Wrote {len(records)} records / {out["unique_opcode_count"]} unique opcodes with {sum(len(x["instructions"]) for x in evidence.values())} evidence instructions')
