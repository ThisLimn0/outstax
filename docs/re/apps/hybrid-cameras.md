# Hybrid cameras: mini Evo, mini Evo Cinema, mini LiPlay / LiPlay+, WIDE Evo

Static analysis on 2026-11-11 of three Android companion apps. The inputs are
recorded in [`data/acquisition/acquisition-hybrid.json`](../../../data/acquisition/acquisition-hybrid.json). These are app-side findings only.
No camera was connected and nothing here has been qualified on hardware.
Vendor source and assets are not copied. Class and method names below are
evidence anchors in the local JADX output, outside the project (`research/decompiled/{miniEvo,liplay,wideEvo}`).
Scratch tables (`sid-*.txt`, `opcode-diff.txt`, `emitted.json`) are outside the project,
in the research workspace (`research/work/hybrid`).

| App | Version | Bluetooth library | Obfuscation |
|---|---|---|---|
| `com.fujifilm.instaxBo19` (mini Evo app) | 2.0.5 (209) | `InstaxBluetoothLibrary-v4.6.7(6)` | Library names kept |
| `com.fujifilm.instaxminiLiPlay` | 7.2.0 (194) | `InstaxBluetoothLibrary-v13.0(31)` | Library names kept |
| `com.fujifilm.instaxWideEvo` | 1.1.3 (108) | Same library, R8-merged | Heavy: `G1.a` request, `Q1.e` names, `O1.*` tasks |

All three APKs have different signer certificates, and each differs from the
Link apps' signers. The certificate identities were read from the signing
blocks but not cryptographically verified.

## Devices and model identifiers

The model number is the text returned by `0001` selector 1. The apps gate every
feature on it, as the Link apps do.

| Model ID | Product | Library DeviceType | Android transport used by the app | Name filter (case-insensitive) |
|---|---|---|---|---|
| `FI019` | instax mini Evo | `BO19` | Classic SPP/RFCOMM (`BtcConnectionService`) | `instax` + `android` + (`instax-3` or `instax-cn-3`) |
| `FI041` | instax mini Evo Cinema (same app, `evocinema` package) | `BO41` | BLE GATT, plus Wi-Fi for media | `instax` + `BLE` + `instax-A1` (CN build: `instax-CN-A1`) |
| `HM1` | instax mini LiPlay | `DM1_GLOBAL` / `DM1_CHINA` | Classic SPP/RFCOMM | `instax` + `android` + (`instax-0` or `instax-8`), CN: `cn` |
| `FI044` | instax mini LiPlay+ (`liplay2` package) | `BO44` | BLE GATT | `instax` + `BLE` + `instax-A` (or `instax-cn-A`) |
| `FI028` | instax WIDE Evo | `BO28` | BLE GATT (`F1/f.java`, `Q1/a.java`) | `instax-9` |

Evidence:
- mini Evo app: `InstaxApplication.onCreate` builds `BtcConnectionService(..., BO19)` and `BleConnectionService(..., BO41)`.
- LiPlay app: `InstaxApplication` builds `BtcConnectionService(DM1_*)` and `BleConnectionService(BO44)`.
- Name tables: `checkValidDevice` in `ble/BleConnectionService` and `BtcConnectionService`.
- In JADX, the Classic check's second substring appears as the inlined constant `ECConstants.OPERATING_SYSTEM = "android"`.

The shared library's BLE name table also contains a `BO19` (`instax-3`) entry.
However, the mini Evo app never creates a BLE service for BO19. Whether a mini
Evo exposes the GATT service (iOS would need it) cannot be determined from
these Android apps.

The FI044 prefix `instax-A` is also a substring of FI041's `instax-A1`. Model
identity has to come from `0001/1`, not from the advertised name.

## 1. Transport and packet codec

**Same Link protocol, same packet codec.**
- **GATT:** service `70954782-…`, write `70954783-…`, notify `70954784-…`, CCCD `2902`. All three apps use exactly the bytes the Link apps use (`bluetooth/util/Constants`, and `Q1/a` in WIDE Evo).
- **Classic:** SPP UUID `00001101-0000-1000-8000-00805F9B34FB`, with secure/insecure socket and the channel-1 reflection fallback (`connection/a`). This is the same code the Link apps use.
- **Request builder:** `data/Request.build` (WIDE Evo `G1/a.b`) writes `"Ab"`, a u16 BE total length of `7+payload`, the mode byte, the type byte, the payload, then `255 - (sum & 255)`.
- **Response:** `"aB"`, the length, the opcode, then a result byte. Payload length is `length-8`.

