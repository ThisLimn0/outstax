#!/usr/bin/env python3
"""Bounded offline signature checks on unmodified official firmware."""
import sys,json,hashlib
from pathlib import Path
sys.dont_write_bytecode=True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import multi, multi_generated, multi_out
from cryptography.hazmat.primitives import serialization,hashes
from cryptography.hazmat.primitives.asymmetric import padding
from cryptography.exceptions import InvalidSignature
R='research/multi/evo-derived/';b=multi('research/contents/wide-evo.DAT').read_bytes();v=multi('research/multi/inputs/wide-evo/FPUPDATE-Ver101.DAT').read_bytes();assert v.startswith(b);t=v[len(b):];signature=t[:256];metadata=t[256:];key=serialization.load_pem_public_key(multi(R+'rootfs-13/854005228/rootfs/etc/network_OTA_bpub.pem').read_bytes());tests=[]
candidates={'base_and_metadata':b+metadata,'base_only':b,'stream_and_metadata':b[128:]+metadata,'stream_only':b[128:],'decoded_and_metadata':multi_generated(R+'decoded.bin').read_bytes()+metadata,'metadata_only':metadata}
for name,data in candidates.items():
 try:key.verify(signature,data,padding.PKCS1v15(),hashes.SHA256());ok=True
 except InvalidSignature:ok=False
 tests.append(dict(candidate=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),valid=ok))
# Inspect only the encoded-message scheme, without treating arbitrary trailing bytes as plaintext.
nums=key.public_numbers();em=pow(int.from_bytes(signature,'big'),nums.e,nums.n).to_bytes(256,'big');is_pkcs1=em.startswith(b'\0\1') and b'\0' in em[2:]
result=dict(public_key='etc/network_OTA_bpub.pem',algorithm='RSA-PKCS1-v1_5-SHA256',signature_location='Ver101 trailer[0:256]',metadata_location='Ver101 trailer[256:1280]',tests=tests,rsa_encoded_message_pkcs1_v1_5=is_pkcs1)
if is_pkcs1:
 tail=em[em.index(0,2)+1:];result['digest_info_hex']=tail.hex();result['digest_info_sha256_oid_matches']=tail[:19].hex()=='3031300d060960864801650304020105000420'
multi_out(R+'signature-verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
