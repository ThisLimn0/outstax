#!/usr/bin/env python3
"""Compare the two official WIDE Evo update source-version variants."""
import hashlib,json,sys
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi, multi_out
base=multi('research/contents/wide-evo.DAT').read_bytes();second=multi('research/multi/inputs/wide-evo/FPUPDATE-Ver101.DAT').read_bytes();assert second.startswith(base);t=second[len(base):]
j=dict(first_size=len(base),second_size=len(second),second_sha256=hashlib.sha256(second).hexdigest(),prefix_identical=True,trailer_size=len(t),trailer_sha256=hashlib.sha256(t).hexdigest(),first256_distinct_values=len(set(t[:256])),descending_bytes_at_256=t[256:288].hex(),descending_32_to_1=t[256:288]==bytes(range(32,0,-1)),ascii_version_offset=t.find(b'0.001.000'),zero_bytes=t.count(0),nonzero_bytes=len(t)-t.count(0));multi_out('research/multi/evo-derived/variant-comparison.json').write_text(json.dumps(j,indent=2)+'\n');print(json.dumps(j,indent=2))