This is exactly the envelope in [PROTOCOL.md](../../protocol/PROTOCOL.md) and `crates/protocol`. There is no
new framing, checksum, encryption or transaction ID. Result codes come from the
same `ResultCode` table. It includes `CAMERA_BUSY` (2), `POST_VIEW_PRINT_ERROR`
(0xc1) and `NOW_PRINTING` (0xc2), which the Rust `DeviceResult` already names.

The mini Evo app also ships a separate Wi-Fi stack (`com.fujifilm.wifi.*`, own
`Request`/`SID`), used only by `InstaxBo41WifiService` (Evo Cinema media
download). It was not analysed further here. BLE is used to bring it up:
`8b00` WIFI_STARTUP, `8b01` WIFI_SHUTDOWN, `0104` WIFI_INFO.

### Opcode diff against [`data/protocol/commands.json`](../../../data/protocol/commands.json) (74 entries)

The enum tables were reconstructed as follows:
- mini Evo and LiPlay: from `util/SID.java`, with constant-inlined values resolved.
- WIDE Evo: the ordinal-to-mode/type switch tables in `G1/a.b()`, joined with the name table `Q1/e.t(int)`.

"Emitted" means the app has a request class or a `new Request(SID.X)` for that
opcode. For WIDE Evo it means an ordinal passed to `G1.a`/`H1.*`/`J1.*`
constructors.

| | mini Evo app | LiPlay app | WIDE Evo app |
|---|---|---|---|
| Enum entries | 109 | 117 | 89 |
| Shared with commands.json | 74 (all) | 74 (all) | 74 (all) |
| Link-only (in commands.json, absent from enum) | none | none | none |
| Emitted by the app | 89 | 113 | 46 |

Every opcode in commands.json is present in every camera app's enum, so there
are no Link-only opcodes at enum level. Camera-only opcodes (absent from
commands.json) are listed below. Opcodes in **bold** are in all three apps.

| Range | Opcodes | Apps |
|---|---|---|
| Identity | `0011` TERMINAL_INFORMATION (len:u8, bytes) | mini Evo |
| Wi-Fi | `0104` WIFI_INFO (selector), `8b00/8b01` start/shutdown, `8b10/8b11` connect/disconnect (enum only) | mini Evo |
| Camera info | **`8010`** ADDITIONAL_CAMERA_INFO (body `00`), **`8011`** PARAMETER_RW, `8012` DATE_AND_TIME_SETTING, **`8015`** CURRENT_SHOOTING_PARAMETER, **`8017`** FAVORITE_SETTINGS | 8012 LiPlay |
| QR / URL | `8020` QR_INFORMATION, `8021` URL_UPLOAD, `8110` URL_UPLOAD_ADDRESS_END | mini Evo, LiPlay |
| Video URL | `8109`–`810e` URL_VIDEO_* / URL_QR_UPLOAD_CANCEL | mini Evo |
| Live view | `8203` LIVE_VIEW_CAMERA(_INFO) (name differs per app), `8211` LIVE_VIEW_SHOOTING_MODE, `8212` LIVE_VIEW_VIDEO_SHOOTING | 8211/8212 mini Evo |
| Frames | `8305` FRAME_INFORMATION | LiPlay |
| Logs | **`8409`–`840b`** CAMERA_LOG_RECORD_DATE_* | all |
| Camera command | **`8500`** CHECK_CAMERA_STATUS, **`8501`** EXECUTE_CAMERA_COMMAND | all |
| Sound | `8600`–`8608` SOUND_* | LiPlay |
| Auto upload | **`8800`–`8805`** IMAGE_AUTO_UPLOAD_INFO/START/DATA/END/CANCEL/COMPLETE | all |
| Sound log | `8a00`–`8a0c` SOUNDLOG_* | LiPlay |
| Activity | `8c00`–`8c02` GET_ACTIVITY_INFO/DATA, SET_ACTIVITY_DATA_DEFAULT | mini Evo |

That is 60 distinct camera-only opcodes across the three apps. The only naming
conflict is `8203` (mini Evo LIVE_VIEW_CAMERA, LiPlay LIVE_VIEW_CAMERA_INFO).

Opcodes in the enum that the app never emits:
- **WIDE Evo emits none of:** `0101 0102 1081 2003 20a0–20a2 20a6 20a7 20a9 20b0 20b1 3000–3020 8000 8001 8100–8108 8300–8304 8403–8408`. Its emitted set is `0000 0001 0002 0010 0100 0103 1000–1003 1080 2000–2002 2010 2080 2081 20a8 3080 3081 8010 8011 8015 8017 8200–8202 8210 8220–8222 8230 8400–8402 8409–840b 8500 8501 8800–8805`.
- **The mini Evo library never emits `0103` BLE_CONNECT**, although the opcode is in its enum.

