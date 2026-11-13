# Run from any directory; vendor inputs belong under --research-root (default: ../research beside the project).
import argparse
from pathlib import Path
_cli=argparse.ArgumentParser()
_cli.add_argument('--project-root',type=Path,default=Path(__file__).resolve().parents[2])
_cli.add_argument('--research-root',type=Path)
_args=_cli.parse_args()
PROJECT_ROOT=_args.project_root.resolve()
RESEARCH_ROOT=(_args.research_root or PROJECT_ROOT.parent/'research').resolve()  # research workspace beside the project
from pathlib import Path
import json,re,hashlib
root=PROJECT_ROOT
commands=json.loads((root/'data/protocol/commands.json').read_text())
# Layout notation is descriptive, not an executable serializer. Result byte is excluded.
layouts={
'0000':('empty','functions:u8, versions:u8[7]','initialization'),
'0001':('selector:u8','selector:u8, length:u8, text:bytes[length]','initialization'),
'0002':('selector:u8','selector:u8, selector-specific fields; see PROTOCOL.md','status'),
'0010':('selector:u8','selector:u8, length:u8, text:bytes[length]','identity'),
'0100':('type:u8; firmware abort sends 0','no fields consumed','power'),
'0102':('mode:u8, reserved:zero[3], timers:u16be[4]','timers:u16be[4]','settings'),
'0103':('min_interval:u16be=12,max_interval:u16be=12,latency:u16be=0,timeout:u16be=400','no fields consumed','BLE upload setup'),
'1000':('picture_type:u8=2,option1:u8,option2:u8,reserved:u8=0,image_length:u32be','block_size:u32be','image upload'),
'1001':('sequence:u32be, data:bytes[block_size]; final block zero-padded','sequence:u32be','image upload'),
'1002':('empty','no fields consumed','image upload'),
'1003':('empty','no fields consumed','image upload'),
'1080':('empty','estimated_seconds:u8','physical print'),
'1081':('empty','estimated_seconds:u8; same PrintTask completion path','cover ejection'),
'2000':('file_length:u32be, start_address:u32be','block_size:u32be','main firmware'),
'2001':('sequence:u32be,data:bytes[block_size]; final block zero-padded','sequence:u32be','main firmware'),
'2002':('empty','no fields consumed','main firmware'),
'2010':('empty','program:u16be','initialization / main firmware'),
'2080':('empty','estimated_seconds:u8','main firmware backup'),
'2081':('empty','program:u16be,estimated_seconds:u8','main firmware activation'),
'20a0':('component:u16be,file_length:u32be,checksum:u32be,reserved:u16be=0','block_size:u32be','Link 3 Bluetooth firmware'),
'20a1':('sequence:u32be,data:bytes[block_size]; final block zero-padded','sequence:u32be','Link 3 Bluetooth firmware'),
'20a2':('empty','no fields consumed','Link 3 Bluetooth firmware'),
'20a7':('empty','component_completion_mask:u32be; app requires 0x3ff','Link 3 Bluetooth firmware'),
'20a8':('empty','program:u16be','Link 3 initialization'),
'20b0':('empty','estimated_seconds:u8','Link 3 Bluetooth firmware backup'),
'20b1':('empty','program:u16be,estimated_seconds:u8','Link 3 Bluetooth firmware activation'),
'3000':('empty','x:i16be,y:i16be,z:i16be,switches:u8','axes / remote shutter'),
'3001':('mode:u8,count:u8,step_time_10ms:u8,repeat:u8,BGR:u8[count][3]','no fields consumed','LED'),
'3002':('constant:u8=2,flags:u8; bit0 power-off, bit1 reprint','no fields consumed','axis actions'),
'3003':('mode:u8,count:u8,step_time_10ms:u8,repeat:u8,steps:BGR[count][channels][3]','no fields consumed','model-specific LED; channels 1/2/3'),
'3004':('mode:u8=1(read)/5(write),reserved:zero[3],value:u8,reserved:zero[11]','current_pattern:u8; app requires total response >=20 bytes, ignores remaining bytes','power LED'),
'3006':('enabled:u8,intensity:u8,reserved:zero[2]','switches:u8','AR vibration / switch'),
'3010':('selector:u8','0:voltage:i16be,temperature:i16be; 1:attempts:u32be,battery_type:u8,color:u8,film_PI:u8[,cpu_type:u8 in Mini]','additional information'),
'3020':('parameter:u8,mode:u8,data:u8[8]','write task consumes no fields; read-mode builder exists but no read task/parser found','Link+ parameter write'),
'3080':('empty','head:u8,judge:u8,reserved:u8,year:u16be,month:u8,day:u8,RGB:u16be[3]','original mini head calibration'),
'3081':('RGB:u16be[3]','no fields consumed','original mini head calibration'),
'8000':('flash:u8,timer:u8,reserved:zero[2]','no fields consumed','hybrid camera'),
'8001':('selector:u8','selector:u8, experienced:u8 OR favorites:u8[6]','hybrid camera'),
'8100':('empty','flags:u8,file_name:UTF8[20],database_index:u32be','hybrid camera media'),
'8101':('database_index:u32be','picture_type:u8,byte_count:u32be,block_size:u32be','hybrid camera media'),
'8102':('sequence:u32be','sequence:u32be,data:remaining_bytes','hybrid camera media'),
'8103':('empty','no fields consumed','hybrid camera media'),
'8104':('database_index:u32be','audio_type:u8,byte_count:u32be,block_size:u32be','hybrid camera media'),
'8105':('sequence:u32be','sequence:u32be,data:remaining_bytes','hybrid camera media'),
'8106':('empty','no fields consumed','hybrid camera media'),
'8107':('database_index:u32be','length:u8,url:UTF8[length]','hybrid camera media'),
'8108':('database_index:u32be,flags:u8; bits0 picture,1 audio,2 address,3 print_image','no fields consumed','hybrid camera media'),
'8200':('zero:u8','no fields consumed','hybrid camera live view'),
'8201':('empty','transfer_type:u8,frame_type:u8,length:u16be,image:bytes[length]','hybrid camera live view'),
'8202':('zero:u8','no fields consumed','hybrid camera live view'),
'8210':('zero:u8','no fields consumed','hybrid camera capture'),
'8220':('empty','picture_type:u8,byte_count:u32be,block_size:u32be','hybrid camera post view'),
'8221':('sequence:u32be','sequence:u32be,data:remaining_bytes','hybrid camera post view'),
'8222':('empty','no fields consumed','hybrid camera post view'),
'8230':('empty','estimated_seconds:u8','hybrid camera print'),
'8300':('picture_type:u8,frame_number:u8,frame_type:u8,reserved:u8=0,image_length:u32be','block_size:u32be','hybrid camera frames'),
'8301':('sequence:u32be,data:bytes[block_size]','sequence:u32be','hybrid camera frames'),
'8302':('empty','no fields consumed','hybrid camera frames'),
'8303':('frame_number:u8,app_string_char_count:u8,name:UTF8_bytes; app may miscount non-ASCII','no fields consumed','hybrid camera frames'),
'8400':('empty','byte_count:u32be,block_size:u32be,profile_info:u32be','hybrid camera logs'),
'8401':('sequence:u32be','sequence:u32be,data:remaining_bytes','hybrid camera logs'),
'8402':('empty','no fields consumed','hybrid camera logs'),
'8403':('log_type:u8','log_type:u8,byte_count:u32be,block_size:u32be,count:u32be','hybrid camera logs'),
'8404':('log_type:u8,sequence:u32be','log_type:u8,sequence:u32be,data:remaining_bytes','hybrid camera logs'),
'8405':('log_type:u8','log_type:u8','hybrid camera logs'),
'8406':('log_type:u8','log_type:u8,byte_count:u32be,block_size:u32be,count:u32be','hybrid camera logs'),
'8407':('log_type:u8,sequence:u32be','log_type:u8,sequence:u32be,data:remaining_bytes','hybrid camera logs'),
'8408':('log_type:u8','log_type:u8','hybrid camera logs'),
}
# Reference lookup records source locations/hashes without redistributing decompiled implementation.
evidence={}
for app,roots in [('wide',['com/fujifilm/bluetooth']),('mini',['O1','P1','Q1','R1','S1','T1','U1','V1','W1','com/fujifilm/bluetooth']),('square',['L1','Q1','R1','S1','T1','U1','V1','W1','X1'])]:
 for sub in roots:
  for p in Path(str(RESEARCH_ROOT)+'/decompiled',app,'sources',sub).rglob('*.java'):
   key=app+':'+str(p).split('/sources/')[1]
   evidence[key]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
