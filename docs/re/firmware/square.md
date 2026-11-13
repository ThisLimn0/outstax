# SQUARE Link firmware `FW_0102.DAT`

Source: external multi-firmware analysis, 2026-11-12. The package,
`instax-multi-firmware-analysis`, is kept outside the project at
research/external/instax-multi-firmware-analysis. Its SQUARE chapter is
`report/multi/square.md` (SHA-256
`e693a8c25b019bc9d60ab14398a7e97dfc09e5c87ca15b631ee91c242b8cf5aa`), and the full
report is `report/multi/REPORT.md` (SHA-256
`31f913997398dde0045201932bb7dc61c1304619b25c4481ed1f0c5527c9f35b`). All package
files matched its SHA256SUMS. Its vendor inputs and derived file systems are under
material/firmware/multi-analysis-2026-11-12, outside the project. The analysis is
static: nothing was executed, emulated or flashed, and no printer was contacted.

Labels:
- **recovered**: the package reads it from bytes or traced code.
- **likely**: inferred from structure, names or strings.
- **unknown**: not established.

Rows marked *our check* are a strings-only look at the package's
decoded image on 2026-11-12, not part of the external analysis.

Cleaned protocol facts: [`data/firmware/square-protocol-evidence.json`](../../../data/firmware/square-protocol-evidence.json).
The full instruction windows (`square-code-windows.txt`/`.json`) and the
resource catalogue (`square-evidence.json`) stay in the package, outside the project.

## Container and CPU

| Fact | Label |
|---|---|
| Input: `assets/Firmware/FW_0102.DAT` from SQUARE Link app 1.6.2 (60), 2,486,908 bytes, SHA-256 `9f6e1315…ddac94` (same file as [firmware-contents.md](firmware-contents.md)) | recovered |
| One raw LZ4 block from offset 0, with no frame wrapper. It decodes to 7,051,540 bytes (SHA-256 `65c323b2…a076150`) starting `SUNP BURN FILE` | recovered |
| Header pointers: HDR1 `0xb1a00`, HDR2 `0xb2a00`, application code `0x3b2b20` (mapped to VA `0x80000000`), HDR3 `0x6b8f20`, declared end `0x6b9410` | recovered |
| CPU: MIPS32 little-endian (startup CP0 code, stack at `0xd0002000`). RTOS banner ThreadX MIPS32_4Kx G4.0c.4.0 at `0xb154c` | recovered (code), but the marketed chip is unknown |
| Two FAT12 volumes (not FAT16): A at `0xb2b20` (2 MiB, 70 files) and B at `0x2b2b20` (1 MiB, 3 files) | recovered |
| Resources: calibration/rank tables (`CLBRANK*`, `SHDRANK*`, `SHDTBL`), 36 gamma tables of 256 LE u16 + complement checksum, `FIF0` containers with 12 child records, a public key `BPUB.PEM` | recovered (format), runtime use of each table unknown |
| RSA-2048 / PKCS#1 v1.5 / SHA-256 signature at the declared end, over everything except the signature and the final 4-byte checksum, verifies with the bundled `BPUB.PEM`. The LE u32 word sum is 0 | recovered (offline), device-side enforcement unknown |

## Command dispatcher and opcode set

- `FFProtocol_Recv_Process` at file `0x57538c` / VA `0x801c286c` compares the
  `Ab` marker, reads a **big-endian u16 total length** at +2, the group at +4 and
  the subtype at +5. The payload starts at +6. Responses start `aB` (builder at
  `0x570af8`). Checksum: byte sum of the whole packet mod 256 = `0xff`. This is
  the same envelope as [PROTOCOL.md](../../protocol/PROTOCOL.md). **recovered**
- The group dispatcher (`0x57425c`) branches on groups `0x01`, `0x10`, `0x20`,
  `0x30` and more. The image switch starts at `0x570b5c`, and the firmware-download
  switch compares subtypes `0, 1, 2, 3, 0x10, 0x80, 0x81`. **recovered**
- Dispatch is reported as per-group switches. No 16-byte descriptor table
  like the STM32 and RX ones is reported (**likely** absent, though not stated). The
  full opcode list, payload layouts and error paths are **unknown**. In particular, whether group `0x01`
  handles `0103` BLE_CONNECT is not established.
- The package's string list names the switches (`FFProtocol_Print_Image_Type_Switch`,
  `…_FW_Download_…`, `…_Information_…`, `…_Camera_Setting_…`, `…_Common_…`) and
  handlers including DownloadStart/Download/DownloadEnd/DownloadCancel,
  PrintImage and RejectFilmCover. Camera-family switch names (Liveview, URL
  upload, Frame picture) are generic framework strings and not proof that the
  printer exposes them. **likely**