## 2. Phone-to-camera print flow

### Common path (all five models)

The app tasks `InstaxInfoTask`, `SendPrintImageTask` and `PrintTask` are
textually the Link WIDE 2.0.0 tasks with minor edits. WIDE Evo has the same
flow, R8-merged into `O1/g.j()`.

1. **Handshake:** `0000`, then `0001` selectors 0..5, then `0002/0` image info, then `2010`.
   - FI041 and FI028 insert `0001` selectors 9/10 (BT_FW_VERSION, BT_OTA_FW_VERSION) after selector 5. FI033 does the same.
   - FI041 then reads `8011 [1c 00 00 00 00 00]` (auto-transfer flag in dt0).
   - SP-4 head-type logic does not apply.
2. **Gates** (`BaseTask.checkInstaxInfo`):
   - The function bitmap from `0000` must have bit 0 (print).
   - The model must be in the print allowlist. mini Evo lib: HM1, SP-4, BO-22, FI019, FI023, FI041. LiPlay lib adds FI017, FI033, FI028, FI044. WIDE Evo: the same list including FI028.
   - Device structure versions must not exceed the client ceilings below.
   - Rich mode needs image option bit 3. Double density needs option2 bit 0.
3. **Preflight:** battery `0002/1` (level & 7 in 1..3), then printer `0002/2` `isPrinterGood(false)`.
4. **BLE only:** `0103 [00 0c 00 0c 00 00 01 90]`. This depends on the library:
   - Sent by the LiPlay lib (`isBleConnection`) and by WIDE Evo (state 8 → `H1.d(9,3)`).
   - **Not sent** by the mini Evo lib v4.6.7, including for the BLE Evo Cinema.
5. **START, DATA and END:**
   - `1000` START body: `picture_type, option1, option2, byte3, jpeg_len:u32`.
   - Then `1001 seq:u32 + block`, with the block size taken from the START reply and the last block zero-padded. Each ACK echoes the sequence number.
   - Then `1002`. `1003` cancels.
6. **PRINT:** `0002/2` printer good, then empty `1080`. The reply's first byte is the estimated seconds.
7. **Completion:** poll `0002/2` every second. Result `c2` means keep polling. Done when the operation flag clears. Busy is tolerated and film=0 is allowed. This matches `Session::wait_for_print`.

The image-info layout is identical: `sel, w:u16, h:u16, type:u8, opt:u8, max:u32`.
- **mini Evo lib:** never reads option2.
- **LiPlay lib:** reads option2 only when the whole frame is 17 bytes. A well-formed image-info frame is at least 19 bytes, so this branch appears unreachable.
- **WIDE Evo:** reads option2 from a separately parsed field (`O1/c` line ~2350). The trigger length was not established.

The printer status, battery and error-bit layouts are the Link layouts.

### Per-model differences

| | mini Evo FI019 | Evo Cinema FI041 | LiPlay HM1 | LiPlay+ FI044 | WIDE Evo FI028 |
|---|---|---|---|---|---|
| Transport | SPP | BLE | SPP | BLE | BLE |
| `0103` before START | no | **no** (lib) | no | yes | yes |
| START `option1` | 0 or `08` (Rich, `Helper.getCurrentPrintOption` = 3D-LUT pref) | 0 or `08` (`printModeRich`) | always 0 (`PrintOption.NONE`) | 0 or `08` (`DirectPrintActivity`) | 0 or `08` |
| START `option2` | always 0 (field hard-coded) | 0 | 0 (default) | 0 (default) | 0 from the only caller (`p218w7/n`: option2 = NONE) |
| START byte 3 | 0 | 0 | `totalPrintNumber`, default 0 | **number of copies** (`numCopiesToPrint`, default 1) | 0 |
| Image | 600×800 JPEG | 600×800 | 600×800 | 600×800 | **1260×840** JPEG |
| JPEG size | see note 1 below | | | see note 2 below | see note 3 below |
| Post-print extra | – | QR print: `8021` URL_UPLOAD after completion (`ECPrintTask`) | – | Sound/QR print: `8021` after completion (`PrintTask.isSoundChekiImagePrint`) | – |
| Client version ceilings (device/image/battery/printer/history/camera/camera-history) | 0/2/0/0/0/5/0 | same app as mini Evo | 0/0/0/0/0/0/0 | same app as HM1 | 0/1/0/0/0/0/0 (`A6.i()`) |

