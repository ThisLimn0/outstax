#!/usr/bin/env python3
"""Assemble the reviewed chapters and input/registry appendices."""
from pathlib import Path
import json,sys
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi_generated, multi_out
P='report/multi/'
parts=['# INSTAX firmware architecture and protocol analysis\n\n'
       'Link WIDE 2 | SQUARE Link | mini Link 2 / 3 Renesas RX | WIDE Evo\n\n'
       'Technical report and reproducible evidence project - 12 November 2026\n\n']
for name in ['overview.md','square.md','wide2.md','renesas.md','evo-analysis.md','conclusions.md']:
 parts.append(multi_generated(P+name).read_text(encoding='utf-8'))
parts.append('# Appendix A: Complete Renesas descriptor registries\n\n'
 'These tables are read from the original images. An opcode entry proves its '
 'handler association and declared frame size, not full semantics of every command. '
 'Duplicated descriptors are retained in the tables; distinct-opcode counts are separate.\n\n')
for name in ['renesas-mini2-registry.md','renesas-mini3-registry.md']:parts.append(multi_generated(P+name).read_text(encoding='utf-8'))
parts.append('# Appendix B: Exact input identities\n\n'
 'SHA-256 identifies each acquired input. The companion manifest includes '
 'source endpoints, app versions, binary reconstructions and provenance paths. '
 'The last mini Link1/mini Evo entries are comparison/reference inputs, not '
 'additional targets of the detailed analysis.\n\n')
manifest=json.loads(multi_generated('research/multi/inputs/manifest.json').read_text())
for item in manifest['inputs']:
 path=item['path'];label=item.get('asset',Path(path).name)
 parts.append('## '+label+'\n\n'
  '- Project path: `'+path+'`\n'
  '- Input bytes: '+str(item['bytes'])+'\n'
  '- SHA-256: `'+item['sha256']+'`\n')
 if 'binary_path'in item:
  parts.append('- Reconstructed binary: `'+item['binary_path']+'`\n'
   '- Binary bytes: '+str(item['binary_bytes'])+'\n'
   '- Binary SHA-256: `'+item['binary_sha256']+'`\n')
 parts.append('\n')
parts.append('# Appendix C: Validation and reproduction index\n\n'
 '| Area | Reproducer | Retained results |\n|---|---|---|\n'
 '| Acquired byte identities | `input_manifest.py` | `research/multi/inputs/manifest.json` |\n'
 '| DEX model/CPU routing | `dex_routing.py` | `dex-routing.json` |\n'
 '| SUNP / FAT / signature / resource decode | `sunp_extract.py` | `square-evidence.json`, `wide2-evidence.json` |\n'
 '| MIPS code and exact packet fields | `sunp_code.py`, `sunp_protocol_evidence.py` | `*-code-windows.json`, `*-protocol-evidence.json` |\n'
 '| RX registries and code proofs | `renesas_extract.py`, `renesas_disasm.py`, `renesas_check_evidence.py` | `renesas-*.json`, selected disassembly |\n'
 '| WIDE Evo container, files, code and signature candidates | `evo_*.py` | `evo-summary.json`, `evo-validation.json`, per-file hashes |\n\n'
 'Commands and dependencies are documented in the project README and individual '
 'chapters. The report PDF is a presentation of these sources; the exact bytes '
 'and machine-readable evidence remain the primary reproducibility artifacts.\n')
text='\n\n'.join(parts)
multi_out(P+'REPORT.md').write_text(text,encoding='utf-8')
print('Wrote REPORT.md:',len(text.split()),'words,',len(text.encode()),'bytes')
