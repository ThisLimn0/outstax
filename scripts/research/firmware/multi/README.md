# Multi-firmware analysis tools (external)

Copies of `tools/multi/*.py` from the external analysis
`instax-multi-firmware-analysis` (delivered 2026-11-12), which covers Link WIDE 2,
SQUARE Link, mini Link 2 / 3 (Renesas RX) and WIDE Evo. They were not written for
this project and are not part of the Rust build. The package, its reports
(`report/multi`, `report/mini1`, `report/contents-addendum`) and the PDF stay in
`research/external/instax-multi-firmware-analysis/`. Vendor inputs and derived
files stay in `material/`. No firmware, vendor code, report or output is copied
into the project. `tools/NTKFWinfo.py`, `rx-elf-objdump` and binutils were not
copied. No multi script imports `NTKFWinfo.py`. Only `package_project.py` puts
it into the archive.

Adapted 2026-11-12: only path handling changed (via `../_paths.py`). Analysis
logic, hashes and assertions are unchanged. Each script sets
`sys.dont_write_bytecode`. The `sys.path` entries for the package's private
dependency folders (`tools/mini1/python`, `tools/multi/python`) were removed,
so dependencies come from the active environment. Path strings written into
outputs stay package-relative with `/` (for example `research/multi/inputs/...`).
Side changes, all outside the analysis:

- `acquire_*.py` write into `MULTI_MATERIAL` via `write_input()`. It refuses to
  overwrite an existing file whose SHA-256 differs.
- `evo_inspect.py`: the cpio extractor also skips names with a drive or root
  (`Path(name).anchor`), so a Windows path cannot leave the output folder.
- `sunp_extract.py`: asserts that each extracted FAT file stays inside its output folder.
- `evo_catalogue.py`, `package_project.py`: the Windows copy of the WIDE Evo
  rootfs holds the 373 recovered symlinks as regular files that contain their
  target. These scripts take the symlink list from the package's `SYMLINKS.json`.
- `assemble_report.py`, `render_report.py`: read and write Markdown as UTF-8.
  Without this, Windows' cp1252 default fails on the chapters.
- `renesas_disasm.py`: `RX_OBJDUMP` overrides the objdump path.
- `package_project.py`: takes `research/` from `MULTI_MATERIAL`, adds
  `third_party/binutils-2.44.tar.xz` from `material/references/binutils-2.44/`,
  and writes the archive to `MULTI_OUT`.

None of the scripts executes firmware, flashes a device or opens Bluetooth.
Only the three `acquire_*.py` use the network.

## Paths

