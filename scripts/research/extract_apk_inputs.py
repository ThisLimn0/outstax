#!/usr/bin/env python3
"""Extract static RE inputs from locally acquired APKs, without executing them."""
import argparse,hashlib,json,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('apk_root',type=Path)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False);items=[]
for apk in a.apk_root.rglob('*.apk'):
    with zipfile.ZipFile(apk) as z:
        for i in z.infolist():
            name=i.filename
            selected=(name.endswith('.dex') or name.startswith(('assets/fw/','assets/Firmware/')) or name in ['assets/20210701_3DLUT.bin','assets/data.zip'] or (name.startswith('lib/arm64-v8a/') and any(x in name.lower() for x in ['fdia','ffaipii','sharpness'])))
            if not selected:continue
            if '..' in Path(name).parts or name.startswith('/'):raise ValueError('unsafe APK entry')
            data=z.read(i);dest=a.output/apk.parent.name/apk.stem/name
            dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
            items.append({'apk':str(apk),'entry':name,'output':str(dest),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
(a.output/'inputs.json').write_text(json.dumps(items,indent=2)+'\n')
print(len(items),'inputs extracted')
