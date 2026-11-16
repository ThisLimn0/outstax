#!/usr/bin/env python3
"""Exercise the built CLI against its simulator for Mini, Square and Wide."""
import argparse
import hashlib
import json
import queue
import re
import struct
import subprocess
import threading
import zlib
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=Path)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
binary = str(args.binary.resolve())
args.output.mkdir(parents=True, exist_ok=False)

def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))

source = args.output / 'test.png'
w, h = 240, 180
pixels = b''.join(b'\0' + bytes(v for x in range(w) for v in (x * 255 // (w-1), y * 255 // (h-1), 255 if x < 30 and y < 30 else 40)) for y in range(h))
source.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(pixels)) + chunk(b'IEND', b''))

def run(*arguments):
    return subprocess.run([binary, *map(str, arguments)], check=True, capture_output=True, text=True, timeout=90).stdout

results = []
for format_name in ['mini', 'square', 'wide']:
    saved = (args.output / (format_name + '-received.jpg')).resolve()
    process = subprocess.Popen([binary, 'simulate', '--listen', '127.0.0.1:0', '--format', format_name, '--save', str(saved)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        lines = queue.Queue()
        threading.Thread(target=lambda: lines.put(process.stdout.readline()), daemon=True).start()
        line = lines.get(timeout=20)
        match = re.search(r'127\.0\.0\.1:\d+', line)
        if not match:
            raise RuntimeError(f'Simulator failed to start: {line}')
        address = match.group(0)
        status = run('--tcp', address, 'status')
        upload_trace = args.output / (format_name + '-upload.jsonl')
        upload = run('--tcp', address, '--trace', upload_trace, 'print', source)
        if saved.exists() or 'Uploaded' not in upload:
            raise AssertionError('Upload-only operation behaved like a print')
        decoded = run('trace', upload_trace)
        frames = [json.loads(row) for row in decoded.splitlines()]
        commands = [row['command'] for row in frames if row['direction'] == 'Request']
        if '1080' in commands or commands[-1] != '1003':
            raise AssertionError('Unexpected upload-only command sequence')
        printed = run('--tcp', address, 'print', source, '--commit')
        if not saved.exists() or 'PrintAccepted' not in printed:
            raise AssertionError('Simulator did not receive a physical-print request')
        results.append({'format': format_name, 'status': status.strip(), 'upload_only': 'passed',
                        'trace_reassembly': 'passed', 'committed_simulated_print': 'passed',
                        'received_jpeg_bytes': saved.stat().st_size,
                        'received_sha256': hashlib.sha256(saved.read_bytes()).hexdigest()})
        print(format_name, 'passed', flush=True)
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
(args.output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
print('Three format workflows passed; these are simulator results, not hardware qualification.')

