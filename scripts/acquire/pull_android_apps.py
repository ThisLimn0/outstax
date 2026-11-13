#!/usr/bin/env python3
"""Pull current installed base/split APKs from one already-authorized ADB phone."""
import argparse
import hashlib
import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

PACKAGES = ['com.fujifilm.instaxminiLink', 'com.fujifilm.instaxSquareLink', 'com.fujifilm.instaxLinkWide']
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--serial')
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
adb = ['adb']
if args.serial:
    adb += ['-s', args.serial]
else:
    lines = subprocess.check_output(['adb', 'devices'], text=True).splitlines()[1:]
    devices = [line.split()[0] for line in lines if len(line.split()) == 2 and line.split()[1] == 'device']
    if len(devices) != 1:
        raise SystemExit('Expected one authorized device; select --serial explicitly.')
    adb += ['-s', devices[0]]
args.output.mkdir(parents=True, exist_ok=False)
manifest = {'acquired_utc': datetime.now(timezone.utc).isoformat(), 'source': 'installed Android packages', 'packages': []}
for package in PACKAGES:
    paths = subprocess.check_output(adb + ['shell', 'pm', 'path', package], text=True).splitlines()
    paths = [line[len('package:'):].strip() for line in paths if line.startswith('package:')]
    if not paths:
        manifest['packages'].append({'package': package, 'status': 'not installed'})
        continue
    folder = args.output / package
    folder.mkdir()
    metadata = subprocess.check_output(adb + ['shell', 'dumpsys', 'package', package], text=True)
    version_name = re.search(r'versionName=(\S+)', metadata)
    version_code = re.search(r'versionCode=(\d+)', metadata)
    row = {'package': package, 'version': version_name.group(1) if version_name else None,
           'version_code': int(version_code.group(1)) if version_code else None, 'files': []}
    for remote in paths:
        local = folder / Path(remote).name
        if local.exists():
            raise SystemExit(f'Refusing to overwrite {local}')
        subprocess.run(adb + ['pull', remote, str(local)], check=True)
        row['files'].append({'name': local.name, 'bytes': local.stat().st_size,
                             'sha256': hashlib.sha256(local.read_bytes()).hexdigest()})
    manifest['packages'].append(row)
    (args.output / 'acquisition.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(args.output / 'acquisition.json')

