#!/usr/bin/env python3
"""Combine independently decoded command schemas into a single device profile."""
from pathlib import Path
import json, sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).parent))
from _paths import out_dir, report
P=out_dir()
protocol=json.loads(report('protocol.json').read_text())
base={int(r['opcode'],0):r for r in protocol['descriptors']}
schemas={
 0x0000:('handshake',7,'none','fixed8bytes0100000000000000','protocol.md'),
 0x0001:('identity query',8,'selector:u8(0..8)','selector:u8,count:u8,data[count]','status.md'),
 0x0002:('capabilities and status',8,'selector:u8(0..3)','selector-dependent11/5/9/9bytes includingselector','status.md'),
 0x1000:('image upload setup',15,'format:u8=2,flags:u8,reserved[2],size:BE32','block_size:BE32=900','protocol.md'),
 0x1001:('image upload block',911,'sequence:BE32,data[900]','sequence:BE32','protocol.md'),
 0x1002:('image upload finalize',7,'none','none','protocol.md'),
 0x1003:('image upload cancel',7,'none','none','protocol.md'),
 0x1080:('print initiation',7,'none','time_estimate:u8 on initial acceptance','protocol.md'),
}
for source in ['settings','update']:
 for item in json.loads(report(source+'.json').read_text())['commands']:
  opcode=int(item['opcode'],0)
  response=item.get('response_payload',item.get('reply_payload',item.get('initial_reply_payload','see detailed report')))
  schemas[opcode]=(item.get('label',base[opcode]['interpretation']),item['request_frame_bytes'],item['payload'],response,source+'.md')
if set(base)!=set(schemas):raise ValueError('incomplete or extraneous opcode mapping')
labels={0x2010:'update metadata word query',0x2080:'external NOR stage and readback verify',
 0x2081:'reply-completed prior-stage handoff',0x3000:'three-word sensor data and read-clear flag',
 0x3001:'three-channel PWM pattern',0x3002:'two-bit runtime mask access',
 0x3080:'hardware and raw parameter query',0x3081:'conditional three-word persistent parameters'}
records=[]
for op in sorted(base):
 r=base[op];label,request,payload,response,source=schemas[op]
 if isinstance(request,int) and request!=r['declared_frame_length']:raise ValueError('request length mismatch')
 records.append({'opcode':f'0x{op:04x}','label':labels.get(op,label),'request_frame_bytes':request,
 'request_payload':payload,'successful_response_payload':response,'handler_code':r['handler_code'],
 'continuation_code':r['continuation_code'],'source_report':source,
 'confidence':'frame shape and handler association checked; physical labels, timing units and hardware behavior may remain unresolved'})
obj={'firmware_sha256':protocol['input_sha256'],'model':'original instax mini Link / SP-4 / BOD0104',
 'command_count':len(records),'commands':records,
 'scope':'All28unique command request shapes assigned. This is not a complete firmware/source/hardware reconstruction.',
 'client_constraints':{'image_upload_bytes':153600,'image_block_bytes':900,'firmware_block_bytes':1024,
 'request_length_upper_exclusive':20800,'async_completion':'Internal12/13 are not wire replies; poll status without blindly reissuing physical commands.',
 'transport':'No Bluetooth backend is provided by the protocol crate; native connections remain separate work.'}}
(P/'command-registry.json').write_text(json.dumps(obj,indent=2)+'\n')
lines=['# Mini Link 0104 command reference','',obj['scope'],'',
 '| Opcode | Behavior | Request frame bytes | Request payload | Detailed evidence |',
 '|---|---|---:|---|---|']
for r in records:lines.append(f'| `{r["opcode"]}` | {r["label"]} | {r["request_frame_bytes"]} | `{r["request_payload"]}` | [{r["source_report"]}]({r["source_report"]}) |')
lines+=['','Response schemas and addresses are in `command-registry.json`; detailed branches, side effects and limitations are in the linked reports.',
 '','The Rust core implements framing, identity/status parsing, image upload planning and bounded readback collection. It does not execute the update, reboot, ejection or persistent-setting paths.']
(P/'client-reference.md').write_text('\n'.join(lines)+'\n')
print(f'Combined {len(records)} unique opcode schemas, all fixed request lengths match the original descriptor table.')
