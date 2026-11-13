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
import dnfile
from dncil.cil.body.reader import read_method_body_from_bytes
from dncil.clr.token import Token,StringToken
p=Path(str(RESEARCH_ROOT)+'/drivers/Fujifilm.Instax.LinkWIDE2.PrinterDriver.Setup.x64/filAF61972469244DEDBC294BA28F413DFF');pe=dnfile.dnPE(str(p));tables=pe.net.mdtables;names={}
for t in tables.TypeDef:
 for m in t.MethodList:names[id(m.row)]=str(t.TypeName)+'.'+str(m.row.Name)
 for f in t.FieldList:names[id(f.row)]=str(t.TypeName)+'.'+str(f.row.Name)
def resolve(op):
 if isinstance(op,StringToken):return repr(pe.net.user_strings.get(op.rid).value)
 if isinstance(op,Token):
  tab=tables.tables.get(op.table)
  if tab and 0<op.rid<=len(tab.rows):
   row=tab.rows[op.rid-1]
   if id(row) in names:return names[id(row)]
   if hasattr(row,'Name'):
    parent=getattr(row,'Class',None);prefix=str(getattr(parent.row,'TypeName','')) if parent else ''
    return prefix+'.'+str(row.Name)
   return str(row)
 return str(op) if op is not None else ''
lines=[]
for t in tables.TypeDef:
 if str(t.TypeName) not in ['PrintTask','BleRequest','BleResponse','BleConstants','BleDataHandler','BleDeviceDiscoveryService','GetFunctionVersionInfoTask']:continue
 for m in t.MethodList:
  row=m.row
  if not row.Rva:continue
  lines.append('\n'+str(t.TypeName)+'.'+str(row.Name))
  try:
   body=read_method_body_from_bytes(pe.get_data(row.Rva))
   for i in body.instructions:lines.append(f'{i.offset:04x} {str(i.opcode):18} {resolve(i.operand)}')
  except Exception as e:lines.append(str(e))
Path(str(RESEARCH_ROOT)+'/re-pass/wide2-driver.il').write_text('\n'.join(lines)+'\n')
print(len(lines),'IL instructions saved')
