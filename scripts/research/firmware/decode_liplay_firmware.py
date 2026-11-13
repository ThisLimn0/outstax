#!/usr/bin/env python3
"""Decode a raw LZ4 block to a NEW file; does not flash or send device commands."""
import argparse,hashlib
from pathlib import Path
import lz4.block
p=argparse.ArgumentParser(description=__doc__);p.add_argument('input',type=Path);p.add_argument('-o','--output',type=Path,required=True);p.add_argument('--max-output-bytes',type=int,default=256*1024*1024);p.add_argument('--expected-sha256');a=p.parse_args()
if a.output.exists():p.error('Output already exists')
if not 1<=a.max_output_bytes<=512*1024*1024:p.error('Output limit must be1..512MiB')
data=lz4.block.decompress(a.input.read_bytes(),uncompressed_size=a.max_output_bytes);sha=hashlib.sha256(data).hexdigest()
if a.expected_sha256 and sha.lower()!=a.expected_sha256.lower():raise SystemExit('Decoded hash mismatch; no output written')
with a.output.open('xb')as f:f.write(data)
print(len(data),'bytes SHA256',sha)
