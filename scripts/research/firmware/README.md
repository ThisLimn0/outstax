# External firmware-analysis tools

These scripts are copies of tools from three external static analyses, added
on 2026-11-12. They were not written for this project and are not part of the
Rust build. The originals, their reports and all inputs stay outside the project
(research/external/…, research/instax-firmware-research/, material/…). No
firmware, vendor code or report output is copied into the project.

On 2026-11-12 the mini Link tools and `fat_contents.py` were adapted so they run
from the project. Only their path handling changed (via `_paths.py`), and the
analysis logic, hashes and assertions are unchanged. Their SHA-256 values
therefore no longer match the origin folders' SHA256SUMS, so the tables below
list both. There are two side changes: `verify_image.py` writes `mini1-0104.bin` only when it is
missing, and `acquire.py` refuses to overwrite an existing input whose SHA-256
differs. `apk_research.py` and `decode_liplay_firmware.py` are unchanged copies.

None of the scripts executes firmware, flashes a device or opens Bluetooth.
`acquire.py`, `apk_research.py` and `check_rust_targets.py` use the network.

The scripts of a fourth external analysis are in [`multi/`](multi/README.md).
This is the multi-firmware analysis of Link WIDE 2, SQUARE Link, mini Link 2 / 3
and WIDE Evo, delivered 2026-11-12. That README covers origin, dependencies, paths,
run results and hashes. They use the same `_paths.py`, with `OUTSTAX_MULTI_MATERIAL`
and outputs in `$OUTSTAX_RESEARCH/work/multi-report/`.

## Running

From the project folder, with a Python 3.10+ environment that has Capstone 5
(`../requirements.txt` pins 5.0.9, and a 5.0.7 run reproduced the snapshot):

```sh
PY="../research/venv/Scripts/python.exe -I"   # run from the project root
$PY scripts/research/firmware/verify_image.py      # check 0104.hex, rebuild/verify the BIN
$PY scripts/research/firmware/startup.py
$PY scripts/research/firmware/protocol_extract.py
$PY scripts/research/firmware/runtime_report.py
$PY scripts/research/firmware/status_extract.py
$PY scripts/research/firmware/settings_extract.py
$PY scripts/research/firmware/update_extract.py
$PY scripts/research/firmware/image_extract.py
$PY scripts/research/firmware/registry_extract.py  # needs protocol/settings/update.json
$PY scripts/research/firmware/check_evidence.py    # needs the five evidence reports
$PY scripts/research/firmware/fat_contents.py
```

Helpers: `runtime_scan.py` (survey), `protocol_flow.py ADDR…`,
`protocol_inspect.py {dis,strings,refs,imm,table} …`, `runtime_disasm.py ADDR SIZE`
print to stdout (`runtime_scan.py` also writes `runtime-survey.json`).
`registry_extract.py` and `check_evidence.py` read reports from the output
folder and fall back to the external snapshot (`report/mini1/`) for any report
not generated yet, with a note on stderr.

