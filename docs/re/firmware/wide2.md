# Link WIDE 2 firmware `BO49/F100.DAT`

Source: external multi-firmware analysis, 2026-11-12. The package,
`instax-multi-firmware-analysis`, is kept outside the project at
research/external/instax-multi-firmware-analysis. Its WIDE 2 chapter is
`report/multi/wide2.md` (SHA-256
`73c906463510d013ba75bc2c8f843479c643af903d77fea2b00faea98877c14f`), and the full
report is `report/multi/REPORT.md` (SHA-256
`31f913997398dde0045201932bb7dc61c1304619b25c4481ed1f0c5527c9f35b`). All package
files matched its SHA256SUMS. Vendor inputs and derived files are under
material/firmware/multi-analysis-2026-11-12, outside the project. The analysis
is static only. Nothing was executed or flashed, and no printer was contacted.

Labels are as in [square.md](square.md): **recovered**, **likely** and **unknown**.
*Our check* marks a strings-only look at the package's decoded image on
2026-11-12, not part of the external analysis.

Cleaned protocol facts: [`data/firmware/wide2-protocol-evidence.json`](../../../data/firmware/wide2-protocol-evidence.json).
Instruction windows (`wide2-code-windows.*`) and the resource and Bluetooth
catalogue (`wide2-evidence.json`) stay in the package, outside the project.

"WIDE2" is the F100 asset under the Link WIDE app's `BO49` directory (model
`FI049`). It is not the WIDE Evo camera ([wide-evo.md](wide-evo.md)).

## Container and CPU

| Fact | Label |
|---|---|
| Input: `assets/Firmware/BO49/F100.DAT` from Link WIDE app 2.0.0 (204), 3,734,383 bytes, SHA-256 `16187d3f…19bb4` | recovered |
| One raw LZ4 block, decoding to 8,300,044 bytes (SHA-256 `c18f0482…a4acb`) starting `SUNP BURN FILE` | recovered |
| Same SUNP layout as SQUARE: code at `0x3b2b20` → VA `0x80000000`, HDR3 `0x754920`, declared SUNP end `0x754e10` | recovered |
| MIPS32 little-endian, ThreadX MIPS32_4Kx G4.0c.4.0 banner | recovered (code), chip marking unknown |
| FAT12 volumes A (55 files) and B (3 files). 18 gamma tables (all checksums `0xffff`). Three built-in JPEGs `WIDE_SP{1,2,3}.JPG`, each 1260×840 | recovered |
| Appended Bluetooth package after the SUNP end: magic `0xabb707ab`, 256-byte header, 11 components partitioning 610,808 bytes. MIPS code at `0x52d788` checks the magic and reads it | recovered |
| RSA-2048 / PKCS#1 v1.5 / SHA-256 signature at `0x7ea108` covers the SUNP part, the whole Bluetooth package and 1,024 metadata bytes, and verifies with the bundled `BPUB.PEM`. On-device verifier wrapper at `0x52d874` → `0x801d2674` | recovered (offline), full update policy unknown |

Of the Bluetooth components, index 0 is a 3,788-byte Thumb stub. Indexes 1–10 carry
IDs `0100 0400 0200 0300 0410 0500 0602 0700 0800 0a01` with `RTL8753BFR`,
`RTL8763BO` or `RTL8773B` headers (**recovered**). *Our check:* that is exactly
the ID set of the Link 3 Bluetooth bundle in
[`data/firmware/bt-firmware-components.json`](../../../data/firmware/bt-firmware-components.json), with
different builds (for example APP `0400` has the same 48,580-byte size but a
different SHA-256). So WIDE 2 **likely** uses the same Realtek RTL8753BFR-class
module as mini Link 3. The package itself does not claim which chips are fitted.

## Command dispatcher and opcode set

- `FFProtocol_Recv_Process` at `0x57d598` / VA `0x801caa78`: `Ab` marker
  (literal at `0x74d700`), **BE u16 total length** at +2 (byte-swap helper
  `0x801ff824`), group +4, subtype +5, payload +6. Responses `aB`, 6-byte header
  (`0x578828`). Checksum helper `0x578460`: byte sum + trailing byte = 255.
  **recovered**
- Group dispatcher `0x57c568`: `0x10` → image (`0x801c63c4`), `0x20` → firmware
  download (`0x801c6afc`), `0x01` → common family. **recovered**