| What | Location |
|---|---|
| Inputs and derived files (`research/contents`, `research/mini1`, `research/multi/{inputs,evo-derived,sunp-derived,renesas-derived}`) | `MULTI_MATERIAL` = `$OUTSTAX_MATERIAL/firmware/multi-analysis-2026-11-12/research/...` (override the whole folder with `OUTSTAX_MULTI_MATERIAL`) |
| Reports, `output/pdf`, `SYMLINKS.json`, `rx-elf-objdump` | `MULTI_EXTERNAL` = `$OUTSTAX_RESEARCH/external/instax-multi-firmware-analysis/` |
| All outputs | `MULTI_OUT` = `$OUTSTAX_RESEARCH/work/multi-report/`, mirroring the package layout (`report/multi/...`, `research/multi/...`, `output/...`) |
| binutils 2.44 source (rx-objdump rebuild, see the package's `tools/multi/rx-objdump/BUILD.md`) | `$OUTSTAX_MATERIAL/references/binutils-2.44/binutils-2.44.tar.xz` |

Environment overrides: `OUTSTAX_MATERIAL`, `OUTSTAX_RESEARCH`, `OUTSTAX_MULTI_MATERIAL`,
`RX_OBJDUMP` (the old `INSTAX_*` names still work as fallbacks). `_paths.py` provides `multi(rel)` (a package-relative input in
`MULTI_MATERIAL`), `multi_generated(rel)` (a file already regenerated in
`MULTI_OUT`, else the original: `research/...` from `MULTI_MATERIAL`, anything
else from the package, with a note on stderr), `multi_out(rel)` and `write_input()`.
A missing input exits with one message naming the path and the variable to set.
Output folders inside the project are refused.

## Running

From the project folder, in the package README order:

```sh
PY="../research/venv/Scripts/python.exe -I"   # run from the project root
M=scripts/research/firmware/multi
$PY $M/input_manifest.py
$PY $M/dex_routing.py
$PY $M/renesas_extract.py
$PY $M/renesas_disasm.py              # Linux: rx-elf-objdump
$PY $M/renesas_check_evidence.py
$PY $M/sunp_extract.py square research/multi/inputs/square/FW_0102.DAT   # cryptography, Pillow
$PY $M/sunp_extract.py wide2 research/multi/inputs/wide/F100.DAT
$PY $M/sunp_code.py square
$PY $M/sunp_code.py wide2
$PY $M/sunp_protocol_evidence.py
$PY $M/evo_extract.py research/contents/wide-evo.DAT    # host liblz4
$PY $M/evo_inspect.py                 # Linux liblz4.so.1
$PY $M/evo_catalogue.py               # cryptography
$PY $M/evo_code.py
$PY $M/evo_driver.py
$PY $M/evo_compare.py
$PY $M/evo_verify.py                  # cryptography
$PY $M/evo_validate.py
$PY $M/verify_project.py              # cryptography
$PY $M/assemble_report.py
$PY $M/render_report.py               # reportlab, pdfplumber, Linux DejaVu fonts
$PY $M/package_project.py             # optional: rebuilds the delivery ZIP (~371 MB) in MULTI_OUT
```

Input arguments (`sunp_extract.py`, `evo_extract.py`) are package-relative and
are resolved against `MULTI_MATERIAL`. Absolute paths also work.
`render_report.py` defaults to `MULTI_OUT` for `--output` and `--qa`.
The WIDE Evo rootfs folders come from `ubireader_extract_files` (ubi-reader,
with lzallright for LZO), as described in the WIDE Evo chapter. No script here
calls it.

### What runs where

- **Windows, current research venv** (Python 3.13, capstone and pyelftools,
  otherwise stdlib): `input_manifest`, `dex_routing`, `renesas_extract`,
  `renesas_check_evidence`, `sunp_code`, `sunp_protocol_evidence`, `evo_code`,
  `evo_driver`, `evo_compare`, `evo_validate`, `assemble_report`, `package_project`.
- **Need extra packages** (package `requirements.txt`: `capstone==5.0.9`,
  `ubi-reader==0.8.16`, `lzallright==0.2.6`, `pyelftools==0.33`,
  `cryptography==50.0.2`, `reportlab`, `Pillow`, `pdfplumber`):
  `sunp_extract` (cryptography, Pillow), `evo_catalogue`, `evo_verify`,
  `verify_project` (cryptography), `render_report` (reportlab, pdfplumber).
  ubi-reader and lzallright are only needed to re-extract the rootfs from UBI.
- **Need Linux**: `renesas_disasm` (the package's `rx-elf-objdump` is x86-64
  Linux, so rebuild it from binutils 2.44 or set `RX_OBJDUMP`), `evo_extract` (host
  `liblz4` via ctypes `find_library('lz4')`), `evo_inspect` (`liblz4.so.1`),
  `render_report` (fonts in `/usr/share/fonts/truetype/dejavu`).
- **Network, not run here**: `acquire_restore`, `acquire_mini_dex`, `acquire_evo_variant`.

On Windows, `write_text` writes CRLF. On a Windows run, `evo_catalogue` would
also record directory sizes and file modes from NTFS rather than from the rootfs.

### Snapshot check (2026-11-12, Windows, research venv, `-I`)

The 11 Windows-runnable analysis scripts (12 runs, `sunp_code` twice)
regenerated 24 files. After normalising line endings, all 24 are byte-identical
to the originals. In `report/multi/` these are dex-routing.json,
renesas-mini2/mini3.json and -registry.md, renesas-evidence-checks.json,
square/wide2-code-windows.json/.txt, square/wide2-protocol-evidence.json,
evo-validation.json and REPORT.md. In `research/multi/` they are
inputs/manifest.json, inputs/SHA256SUMS,
renesas-derived/mini2|mini3/{mini2|mini3.bin, selected-strings.json} and
evo-derived/{code, printer-code, printer-symbols, variant-comparison}.json.
`evo_validate` passed all 15 checks against the relocated material.
`package_project` reproduced `PACKAGING.json` and `SYMLINKS.json` (identical
apart from line endings). Its `SHA256SUMS` matches the package's, except for the
two lines hashing those CRLF-written files.

## Scripts

Original SHA-256 = `tools/multi/<script>` in the package (matches its `SHA256SUMS`).

| Script | What it does | Dependencies | Windows venv | Original SHA-256 | Adapted SHA-256 (2026-11-12) |
|---|---|---|---|---|---|
| `acquire_restore.py` | Range-request XAPK reader. Extracts and hash-checks the mini2/mini3 Intel HEX, WIDE `F100.DAT` and SQUARE `FW_0102.DAT`, then rebuilds RX binaries | stdlib, network | not run (network) | `087ffc9abf1bd9b4daf4548caa0a707d708f89925a272c19bf65410933870ae4` | `0f041983c810da04b378cf5e6133d6dc8633844fe11f0c56f75d0e273583798d` |
| `acquire_mini_dex.py` | Fetches only `classes.dex` of the mini Link 7.0.6 APK and checks its DEX checksums | stdlib, network | not run (network) | `057372d27411da262e64dcbdfed6aab75fbfecce45343a57d38463d929f3d302` | `58e99b6311b5c7469aab112ea14d928700312c3b5f9c0d535820cd2f13534b1e` |
| `acquire_evo_variant.py` | Downloads the WIDE Evo `Ver101` updater, checks it is the base image plus a 1,280-byte trailer | stdlib, network | not run (network) | `66931da69a5e2fbadcde4cf0fe9879573929b3158b068b42415c8d9509c1b019` | `0fca0f9aee893bd8c606cf7a9308c5b4e67e6320186b4a89c7c90f1876b6abb6` |
| `input_manifest.py` | Re-hashes every input and writes `inputs/manifest.json` + `SHA256SUMS` | stdlib | yes | `a58e5848ad9e5832c7807ffa1c8a079fe406a984a008e2f7206cd119a6e906a5` | `789b8e788e4caf590e12f6e2e91130e51e38816f35fe3c39df536531093fdefb` |
| `dex_routing.py` | Decodes selected DEX method bodies: model and CPU routing to the BO23 / LINK3 RenesasRX firmware | stdlib | yes | `8cd158e005b45cbca3117143c0ffaf7398bcef5bc52dcaf28e12c163a5b94381` | `7930e58efa174fefd34e3e8b71bc0e97b1d2ec10d705ccbb7ef68e3525c48248` |
| `renesas_extract.py` | Intel HEX validation, RX binary, 16-byte command descriptor registries, selected strings | stdlib | yes | `62efba0af7bd2aa6f310db9d2430c2e2338855b71434ca7eeed03128753aaf11` | `c0b0303f376c4474fe1f2bf8c28a1632f47da2640038a9ce3aef783575c0965e` |
| `renesas_disasm.py` | RX disassembly of selected handler windows | `rx-elf-objdump` | no (Linux binary) | `21ab9c51254d798925b003e5fa0a6d212c06e9e1f5ab9e10077aecc75921ab32` | `903cb234310039d7fb897e1a29c0731eff26960d51cf8008382fef4f8f496a18` |
| `renesas_check_evidence.py` | Hash-pinned exact-byte checks and registry invariants for mini2/mini3 | stdlib | yes | `703fb01e5a2b6f23259e8e9d5a9a8c57df907df9cfa4cf60679738bd27698bdb` | `fe18ff2865e7b5fd0d55220a929d86375980dddfab273315f764cda84e95b277` |
| `sunp_extract.py` | Raw-LZ4 decode of SQUARE/WIDE2 SUNP images, FAT extraction, FIF0 containers, gamma tables, JPEG metadata, RSA signature check, Bluetooth trailer | stdlib, cryptography, Pillow | no (packages) | `773390253649f83cdcdc27a0d30b38391fd7fea3c470777b668eb1fb4314f758` | `c1d72eac254c875a0ddf2b0adbb1698f4822797987b04558de0dc14b5ca81654` |
| `sunp_code.py` | MIPS32 LE disassembly of fixed evidence windows | capstone | yes | `6ccb40878cc7071826114642f7d88c4b1b55dac8805f4e572913d785c85faf5b` | `1f853b2edc45906998e89191e5274c3d34109eae881a08ab0a9b56bad04ed60c` |
| `sunp_protocol_evidence.py` | Protocol frame, command and geometry anchors with instruction regions | capstone | yes | `494dddacac1f37c809a54787d4adacc742c3c0445f94eb3d308ece22d2b05ee9` | `4053a68270ec17919c568eef4e0b5ef7a0b203e4ac3958fd59be6cdb8d15abee` |
| `evo_extract.py` | WIDE Evo DAT: chained LZ4 blocks, partition table, UBI parts | host liblz4 (ctypes) | no (liblz4) | `512c4f886bdd4a317219ecc50707765d922c0f2ee4dbd8db76e798d677e3e2f2` | `6894894d07730cd5444aeb8ee8ae3421091e1714d501dabbf0c1ae2bd2c366d1` |
| `evo_inspect.py` | DTBs, kernel, initramfs (bounded cpio), UBI/UBIFS CRC statistics | `liblz4.so.1` (ctypes) | no (Linux liblz4) | `fa969a0307131070e260b98a4039b253d30d21d2fea71b46de9c15c59961f22b` | `e0358a0bc785c06d179d390966eaff17cd0d952d008e7556a0d3378efe891514` |
| `evo_catalogue.py` | Catalogues the three rootfs trees: files, symlinks, ELF metadata, PEM metadata only | pyelftools, cryptography | no (cryptography) | `b0cef9f1602f1a4f7a7f77be520ef50672d45949c3ebf90526cb9995058a7c7b` | `7d8254a86dd1994b5854284a82df146c51204cf8d6cb24dcd07143b38c7a1689` |
| `evo_code.py` | ARM disassembly of selected `libhost.so` functions, `ab_printer.ko` symbols | capstone, pyelftools | yes | `c373619694e2c7202019af2096afab1e4acaf7f3c896548b9c6477848e59ca94` | `c32c7232c5487098071119d89cc1665b5807dd95974027d4bd07913accaf2d20` |
| `evo_driver.py` | ARM disassembly of `ab_printer.ko` functions with relocations, ioctl dispatch table | capstone, pyelftools | yes | `5158a7ff6f79f043b727cbb58e499247981fc77cd07749c4515082e9c749d0a6` | `688bd2a6aaab40db3bc41a5c0cc58de83a5368b68fbf9e022fb3bd265ac89ec0` |
| `evo_compare.py` | Compares the two WIDE Evo updater variants | stdlib | yes | `58caf5c4b63e2e49ad3c6b31fc3177be016a844254c2a8f18b2374e6871320a9` | `20c9052a4646f41c9d31b53383a42a15dbed51d8e90bacb0baaac90a25b2041b` |
| `evo_verify.py` | Offline RSA signature candidates for the Ver101 trailer | cryptography | no (cryptography) | `17f90023add126231257d9942a475419b53a652161ff8592e2652e3a3b911f8f` | `a1b42f1595eabb90aa34e7037cc179f5b2df1c4ed5fd9c6b2cda8c026ba287d2` |
| `evo_validate.py` | Checks recovered WIDE Evo data against source hashes and structural totals | stdlib | yes | `3321d939d5b23d95acf5f99c3eef731459a72c9ce054327e4b99219623ae4fe5` | `942299fbd320e37ad4c5d13176515b0b3b37d3f92834a34cf7535afadd3b496b` |
| `verify_project.py` | Runs three checkers, re-verifies SUNP signatures, file hashes, instruction bytes and the PDF hash | cryptography | no (cryptography) | `26ba4847adde9fa4b9b8142a799f6ec7b61a8d4f842e40b12e52deeccd408c97` | `b2e7838cffcd519f974b1545de79d4dac92ea975ced48b0eb4d175c871a15d4d` |
| `assemble_report.py` | Assembles `REPORT.md` from the chapters, registries and input manifest | stdlib | yes | `5fc49d4978fff631307a842c445f7e6c1960a54ab08f070619e44f86e6636b2c` | `aa0cfdbdce70f6555410eb3049d891c531101ebb1d2f8c64c54b173698cd29fc` |
| `render_report.py` | Renders `REPORT.md` to the PDF and writes `pdf-qa.json` | reportlab, pdfplumber, DejaVu fonts | no (packages, Linux fonts) | `5a693a719dc9baa838e8458b16e9e091c1264829fc4fbbe904271dc8427f0aa0` | `9f9576d7706a558f9c5f4de16b4166abba6e889523ebe89ee45862b94b617d8c` |
| `package_project.py` | Rebuilds the delivery ZIP with `SHA256SUMS`, `SYMLINKS.json`, `PACKAGING.json` without following symlinks | stdlib (Python 3.11+) | yes | `2a92c0008229a26158eb443de59c413797cc1460bbad88e7d1948c0bff3da041` | `8287cd3f9537df8d50eabf7c5a96a3700e4ee4acdaeb8de32d8c002f1bc196bd` |