JPEG size notes:
1. **mini Evo app (FI019, FI041):** `Compressor.compressImage(path, 60, dir, 600, 800)` scales to fit 600×800, then re-encodes from quality 100 down in steps of 5 until the file is ≤ 60 KiB. The result is then rejected if larger than the device's `max_bytes` (`Helper.isPrintImageSizeSupportedByCamera`).
2. **LiPlay app (HM1, FI044):** the same compressor. HM1 uses the 600×800 crop. FI044 uses `compressionPreference = 60` KiB by default.
3. **WIDE Evo (FI028):** constants `PRINT_IMAGE_WIDTH/HEIGHT = 1260/840` and `PRINT_IMAGE_SIZE_IN_KB = 203`. `Q2/Z2.a` returns the file unchanged if it is already 1260×840 and ≤ 207 872 bytes. Otherwise it scales and re-encodes from quality 100 down in steps of 5 until it fits.

Rich mode on the cameras uses the same 3D-LUT option bit (`PICINF_PICOP_3DLUT = 8`)
as the Link apps, applied on the host. This pass did not check whether each
camera app bundles the same `20210701_3DLUT.bin` (compare the SharedAssets hashes).
Other `PicturePrintOption` values present only in the enum:
- `0x10` 1_STPRT
- `0x20` FORCED_PRT
- `0x40` ONLY_ONE
- `0x80` NOSAVE

### QR / sound prints (Evo Cinema and LiPlay+ only)

1. Before printing, the app sends `8020` QR_INFORMATION (mini Evo lib `QrSettingInfoRequest`):
   `qr_x:u16, qr_y:u16, qr_color:u8, logo_display:u8, url_index:u8, url_total:u8, url_len:u8, url_bytes`.
2. After completion, the app sends `8021` and parses the returned URL address (`UrlUploadAddressForQRResponse` / `UrlUploadAddressResponse`).

A plain print never needs either packet.

## Camera-specific functions (not needed to print)

- **Remote shutter / live view** (`LiveViewTask`, FI019 gate = live-view function bit 1 + model HM1/FI019):
  1. `0002/1` battery, then the camera status query (`0002/4`).
  2. `8200 [00]` starts the view, and the app then polls `8201` for frames. A frame response carries `transfer_type:u8, frame_type:u8, len:u16, data`, reassembled until FrameEnd. CAMERA_BUSY means retry after 500 ms, at most 20 times.
  3. Optional `8000 [flash, timer, 0, 0]`.
  4. `8210 [00]` takes the picture.
  5. Download the post-view: `8220` (reply `pic_type:u8, size:u32, block:u32`), then `8221 seq:u32` (reply `seq:u32 + data`), then `8222`.
  6. `8230` POST_VIEW_PRINT, then `0002/2` polling as above.
  7. `8202 [00]` stops.
- **WIDE Evo and LiPlay-lib BO28 paths:**
  - Flash is set with `8011` instead of `8000`.
  - Status uses `8500` (empty).
  - Commands use `8501 [type, p0..p7]`: 5 = batch transfer un-transferred, 6 = favourite clear, 7 = notify action. The status reply is acceptable/not/processing.
- **Settings, lens and film effects:** `8011` PARAMETER_RW body `kind:u8, mode:u8, dt0..dt7`. This is the LiPlay lib layout. The mini Evo lib sends only 4 data bytes: `kind, mode, dt0, 0,0,0`. Modes: 0 = current, 1 = factory, 2 = set.

  | Kind | Setting |
  |---|---|
  | 0 | auto power-off |
  | 1 | start-up LED |
  | 2 | auto delete |
  | 3 | volume |
  | 4–6 | sound types |
  | 7, 8 | date print / order |
  | 9 | 3D-LUT print |
  | 10 | transfer format (mini V / mini H / square / wide) |
  | 11 | flash |
  | 12 | exposure |
  | 19 | lens effect (0–9) |
  | 20 | film effect (0–9) |
  | 21 | film style (0–5) |
  | 22 | lens-effect value |
  | 23 | multi-exposure |
  | 24 | macro |
  | 25 | lens position (normal/wide) |
  | 26 | white balance |
  | 27 | film-style LED |
  | 0x1c | FI041 auto transfer |

  - `8015 [01, mode]` reads the current shooting parameters: kind, flash, cine/still exposure, era dial, degree, frame, macro, WB, sound effect, video light, stabilisation (Evo Cinema).
  - `8001 [sel]` returns favourites.
  - `8017` stores favourites.