for c in commands:
 op=c['opcode'];name=c['name']; refs=[]
 for key in evidence:
  if key.startswith('square:'):continue # R8 removed command symbol names; mapped separately below.
  p=Path(str(RESEARCH_ROOT)+'/decompiled',key.split(':')[0],'sources',key.split(':',1)[1]);s=p.read_text()
  if name in s and not '/util/SID.java' in key and not key.endswith('W1/D.java'):
   refs.append(key)
 c['references']=refs
 if op in layouts:
  req,res,scope=layouts[op];c['request']=req;c['success_body']=res;c['scope']=scope;c['re_status']='wire layout recovered'
 else:
  c['request']=None;c['success_body']=None;c['scope']='library enum';c['re_status']='enum only; no emitting request builder/call site identified; do not guess payload'
 if op in ('3008','0101','2003','20a6','20a9','8304'):c['re_status']='enum only; no emitting request builder/call site identified; do not guess payload'
 if op=='3020':c['re_status']='write path recovered; read response undetermined'
 if op in ('0000','3000','3010'):c['implementation']='typed parser implemented; no hardware qualification'
 if op in ('0102','3001','3002','3003','3004','3006','3020','3080','3081'):c['implementation']='typed builder/parser implemented as applicable; no complete application UX'
 if op.startswith('20') and op in layouts:c['implementation']='packet builder or layout documented; offline firmware inspection; no updater UI or recovery qualification'
 c['hardware_verified']=False
(root/'data/protocol/commands.json').write_text(json.dumps(commands,indent=2)+'\n')
(root/'data/apps/source-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
print(len(commands),'commands;',len(layouts),'layouts;',len(evidence),'source hashes')
