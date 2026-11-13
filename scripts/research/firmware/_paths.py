"""Input and output locations for the adapted firmware research scripts (stdlib only).

Layout (the folder that contains the project): <project> = outstax, material/ (vendor inputs, never
shipped), research/ (analysis workspace). Override the two outside folders with
OUTSTAX_MATERIAL and OUTSTAX_RESEARCH (and the multi-analysis inputs with
OUTSTAX_MULTI_MATERIAL). The old INSTAX_* names still work as fallbacks.
Outputs go to research/work/, never into the project.
"""
import hashlib
import os
import sys
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[3]
OUTSTAX = PROJECT.parent


def env(name):
    """OUTSTAX_<name>, else the old INSTAX_<name>, else None."""
    return os.environ.get('OUTSTAX_' + name) or os.environ.get('INSTAX_' + name)


MATERIAL = Path(env('MATERIAL') or OUTSTAX / 'material').resolve()
RESEARCH = Path(env('RESEARCH') or OUTSTAX / 'research').resolve()

MINI1 = MATERIAL / 'firmware/mini1-0104'          # 0104.hex, mini1-0104.bin
IJG = MATERIAL / 'references/ijg-9a'              # ijg-source.json + headers
EVO_IMAGE = MATERIAL / 'firmware/bundle-2026-11-11/4f412d0868844321_FPUPDATE.DAT'
EXTERNAL = RESEARCH / 'external/instax-mini1-firmware-re'
INPUT_JSON = EXTERNAL / 'research/mini1/input.json'
FIXTURES = EXTERNAL / 'research/mini1/fixtures'
CRATE = EXTERNAL / 'rust/instax-mini1'            # archived; facts merged into crates/protocol
SNAPSHOT = EXTERNAL / 'report/mini1'
OUT = RESEARCH / 'work/mini1-report'
CONTENTS_OUT = RESEARCH / 'work/contents'

# Multi-firmware analysis (multi/): vendor inputs and derived files keep the
# package-relative layout (research/...) under MULTI_MATERIAL; reports,
# output/ and SYMLINKS.json stay in the package; outputs mirror it in MULTI_OUT.
MULTI_MATERIAL = Path(env('MULTI_MATERIAL')
                      or MATERIAL / 'firmware/multi-analysis-2026-11-12').resolve()
MULTI_EXTERNAL = RESEARCH / 'external/instax-multi-firmware-analysis'
MULTI_OUT = RESEARCH / 'work/multi-report'


def need(path):
    """Return an existing input path or exit with one message naming it."""
    path = Path(path)
    if not path.exists():
        var = ('OUTSTAX_MULTI_MATERIAL' if path.is_relative_to(MULTI_MATERIAL)
               else 'OUTSTAX_MATERIAL' if path.is_relative_to(MATERIAL) else 'OUTSTAX_RESEARCH')
        sys.exit(f'missing input: {path} (set {var} to override its base folder)')
    return path


def out_dir(path=OUT):
    """Create and return an output folder; refuse anything inside the project."""
    path = Path(path).resolve()
    if path.is_relative_to(PROJECT):
        sys.exit(f'refusing to write inside the project: {path} (set OUTSTAX_RESEARCH)')
    path.mkdir(parents=True, exist_ok=True)
    return path


def report(name):
    """A generated report from OUT, else the external package's snapshot."""
    path = OUT / name
    if path.exists():
        return path
    print(f'note: {path} not generated yet, using snapshot {SNAPSHOT / name}', file=sys.stderr)
    return need(SNAPSHOT / name)


def multi(rel):
    """Map a package-relative input path ('research/multi/inputs/...') to MULTI_MATERIAL."""
    return need(MULTI_MATERIAL / rel)


def multi_generated(rel):
    """A package-relative file regenerated in MULTI_OUT, else the original
    (research/... from MULTI_MATERIAL, anything else from the package)."""
    path = MULTI_OUT / rel
    if path.exists():
        return path
    orig = (MULTI_MATERIAL if Path(rel).parts[0] == 'research' else MULTI_EXTERNAL) / rel
    print(f'note: {path} not generated yet, using {orig}', file=sys.stderr)
    return need(orig)


def multi_out(rel):
    """Output path for a package-relative file, under MULTI_OUT."""
    path = MULTI_OUT / rel
    out_dir(path.parent)
    return path


def write_input(path, data):
    """Write an acquired input; refuse to overwrite one whose SHA-256 differs."""
    path = out_dir(Path(path).parent) / Path(path).name
    if path.exists() and hashlib.sha256(path.read_bytes()).digest() != hashlib.sha256(data).digest():
        sys.exit(f'refusing to overwrite {path}: existing file has a different SHA-256 '
                 '(set OUTSTAX_MULTI_MATERIAL to use another folder)')
    path.write_bytes(data)
    return path