## Upload

| Item | SQUARE | Label |
|---|---|---|
| `1000` START request | 8 payload bytes, 15-byte frame: picture type, option, option2, a fourth byte, image byte count BE32 | recovered |
| START fields copied | first three bytes copied into zeroed fields (`0x570e5c`–`0x570e88`). The **fourth byte is not copied** on this path | recovered |
| START checks (type, option bits, size cap) | not traced: no validator, no maximum byte count and no rejection codes are reported | **unknown** |
| START reply | 12-byte frame: result u8 + segment size BE32 (`0x570ef4`–`0x570f64`) | recovered |
| Block size | the segment size is a runtime value (strings `Hce_FFProtocol_SetSegmentSize`/`GetSegmentSize`), but its value is not given | recovered (field), value unknown |
| `1001` DATA | `seq:BE32` + image bytes, frame length = chunk + 11. The parser subtracts 5 overhead bytes and passes data at +4 (`0x5757f4`–`0x575838`). The reply is 12 bytes with result + sequence | recovered |
| Short or padded final block | not stated. The parser takes the data length from the frame length | **unknown** |
| Sequence checks, retry and cancel paths | not traced | **unknown** |
| Max JPEG / geometry | processing allocates 800×800 (`0x54f1b4`, clear of 0x138800 = 800·800·2 bytes). The START fields carry a byte count only, so no wire-input size limit is established | recovered (processing), input limit unknown |

The transfer timer (the STM32 "watchdog" of [firmware.md](firmware.md)) is **not
covered** by the analysis: no timeout constant, reload rule or expiry
behaviour is reported for SQUARE. **unknown**

## Sleep / auto power-off

Not analysed by the package. **unknown.**
*Our check:* the decoded image contains `BTInterface_AutoSleepSettingFromAPP`,
`AutoSleepSettingReq/Res`, `AutoSleepTime` and `nAutoOffTime`, so an app-driven
auto-sleep setting exists (**likely** the `0102` path the app uses). Units,
defaults and what resets the timer are **unknown**.

## Bluetooth: name, advertising, pairing, link drops

- Strings in the package's evidence list: `FFProtocol_GattRecvCallback`/`…Send…`/
  `…Error…` next to `FFProtocol_SppRecvCallback` (both a GATT and an SPP path into
  the same parser), `realtekControlInit`, `realtekUartInit`, `Realtek_PowerOn_Seq`/
  `PowerOff_Seq`, `realtekStackMonitor_BleConnectParameterSet`, chip names
  `RTL8762BEE` / `RTL8761ATT`, and `Realtek not support BTA_DmSetGattGapDeviceName?`.
  So the MIPS application drives a Realtek controller over UART, serves the Link
  protocol over GATT as well as SPP, and has an LE connection-parameter path.
  **likely** (from strings, call sites not traced)
- `FFProtocolClient_DisconnectedControl` exists, but its trigger is **unknown**.
- No pairing, bonding or supervision-timeout logic is reported. **unknown**
- *Our check (name):* the decoded image has the format `%s-%s %s` next to
  `INSTAX`, `(ANDROID)` and `(IOS)`, plus `INSTAX-%s` / `BT_NAME:%s`. The
  advertised names are therefore **likely** `INSTAX-<id> (IOS)` (LE) and
  `INSTAX-<id> (ANDROID)` (Classic), with a **space** before the suffix, unlike
  the STM32 names in [firmware.md](firmware.md). The `<id>` content (the SQUARE
  app's Classic filter expects `instax-5…`, [connection.md](../apps/connection.md))
  is not established from the firmware.

Nothing in the package explains a BLE link drop on SQUARE. The STM32 evidence
in [firmware.md](firmware.md) (no MCU pairing timer, silent `0x90`/`0x91`
paths) does **not** carry over: this is a different code base.

## Agreement and disagreement with our docs

- **Agrees:** frame envelope, checksum rule, START/DATA field layout, START
  reply carries the block size (the client already reads it, see
  [PROTOCOL.md](../../protocol/PROTOCOL.md)). 800×800 matches `Format::Square`.
- **Corrects** [firmware-contents.md](firmware-contents.md): the resource file
  systems are now enumerated, and they are FAT12.
- **Narrows** [firmware.md](firmware.md)'s client note "every Link printer
  requires the 911-byte frame": that holds for the STM32 and RX tables only.
  For SQUARE no fixed frame length is reported. The block size comes from the
  START reply, which is what `crates/client/src/session.rs` already uses.
- **Open for the client:** START option checks, the block-size value, the
  transfer timer, final-block handling and `0103` support. See
  [README-implications.md](README-implications.md).