- Full opcode list and payload schemas: **unknown**. No descriptor table is
  reported.

## Upload

| Item | WIDE 2 | Label |
|---|---|---|
| `1000` START request | 15-byte frame: picture type, option, option2, a fourth byte, image byte count BE32 | recovered |
| START fields copied | **all four** option bytes are copied into separate fields (`0x579274`–`0x5792ac`). The fourth byte's meaning is unassigned | recovered |
| START checks | no validator, size cap or rejection codes reported | **unknown** |
| After START | calls the download-start service, opens a receive file, stores `ceil(image_byte_count / segment_size)` as the block count (`0x579320`–`0x579370`) | recovered |
| START reply | 12-byte frame: result + segment size BE32, where the size is a runtime value. The 0x2800-byte stream buffer is **not** a fixed chunk size | recovered |
| `1001` DATA | `seq:BE32` + data, frame = chunk + 11 bytes. The data length is taken from the frame (`0x57da50`–`0x57da94`) | recovered |
| Sequence check | the handler compares the received sequence with the **expected** sequence and the total block count (`0x5795ec`–`0x579618`) | recovered (check), but the error code is not given |
| Final block | the image-data writer (`0x578084`) bounds accumulated data by the allocated receive buffer and writes to the file | recovered. Whether trailing zero padding is truncated or rejected is **unknown** |
| Geometry | code sets 1260×840 at `0x55c098`/`0x55c0a8` and in three processing branches. START carries a byte count only, so no single accepted upload size is established. The package says there is no basis for 1260×800 | recovered (processing default) |
| Max JPEG | not established | **unknown** |

Transfer timer: **not covered** by the analysis (no timeout constant, reload
rule or expiry). **unknown**

## Sleep / auto power-off

Not analysed. **unknown.** *Our check:* the image has the same `BTInterface_AutoSleepSettingFromAPP`,
`AutoSleepSettingReq/Res` and `nAutoOffTime` strings as SQUARE (**likely** an
app-driven setting). Units, defaults and reset rules are **unknown**.

## Bluetooth: name, advertising, pairing, link drops

- Package strings: GATT and SPP callbacks into the same parser
  (`FFProtocol_GattRecvCallback`, `FFProtocol_SppRecvCallback`), a Realtek UART
  host (`realtekControlInit`, `Realtek_EnterHciMode_Seq`, `Host_DownloadRealtekRamPatch`,
  `Host_RealtekStkUpdateBaudrate`, `B:\CFG_RTL8753BFR.bin`, `B:\APP_RTL8753BFR.bin`),
  `rtl8761att ready`, and `FFProtocolClient_DisconnectedControl`. The MIPS
  application loads the module's patch/config at start. **likely**
- Pairing, bonding, encryption requirement and supervision timeout: **not
  covered.** The WIDE 2 app's 28 s "must be bonded" rule
  ([connection.md](../apps/connection.md)) is still neither confirmed nor refuted.
- *Our check (name):* format `%s-%s%s` next to `INSTAX-P`, `(EDR)` and `(BLE)`,
  plus `INSTAX-P-%s`, `BT_NAME:%s` and `SIDC_PRODUCT_NAME = "INSTAX Link WIDE 2"`.
  The names are **likely** `INSTAX-P-<id>(BLE)` and `INSTAX-P-<id>(EDR)`,
  matching the app's BLE filter family `instax-P-A3` (APP_RE.md). Unlike the
  other Link printers, the prefix is `INSTAX-P-`, not `INSTAX-<digit>`.

For link drops: the package gives no firmware-side disconnect timer. The
strict expected-sequence check means a resent or skipped block is refused
rather than overwritten (STM32 overwrites). The client never resends blocks,
so this does not affect it today.

## Agreement and disagreement with our docs

- **Agrees:** envelope, checksum and START/DATA layout. The START reply carries
  the block size, and 1260×840 matches `Format::Wide`.
- **Corrects** [firmware.md](firmware.md) "Not analysed" note: F100 is now
  decoded (one raw LZ4 block → SUNP/MIPS32), with an appended Realtek
  Bluetooth package.
- **Differs from the STM32 printers:** DATA has an expected-sequence check
  (STM32: none, [firmware.md](firmware.md) §2). The block size is not fixed at
  900, and the fourth START byte is stored (STM32: ignored).
- **Open for the client:** START checks, block-size value, final-block padding,
  transfer timer, bonding. See [README-implications.md](README-implications.md).
