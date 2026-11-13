"""Retrieve analysis inputs without installing or executing vendor packages."""
import argparse, concurrent.futures, hashlib, io, json, pathlib, urllib.request, zipfile
from datetime import datetime, timezone

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=pathlib.Path, required=True)
parser.add_argument('--assets', action='store_true', help='Also retrieve SharedAssets splits, including bundled firmware')
parser.add_argument('--native', action='store_true', help='Also retrieve ARM64 native splits')
args = parser.parse_args()
ROOT = args.output
ROOT.mkdir(parents=True, exist_ok=False)

class RemoteZip(io.RawIOBase):
    def __init__(self, url):
        self.url = url
        self.pos = 0
        with urllib.request.urlopen(urllib.request.Request(url, method='HEAD', headers={'User-Agent': 'Mozilla/5.0'}), timeout=40) as r:
            self.size = int(r.headers['Content-Length'])
        self.cache = {}
    def readable(self): return True
    def seekable(self): return True
    def tell(self): return self.pos
    def seek(self, offset, whence=0):
        self.pos = offset if whence == 0 else (self.pos if whence == 1 else self.size) + offset
        return self.pos
    def read(self, n=-1):
        n = self.size-self.pos if n < 0 else min(n, self.size-self.pos)
        if n <= 0: return b''
        start = self.pos
        end = start+n-1
        req = urllib.request.Request(self.url, headers={'User-Agent':'Mozilla/5.0', 'Range':f'bytes={start}-{end}', 'Accept-Encoding':'identity'})
        with urllib.request.urlopen(req, timeout=120) as r:
            if r.status != 206: raise RuntimeError(f'Range request refused: {r.status}')
            data = r.read(n)
        self.pos += len(data)
        return data

def get_app(package):
    url = 'https://d.apkpure.com/b/XAPK/' + package + '?version=latest'
    folder = ROOT / package
    folder.mkdir(exist_ok=True)
    row = {'package':package, 'source':url, 'acquired_utc':datetime.now(timezone.utc).isoformat(), 'source_trust':'third-party mirror; compare signer with Google Play before trust', 'files':[]}
    try:
        with zipfile.ZipFile(RemoteZip(url)) as z:
            infos = z.infolist()
            row['entries'] = [{'name':i.filename,'size':i.file_size,'compressed':i.compress_size} for i in infos]
            (folder/'acquisition.json').write_text(json.dumps(row,indent=2))
            print(package, 'reading archive metadata', flush=True)
            for i in sorted(infos, key=lambda i: i.filename != 'manifest.json'):
                if i.filename == 'manifest.json' or (i.filename.endswith('.apk') and (i.filename == package + '.apk' or (args.native and 'arm64' in i.filename) or (args.assets and 'SharedAssets' in i.filename))):
                    target = folder / pathlib.Path(i.filename).name
                    target.write_bytes(z.read(i))
                    item = {'name':target.name,'size':target.stat().st_size,'sha256':hashlib.sha256(target.read_bytes()).hexdigest()}
                    row['files'].append(item)
                    if i.filename == 'manifest.json': row['manifest']=json.loads(target.read_text())
                    print(package,item,flush=True)
        row['status']='retrieved'
    except Exception as e: row['status']='failed'; row['error']=repr(e)
    (folder/'acquisition.json').write_text(json.dumps(row,indent=2))
    print(package,row['status'],row.get('error',''),flush=True)
    return row

with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    rows=list(pool.map(get_app,['com.fujifilm.instaxminiLink','com.fujifilm.instaxSquareLink','com.fujifilm.instaxLinkWide']))
(ROOT/'apps.json').write_text(json.dumps(rows,indent=2))
