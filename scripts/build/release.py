#!/usr/bin/env python3
"""Build a binary release zip of outstax + outstax-desktop for the current host.

Run from anywhere; the zip is written to the project root as
outstax-<version>-<os>-<arch>.zip. It contains both executables, README.txt,
TROUBLESHOOTING.md, LICENSE, licenses/ and THIRD-PARTY-LICENSES.txt (licence texts
of every crate linked into the binaries, taken from the local cargo sources), plus
SHA256SUMS.txt. Existing zips are never overwritten.

  python scripts/build/release.py                # builds with cargo, then packages
  python scripts/build/release.py --no-build     # package already-built binaries
  python scripts/build/release.py --output-dir ..   # write the zip next to the project folder
  python scripts/build/release.py --target x86_64-unknown-linux-gnu.2.31
      # cross-build for Linux with cargo-zigbuild (zig as linker), glibc 2.31 baseline
  python scripts/build/release.py --target universal-apple-darwin
      # on a Mac: build Apple Silicon and Intel, merge them with lipo, package OutStax.app

Uses CARGO_TARGET_DIR when set (keep build caches outside the project).
"""
import argparse, hashlib, json, os, pathlib, shutil, subprocess, tempfile, zipfile

PROJECT = pathlib.Path(__file__).resolve().parents[2]
LICENCE_PREFIXES = ('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'UNLICENSE', 'OFL', 'UFL')


def cargo(*args):
    return subprocess.run(['cargo', *args], cwd=PROJECT, check=True, capture_output=True, text=True).stdout


def host_triple():
    for line in subprocess.run(['rustc', '-vV'], capture_output=True, text=True, check=True).stdout.splitlines():
        if line.startswith('host: '):
            return line[6:]
    raise SystemExit('rustc -vV did not report a host triple')


def third_party_licences(triple):
    meta = json.loads(cargo('metadata', '--format-version', '1', '--locked',
                            '--features', 'outstax-desktop/desktop', '--filter-platform', triple))
    pkgs = {p['id']: p for p in meta['packages']}
    nodes = {n['id']: n for n in meta['resolve']['nodes']}
    stack = [p['id'] for p in meta['packages'] if p['name'] in ('outstax', 'outstax-desktop')]
    seen = set()
    while stack:  # normal (linked) dependencies only, no dev/build deps
        node = nodes[stack.pop()]
        if node['id'] not in seen:
            seen.add(node['id'])
            stack += [d['pkg'] for d in node['deps'] if any(k['kind'] is None for k in d['dep_kinds'])]
    third = sorted((pkgs[i] for i in seen if pkgs[i]['source'] is not None), key=lambda p: (p['name'], p['version']))
    parts, apache_without_text = [], []
    for p in third:
        root = pathlib.Path(p['manifest_path']).parent
        texts = sorted(f for f in root.rglob('*') if f.is_file() and len(f.relative_to(root).parts) <= 3
                       and f.name.upper().startswith(LICENCE_PREFIXES) and 'target' not in f.parts)
        lic = p.get('license') or 'licence field missing'
        header = f"{'=' * 78}\n{p['name']} {p['version']}  ({lic})\n{p.get('repository') or ''}\n{'=' * 78}\n"
        if not texts:
            if 'Apache-2.0' in lic:
                apache_without_text.append(p['name'])
            parts.append(header + '(the published crate ships no licence file; the licence named above applies, '
                         'copyright its authors, see the repository)\n')
        for t in texts:
            parts.append(header + f'--- {t.relative_to(root).as_posix()} ---\n'
                         + t.read_text(encoding='utf-8', errors='replace').rstrip() + '\n')
    if apache_without_text:  # append the Apache-2.0 text once, from any crate that ships it
        for p in third:
            f = pathlib.Path(p['manifest_path']).parent / 'LICENSE-APACHE'
            if f.is_file():
                parts.append(f"{'=' * 78}\nApache License 2.0 (full text; applies to: {', '.join(apache_without_text)})\n"
                             f"{'=' * 78}\n" + f.read_text(encoding='utf-8', errors='replace').rstrip() + '\n')
                break
    intro = (f'Third-party components linked into outstax and outstax-desktop ({len(third)} crates, {triple}).\n'
             'Licence texts copied from the published crate sources, generated from Cargo.lock.\n\n')
    return intro + '\n'.join(parts), len(third)


README = """OutStax: independent client for INSTAX Link printers ({name})

What this is
  {cli:<20}command-line client (scan, status, test upload, print, trace tools, simulator)
  {gui:<20}desktop app (pick photo, adjust, test upload without film, print, history)
Both talk to INSTAX Link printers over Bluetooth LE.

Status
  Research release. Tests, the protocol simulator and the build pass. On real hardware it
  has been tested on one printer so far: an original mini Link (SP-4, firmware 0104) over
  Bluetooth LE on Windows 11, where a test upload and a real print both worked. Other models
  are untested. A print uses one sheet of film and is never retried.
  Start with "Test upload (no film)" in the desktop app, or:
    outstax scan
    outstax --device "EXACT_ID_FROM_SCAN" status
    outstax --device "EXACT_ID_FROM_SCAN" --trace upload.jsonl print photo.jpg   (no --commit: nothing prints)

Before you start
  Turn Bluetooth on, switch the printer on and close other printer apps. If uploads drop,
  try pairing the printer in the operating system's Bluetooth settings first.

Logs
  Desktop: logs/outstax-desktop.log in the app data folder (Windows: %APPDATA%\\outstax),
  plus one protocol trace per job in logs/traces (traces contain the image).
  Settings > "Open log folder". CLI: add -v / -vv / -vvv, or set OUTSTAX_LOG=debug.
  TROUBLESHOOTING.md explains the common errors and what to send with a report.

{platform_note}Source and build instructions: https://github.com/ThisLimn0/outstax
License: MIT (see LICENSE). Notices are in licenses/ and THIRD-PARTY-LICENSES.txt.
"""


MACOS_NOTE = """macOS
  OutStax.app is not notarised by Apple, so macOS blocks the first start. Open System
  Settings > Privacy & Security and click "Open Anyway" next to the OutStax message
  (on macOS 14 and older, right-click the app and choose Open instead). Allow Bluetooth
  when macOS asks. The command-line tool asks on behalf of the Terminal app you use.

"""


def app_bundle(binary, version):
    """Build OutStax.app around the desktop binary, sign it ad hoc and return its files."""
    plist = (PROJECT / 'platform/macos/Info.plist').read_text(encoding='utf-8').replace(
        '</dict></plist>',
        f'<key>CFBundleShortVersionString</key><string>{version}</string>\n'
        f'<key>CFBundleVersion</key><string>{version}</string>\n'
        '<key>LSMinimumSystemVersion</key><string>11.0</string>\n'
        '<key>NSHighResolutionCapable</key><true/>\n</dict></plist>')
    with tempfile.TemporaryDirectory() as tmp:
        app = pathlib.Path(tmp) / 'OutStax.app'
        (app / 'Contents/MacOS').mkdir(parents=True)
        shutil.copy2(binary, app / 'Contents/MacOS/outstax-desktop')
        (app / 'Contents/Info.plist').write_text(plist, encoding='utf-8', newline='\n')
        if shutil.which('codesign'):  # Apple Silicon refuses unsigned code, an ad hoc signature is enough
            subprocess.run(['codesign', '--force', '--deep', '--sign', '-', str(app)], check=True)
        return {f'OutStax.app/{f.relative_to(app).as_posix()}': f.read_bytes()
                for f in sorted(app.rglob('*')) if f.is_file()}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--no-build', action='store_true', help='package existing release binaries')
    ap.add_argument('--output-dir', type=pathlib.Path, default=PROJECT, help='folder for the zip (default: project root)')
    ap.add_argument('--target', help='cross-build with cargo-zigbuild for this Rust target, optionally with a '
                    'glibc suffix, e.g. x86_64-unknown-linux-gnu.2.31 (default: native build for this host)')
    a = ap.parse_args()
    packages = ['-p', 'outstax', '-p', 'outstax-desktop', '--features', 'outstax-desktop/desktop']
    root = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', PROJECT / 'target'))
    universal = a.target == 'universal-apple-darwin'
    macs = ('aarch64-apple-darwin', 'x86_64-apple-darwin')
    triple = macs[0] if universal else a.target.split('.', 1)[0] if a.target else host_triple()
    if universal:
        target = root / 'universal-apple-darwin' / 'release'
        if not a.no_build:
            for arch in macs:
                subprocess.run(['cargo', 'build', '--release', '--locked', '--target', arch, *packages],
                               cwd=PROJECT, check=True)
            target.mkdir(parents=True, exist_ok=True)
            for program in ('outstax', 'outstax-desktop'):
                subprocess.run(['lipo', '-create', '-output', str(target / program),
                                *(str(root / arch / 'release' / program) for arch in macs)], check=True)
    else:
        build = ['cargo', 'zigbuild', '--target', a.target] if a.target else ['cargo', 'build']
        if not a.no_build:
            subprocess.run([*build, '--release', '--locked', *packages], cwd=PROJECT, check=True)
        target = root / triple / 'release' if a.target else root / 'release'
    windows = 'windows' in triple
    macos = 'apple' in triple
    exe = '.exe' if windows else ''
    version = json.loads(cargo('metadata', '--format-version', '1', '--no-deps', '--locked'))['packages'][0]['version']
    system = 'windows' if windows else 'macos' if macos else 'linux' if 'linux' in triple else triple.split('-')[2]
    name = f'outstax-{version}-{system}-{"universal" if universal else triple.split("-")[0]}'
    a.output_dir.mkdir(parents=True, exist_ok=True)
    out = a.output_dir.resolve() / f'{name}.zip'
    if out.exists():
        raise SystemExit(f'{out} exists; move or delete it first (releases are never overwritten)')
    desktop = target / f'outstax-desktop{exe}'
    files = {f'outstax{exe}': target / f'outstax{exe}', 'LICENSE': PROJECT / 'LICENSE',
             'TROUBLESHOOTING.md': PROJECT / 'docs/guides/TROUBLESHOOTING.md'}
    files.update({f'licenses/{f.name}': f for f in sorted((PROJECT / 'licenses').iterdir())})
    for path in [*files.values(), desktop]:
        if not path.is_file():
            raise SystemExit(f'missing {path} (build first, or check CARGO_TARGET_DIR)')
    third, count = third_party_licences(triple)
    data = {n: p.read_bytes() for n, p in files.items()}
    if macos:  # a signed (ad hoc) .app bundle, so macOS can ask for Bluetooth permission
        data.update(app_bundle(desktop, version))
        gui, programs = 'OutStax.app', {'outstax', 'OutStax.app/Contents/MacOS/outstax-desktop'}
    else:
        gui = desktop.name
        data[gui] = desktop.read_bytes()
        programs = {f'outstax{exe}', gui}
    extra = {'README.txt': README.format(name=name, cli=f'outstax{exe}', gui=gui,
                                         platform_note=MACOS_NOTE if macos else ''),
             'THIRD-PARTY-LICENSES.txt': third}
    newline = '\r\n' if windows else '\n'
    data.update({n: t.replace('\n', newline).encode() for n, t in extra.items()})
    data['SHA256SUMS.txt'] = ''.join(f'{hashlib.sha256(b).hexdigest()}  {n}\n' for n, b in data.items()).encode()
    with zipfile.ZipFile(out, 'x', zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for n, b in data.items():
            info = zipfile.ZipInfo(f'{name}/{n}', date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3  # Unix, so unzip on Linux/macOS applies the mode below
            info.external_attr = (0o100755 if n in programs else 0o100644) << 16
            z.writestr(info, b)
    with zipfile.ZipFile(out) as z:
        if z.testzip():
            raise SystemExit('zip CRC check failed')
    print(json.dumps({'release': str(out), 'entries': len(data), 'bytes': out.stat().st_size,
                      'third_party_crates': count, 'sha256': hashlib.sha256(out.read_bytes()).hexdigest()}, indent=2))


if __name__ == '__main__':
    main()