Network: `acquire.py` (downloads the Mini Link XAPK) and `check_rust_targets.py`
(downloads Rust target stdlibs and needs the crate's Linux `.toolchain/installed`).
`check_rust_cli.py` is offline but needs the archived crate built first with
`cargo build --release --offline --manifest-path <crate>/Cargo.toml`. It honours
`CARGO_TARGET_DIR`. The `instax-mini1` crate is archived in the external package,
and its facts were merged into `crates/protocol`.

## Paths

`_paths.py` (stdlib only) resolves everything from the project's parent folder
(the folder that contains the project). Two environment variables override the outside folders:

- `OUTSTAX_MATERIAL` (default `<parent>/material`): vendor inputs, never shipped.
- `OUTSTAX_RESEARCH` (default `<parent>/research`): analysis workspace and outputs.

The old names `INSTAX_MATERIAL`, `INSTAX_RESEARCH` and `INSTAX_MULTI_MATERIAL` still
work as fallbacks when the `OUTSTAX_*` variable is not set.

| What | Location |
|---|---|
| mini Link inputs | `$OUTSTAX_MATERIAL/firmware/mini1-0104/0104.hex` (SHA-256 `f3d3e5d2b981555c6aa6b059504bc1416d17303dede8e07c2fec057fce91b0f7`), `mini1-0104.bin` (SHA-256 `b04e3490f57f93f562edc868dc34cc06ce670dd41476bf91782e9519aefcda4c`). `acquire.py` writes here |
| IJG reference | `$OUTSTAX_MATERIAL/references/ijg-9a/ijg-source.json` |
| mini Evo image | `$OUTSTAX_MATERIAL/firmware/bundle-2026-11-11/4f412d0868844321_FPUPDATE.DAT` (official 1.09 `FPUPDATE.DAT`, 97,328,468 bytes, SHA-256 `4f412d08688443212c2cf350586a8c01d43d467c580eb19389a82b40929a93ed`) |
| external package | `$OUTSTAX_RESEARCH/external/instax-mini1-firmware-re/`: `research/mini1/input.json`, `research/mini1/fixtures/`, `rust/instax-mini1/` (archived crate), `report/mini1/` (snapshot) |
| mini Link outputs | `$OUTSTAX_RESEARCH/work/mini1-report/` |
| `fat_contents.py` output | `$OUTSTAX_RESEARCH/work/contents/mini-evo-fat-files.json` |

A missing input exits with one message naming the expected path and the variable
to set. Output folders inside the project are refused. The scripts set
`sys.dont_write_bytecode` so no `__pycache__` appears here.

`apk_research.py PACKAGE OUTPUT_DIR` and `decode_liplay_firmware.py INPUT -o OUTPUT`
take explicit paths. The decoder needs the `lz4` package (`lz4>=4.3,<5`).

## Origin and purpose

From instax-mini1-firmware-re, `tools/mini1/` (static RE of the original
mini Link firmware `assets/fw/BOD/0104.hex`):

| Script | What it does | Original SHA-256 | Adapted SHA-256 (2026-11-12) |
|---|---|---|---|
| `acquire.py` | Downloads the Mini Link XAPK by HTTP range requests, extracts `0104.hex`, checks its hash and writes the flat binary | `4c4dcc8c210536ac4be573b6c551cb8d6a18f63fe1a2bdef85f70cf36e357af8` | `7ba7e3b4f046ac96013c98f900e4ce210751f770812b5443745cca5ba4132c66` |
| `verify_image.py` | Validates every Intel HEX record and rebuilds the flat image | `71d8aae3b4379cbb0fea41c70f17dcebd54eb6cb16695b42bb73e4ca73b2acb8` | `2691229af3c91de50ab257fd19ec35851bba773c731bcb2318db10b57df13cb6` |
| `startup.py` | Recovers header, vectors and the runtime initialization table | `bf60c377ebc67fa872274b0a4cfd9182eacbf6dc80eee928bce157680a2fdded` | `201ab48e6fa98669678e464ed24c6e5c38cdae8eec47cf5490f2edd5ffa0face` |
| `protocol_extract.py` | Extracts the command table and selected handler disassembly | `33b0c90b7c11ec7629fc4218b0eeb560786e590ca938f40f6cc940ba1013cfd0` | `c63e528059446fcc9a94ca405f4116fde387d0087c4dd6c6f27590c5c946aae2` |
| `protocol_flow.py` | Conservative control-flow walk from given Thumb entry points | `c19e37306ec0903012a4267af5e9a04469a0b7e985b9933972c98f0871259c97` | `48973779b849e0a5db40aaa51970875a1cadb5e9b3e803d700674e8b81925f1e` |
| `protocol_inspect.py` | Read-only inspection helper (disassembly, strings, refs, immediates, tables) | `c86e5d007afbca820ff4ed0ea7ff7b87e9ef2907ca8768a10f61c0fe6f0ca57c` | `6f81062dcda50cd83d942232dd9b05074cf4db7f59fdf569b984b634ac345421` |
| `runtime_disasm.py` | Disassembles a range with literal-pool annotation | `a240b1e380a11af80ee2738cff922c2e5099c401de7e440d231afb28aa86b764` | `33ce6c553dedc718236f8925a18b928fcd1bf49ad39c00a099cb9c4f1348cf01` |
| `runtime_report.py` | Task, RTOS, resource and peripheral evidence report | `71433ecfbac2fd4c8194f606d31f1a280906bdde64f372b12da04aaa23971977` | `48a186c33c730fc92cc4fee65f79a110f75e60e927a8cb0604ef6c073f189dc5` |
| `runtime_scan.py` | Exploratory string and fill-run survey | `05049cad19b11757b40f2bfe91626e0fb47f8474746659f31b574909df9848fa` | `f8e1ed6f4d02e936c06b64c33d448702976faaf0c75d7be9c8cd31c1d05f78f5` |
| `status_extract.py` | Status, identity and result-code evidence | `46ca5326bcb795dd16b94fd94263282c2c73eecfc04583cd481747db7107e5c4` | `4ace9f1d032120bf88714a67860e5d5f9f79f511ff7e10ebf362820fac0e1422` |
| `settings_extract.py` | Control and readback command evidence (`0102`, `0103`, …) | `07e2ae3005be6390b45b9049f7d021a128b360b8febea947bce5b8a079145093` | `0c6c34b51b72a38cc12f8658779e601acc397651f7f8bbe30d1689902b395171` |
| `update_extract.py` | Firmware-update and auxiliary-command evidence | `61748ae66b1263bda0cf3f46062d75c62efec62c62a0c4fe9f1c6d15ff311afc` | `367e8aefc4c1d633ae8659b2484de4cf18adeed93626f78cdc644c78710440e7` |
| `image_extract.py` | JPEG wrapper evidence (embedded IJG decoder) | `e6e066794402bf850d6d4912ff5eac89c565e37f4ffbbe7d7216c31f90763985` | `8492ac628c3737f44ee39cfc09c118e8d0253e20e5aac130274580be5be7bcdd` |
| `registry_extract.py` | Combines the decoded command schemas into one device profile | `c3d847e6d8dae1ea41bdd3382774a7e0e6fc3c996bfe5369b931f312dec2439d` | `6a6f106933f16f9a6f1c9c116961551888aa001c90c0dd29957207afada64d5a` |
| `check_evidence.py` | Checks published instruction bytes and descriptors against the image | `ab3b9c45e7c3c18d43a2c377218cf3fce2f2bc9a8b70979ad8aff2baf20a7ef1` | `039d605b06cea7e09fd5b14e841380d555f5c428bd293a942bc7f3b3187265d1` |
| `check_rust_cli.py` | Checks the external analysis's own `instax-mini1` crate (not this project) | `f62b56b1f9e3890fdda23707e39a4ab3f05b6c7edd9e41bba994bfce4ac47662` | `d574f1c6bf9ac87c6e81fb0d063836025cb5f011d654a606892342a34ebeee3c` |
| `check_rust_targets.py` | Cross-target `cargo check` for that external crate | `7d95d971ea7e7101d49bba7e8ef943a9d0eb77ebb75658c3cae9d3b2ecec47f9` | `84daa3a6d43254e3a224230fdda1bbd493f27d640d0dc3a61cf8b74bb471df83` |

From the contents addendum, `tools/`:

| Script | What it does | Original SHA-256 | Adapted SHA-256 (2026-11-12) |
|---|---|---|---|
| `fat_contents.py` | Read-only parser of the two FAT16 volumes in the mini Evo update image. Lists paths, sizes and leading bytes | `64b917bda88a77b4fcd2dc64964895d3b970081b9d5f950b4d89ac02c7477ed9` | `155f7ca649ac001b84bde9ff4ce28d4c471e6a98927726b4142fc6ddf005125d` |

From the firmware research bundle (instax-firmware-research), `scripts/`:

| Script | What it does | SHA-256 (unchanged) |
|---|---|---|
| `apk_research.py` | Downloads an app XAPK by range requests, saves base/asset/ARM64 splits, extracts firmware-like assets and DEX string references | `656bd00289d3bcf304fe5aec7c9ad62bae5aae62abe4e40490c2bd7fb5c2e755` |
| `decode_liplay_firmware.py` | Decodes a raw LZ4 block (LiPlay/LiPlay+ app payload) to a new file, with an optional expected SHA-256 | `afaf7cbc7e2a7abd89486afc08e1167dbd7b763f548620b4234365e403b08592` |

Findings from these analyses are summarized in
[firmware-contents.md](../../../docs/re/firmware/firmware-contents.md) and
[mini-link-0104.md](../../../docs/re/firmware/mini-link-0104.md).