- **Camera-to-phone image transfer:**
  - **Auto upload** `8800`–`8805`: info returns `db_index:u32, cuts:u8`. Start `db_index:u32, cuts:u8` returns `size:u32, block:u32, printing_status, dial, degree, file_type`. Then data `seq:u32`, end, and complete `db_index:u32`.
  - **mini Evo URL upload** (printed images to phone, `URLUploadTask`): `8100` info returns `db_index:u32 …`. Then `8101 db:u32` / `8102 seq:u32` / `8103`, then `8107` address, then `8108` complete flags.
  - **Evo Cinema video** goes over the `810x` URL_VIDEO_* opcodes and Wi-Fi.
- **LiPlay extras:** frame set (`8300`–`8305`), sound/BGM (`86xx`, `8axx`), date/time `8012`.
- **Firmware:** `2000`–`2081` main-firmware flow as for the Link printers. Camera variants `FirmwareUpdateCameraTask` / `ECFirmwareUpdateCameraTask` were not traced here.

## 3. Verdict per camera

The Rust client today:
- **Transport:** BLE and TCP only, no RFCOMM.
- **Model gate:** `PrinterIdentity` bails on any model outside `LinkModel` (SP-4, FI023, FI033, FI046, FI017, BO-22, FI049).
- **Version check:** `validate_print` accepts image version ≤ 2, rejects other non-zero printer-structure versions, and ignores the camera versions.
- **BLE setup:** it sends `0103` on BLE.
- **START:** it passes all four START option bytes through (`download_start(len, [u8;4])`).

| Camera | Verdict |
|---|---|
| **WIDE Evo (FI028)** | **Model/capability gate only.** BLE, same packets, and the same order including `0103`. Image 1260×840 from `0002/0`. Add FI028 to the model enum, or skip the enum for capability-gated printing. `20a8` and selectors 9/10 are optional for printing. Respect the device `max_bytes`. The app itself caps at 207 872 bytes. |
| **mini LiPlay+ (FI044)** | **Model/capability gate only** for a plain print. Optionally set START byte 3 = copies. The app sends 1, while the Rust default of 0 is what other libs send and has not been checked on hardware. Sound/QR prints need `8020` before and `8021` after (layouts above). |
| **mini Evo Cinema (FI041)** | **Model gate, and possibly omit `0103`.** BLE and the same print packets. Its app never sends `0103`. Whether the camera accepts or rejects it is unknown, so make it per-model. QR print needs `8020`/`8021`. |
| **mini Evo (FI019)** | **Needs a different transport.** Same codec and print packets (no `0103`). The only app path is Classic SPP/RFCOMM (`00001101`, name `instax-3…`). Requires an RFCOMM transport plus the model gate. BLE availability on the device is unproven. |
| **mini LiPlay (HM1)** | **Needs a different transport.** Classic SPP only, same as FI019. Plain JPEG print with option1 = 0. The app's client image-info ceiling is 0, which the Rust check (≤ 2) tolerates. |

**WIDE Evo firmware cross-check** (external multi-firmware analysis,
2026-11-12, see [wide-evo.md](../firmware/wide-evo.md)). The Linux firmware does not
change the WIDE Evo verdict: its `libhost.so` names print-download, printer-capability,
image-info and reject-film-cover handlers and a BLE connect-interval handler
(`0103`), and it decodes and crop/scales the received JPEG before its own 3D LUT
(**likely**, symbol names plus traced calls). It adds one caveat: the camera has
an LE pairing database, numeric comparison and a pair-registration menu, so a
host may need to be registered on the camera first. Unknown until tested.

No camera needs a new print packet. The shared library's own print allowlist,
even inside the Link WIDE 2.0.0 and mini Link apps, already includes
HM1/FI019/FI028/FI044. The model gate is therefore an app-UI restriction, not
a protocol difference.

## Open items

- Confirm GATT presence on FI019 and HM1, and what an RFCOMM-only path costs on each OS.
- Check whether FI041 tolerates `0103`.
- Check the meaning of START byte 3 on non-LiPlay+ devices.
- Check the real `max_bytes` and option2 extension per camera.
- Find what triggers WIDE Evo's option2 parse.
- Check whether WIDE Evo accepts print commands from a host that is not
  paired/registered on the camera ([wide-evo.md](../firmware/wide-evo.md)).
- Compare the camera apps' LUT asset hashes with the Link LUT.
- Trace the Evo Cinema Wi-Fi protocol (`com.fujifilm.wifi`) if camera-to-phone media is in scope.
