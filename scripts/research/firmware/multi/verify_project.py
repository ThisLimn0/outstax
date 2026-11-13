#!/usr/bin/env python3
"""Cross-check retained evidence against original bytes; never run vendor code."""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

sys.dont_write_bytecode = True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi_generated, multi_out
HERE = Path(__file__).resolve().parent
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding

def digest(data):
    return hashlib.sha256(data).hexdigest()

def load(path):
    return json.loads(multi_generated(path).read_text())

results = {'passed': False, 'checks': {}}
for script in ['input_manifest.py', 'renesas_check_evidence.py', 'evo_validate.py']:
    proc = subprocess.run([sys.executable, str(HERE / script)],
                          check=True, capture_output=True, text=True)
    results['checks'][script] = {'passed': True, 'output': proc.stdout.strip()}

keys = []
for model in ['square', 'wide2']:
    evidence = load(f'report/multi/{model}-evidence.json')
    base = multi_generated(f'research/multi/sunp-derived/{model}/decoded.bin').parent
    data = (base / 'decoded.bin').read_bytes()
    assert digest(data) == evidence['decoded_sha256']
    envelope = evidence['signature']
    offset, size = envelope['offset'], envelope['bytes']
    message = data[:offset] + data[offset + size:-4]
    assert digest(message) == envelope['signed_sha256']
    key = serialization.load_pem_public_key((base / 'files/A/RO_RES/BPUB.PEM').read_bytes())
    # Independent OpenSSL-backed verification of the extractor's DER/RSA result.
    key.verify(data[offset:offset + size], message, padding.PKCS1v15(), hashes.SHA256())
    keys.append(digest(key.public_bytes(serialization.Encoding.DER,
                                       serialization.PublicFormat.SubjectPublicKeyInfo)))
    assert len(data) % 4 == 0
    assert sum(struct.unpack('<' + 'I' * (len(data) // 4), data)) & 0xffffffff == 0
    for item in evidence['files']:
        path = base / 'files' / item['path']
        assert not path.is_symlink()
        raw = path.read_bytes()
        assert len(raw) == item['bytes'] and digest(raw) == item['sha256'], str(path)
    for item in evidence['gamma_tables']:
        raw = (base / 'files' / item['path']).read_bytes()
        assert len(raw) == 514
        words = struct.unpack('<257H', raw)
        assert list(words[:256]) == item['values']
        assert sum(words) & 0xffff == 0xffff
    windows = load(f'report/multi/{model}-code-windows.json')
    regions = load(f'report/multi/{model}-protocol-evidence.json')['instruction_regions']
    instructions = windows + [item for region in regions.values() for item in region]
    for item in instructions:
        offset = int(item['file_offset'], 0)
        raw = bytes.fromhex(item['bytes'])
        assert data[offset:offset + len(raw)] == raw, (model, item)
    results['checks'][model] = {'passed': True, 'rsa_signature_independently_verified': True,
        'public_key_spki_sha256': keys[-1], 'file_hashes': len(evidence['files']),
        'gamma_checksums': len(evidence['gamma_tables']),
        'instruction_records_compared_to_binary': len(instructions)}
assert keys[0] != keys[1]
results['checks']['distinct_sunp_signing_keys'] = True
for path in HERE.glob('*.py'):
    compile(path.read_bytes(), str(path), 'exec')
qa = load('report/multi/pdf-qa.json')
assert qa['pages'] == 37 and qa['visual_validation'] == 'passed'
assert digest(multi_generated(qa['output']).read_bytes()) == qa['pdf_sha256']
results['checks']['pdf'] = {'pages': 37, 'sha256': qa['pdf_sha256'], 'visual_review': 'passed'}
results['passed'] = True
multi_out('report/multi/project-verification.json').write_text(json.dumps(results, indent=2) + '\n')
print(json.dumps(results, indent=2))
