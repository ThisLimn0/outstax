# Unused commands: investigation backlog

One entry per opcode, selector or mode that [`data/protocol/enum-closure.json`](../../data/protocol/enum-closure.json) marks
`never-called` in all three Link apps (Mini 7.0.6, SQUARE 1.6.2, WIDE 2.0.0).
The rule and the evidence are in [`docs/re/apps/protocol-closure.md`](../re/apps/protocol-closure.md). This is a
backlog for later work. The Rust client implements none of these IDs (P7), and
none of them was sent to a printer for this document.

Coverage: the 38 never-called opcodes, the 4 selector/mode rows, and one extra
row for the unused `0010` selectors, which the JSON records as a note on a
called opcode. That makes 43 entries.

## Sources for the firmware column

- [`data/firmware/firmware-dispatch.json`](../../data/firmware/firmware-dispatch.json): the static dispatch tables of five STM32
  main-controller images: original mini Link SP-4 `0104`, Link 2 FI023 `0100`,
  Link 3 FI033 `0102`, Link+ FI046 `0100` and WIDE BO-22 `0101`. "Handler" means
  that the table has an entry, with its handler address and the exact frame
  length it accepts. [`docs/re/firmware/firmware.md`](../re/firmware/firmware.md) did not
  exist when this was written, so the handler semantics behind the addresses are
  **pending** unless another source is cited.
- **External multi-firmware analysis, 2026-11-12**:
  - Renesas RX descriptor tables for mini Link 2 (BO23 `0100`) and mini Link 3
    (LINK3 `0102`) in [`data/firmware/renesas-mini2.json`](../../data/firmware/renesas-mini2.json) and [`renesas-mini3.json`](../../data/firmware/renesas-mini3.json)
    ([renesas-mini2-mini3.md](../re/firmware/renesas-mini2-mini3.md)). Same opcode sets, lengths and flags as the
    STM32 build of the same model. "RX" below gives the RX handler address.
  - SQUARE `FW_0102.DAT` and WIDE 2 `F100.DAT` (Sunplus/MIPS): no descriptor
    table. Only group switches and handler names are reported
    ([square.md](../re/firmware/square.md), [wide2.md](../re/firmware/wide2.md)). "SUNP" below is **likely** at best.
- **External mini Link 0104 analysis**: an independent static analysis of the
  original mini Link firmware `0104` (its command reference and its
  settings/status/update reports). It covers SP-4 only.

## Risk legend

| Mark | Meaning |
|---|---|
| **SAFE-READ** | Read-only query, and the handler shape is known. A single probe is reasonable. |
| **READ?** | Probably a read, but the handler has not been traced. Wait for [`docs/re/firmware/firmware.md`](../re/firmware/firmware.md). |
| **WRITE** | Changes a runtime or persistent setting. |
| **PHYSICAL** | Moves hardware, resets or powers the device, or uses film. |
| **FIRMWARE** | Firmware-update state machine. Can leave the device in an update or bootloader state. |
| **CAMERA** | Hybrid-camera opcode. No Link printer image handles it. |

## Counts by category

| Category | Entries | IDs |
|---|---:|---|
| System | 1 | `0101` |
| Print path | 1 | `1081` |
| Firmware-update exits | 3 | `2003`, `20a6`, `20a9` |
| Camera (`8xxx`) | 33 | `8000 8001 8100–8108 8200–8202 8210 8220–8222 8230 8300–8304 8400–8408` |
| Selectors/modes of called opcodes | 5 | `0001/6-8`, `0002/4-5`, `0010/0,2,3`, `0102/1,4`, `3020` reads and params 1/2/4 |
| **Total** | **43** | |

By risk: SAFE-READ 4 (`0001/6-8`, `0002/4-5`, `0010/0,2,3`, `0102/1`).
READ? 1 (`3020` read modes). WRITE 2 (`0102/4`, `3020` params 1/2/4).
PHYSICAL 2 (`0101`, `1081`). FIRMWARE 3 (`2003`, `20a6`, `20a9`). CAMERA 33.
Within those, `8230` prints, `8402/8405/8408` clear logs, and
`8000/8300–8303` write camera state.

## Worth investigating first

1. **`0010` selectors 0/2/3** (SAFE-READ). Every analysed image has a handler.
   The external analysis gives exact reply shapes for SP-4. It could identify
   the Bluetooth address and the runtime identity without any write.
2. **`0102` mode 1** (SAFE-READ). On Link 3 it reads the session timers. Paired
   with mode 0, it settles the timer units (Link 3 dispatch notes: about 0.1 s)
   and the difference between session and saved values.
3. **`3020` read modes on Link 3/Link+** (READ?). Both images have a 17-byte
   handler, and the app never reads it, so the layout is open. Trace the handler
   (`0x08083910` Link 3, `0x0807df90` Link+) before any probe.
4. **`0001/6-8` and `0002/4-5`** (SAFE-READ). Cheap probes that confirm the
   per-model reply (empty strings and `0x81` respectively on SP-4).

Do not probe `0101`, `1081`, `2003`, `20a6`, `20a9`, `0102/4` or the `3020`
writes on a printer someone depends on. Trace them statically first.

## Opcodes

### `0101` RESET (PHYSICAL)

- **Names:** Mini `W1.D.RESET`, Square SID ordinal 7 (`W1.i.v`), Wide `SID.RESET`.
  It is in all three enums.
- **Builder/constant:** none. The enum constant appears only in the SID enum
  and the parse switch in all three apps.
- **Payload hint:** none in the apps. Firmware: an 8-byte frame, so a 1-byte body.
  External mini Link 0104 analysis: `mode:u8`, 0 or 1 (mode 1 maps to an
  internal mode 4). No reply payload.
- **Firmware handler:** present in all five images: SP-4 `0x0805b534`, Link 2
  `0x0805d60c`, Link 3 `0x08080a04` (continuation `0x08080ad8`), Link+
  `0x0807b19c`, WIDE `0x0805b760`. External mini Link 0104 analysis: a
  **software reboot**. The reply is sent first, then a Cortex-M AIRCR system
  reset follows. It is rejected while another operation runs (except `0300`).
  RX: BO23 `0xfff57673` (continuation `0xfff5772f`), LINK3 `0xfff6aafe`
  (continuation `0xfff6abba`). SUNP: not reported.
- **Related:** `0100` SHUT_DOWN (sent as the firmware abort `0100 [00]`).
- **How to investigate:** read the handler and continuation in [`docs/re/firmware/firmware.md`](../re/firmware/firmware.md)
  for the other models. If ever probed, expect the link to drop right after
  the OK reply. No app capture is possible because no app sends it.

### `1081` REJECT_FILM_COVER (PHYSICAL)

- **Names:** Mini `W1.D.REJECT_FILM_COVER`, Square SID ordinal 15, Wide
  `SID.REJECT_FILM_COVER`. It is in all three enums.
- **Builder/constant:** Wide `PrintImageRequest.rejectFilmCoverRequest` (empty
  payload) has no caller. Mini and Square have only the constant.
- **Payload hint:** empty request (7-byte frame in every firmware table).
  External mini Link 0104 analysis: no reply payload at first acceptance.
- **Firmware handler:** present in all five images (SP-4 `0x0805c1a0`, Link 2
  `0x0805e288`, Link 3 `0x080816ac` with continuation `0x0808180e`, Link+
  `0x0807be44`, WIDE `0x0805c3e0`). External mini Link 0104 analysis: a **film
  reject/eject** operation. It submits an internal operation and logs
  "RejectEnd" in its continuation. The initial OK reply means only that the
  command was accepted. RX: BO23 `0xfff5872f` (continuation `0xfff588e3`),
  LINK3 `0xfff6bc29` (continuation `0xfff6bddd`). SUNP: SQUARE and WIDE 2 both
  name a `FFProtocolCmd_PrintImage_RejectFilmCover` handler (**likely**).
- **Related:** `1080` PRINT, `1003` CANCEL, `0002/2` status (operation and
  print-result fields).
- **How to investigate:** static only, through the guard conditions in the handler.
  A probe would eject film (it uses up an exposure), so use an empty or spent
  pack on a sacrificial unit.

### `2003` FW_UPGRADE_EXIT (FIRMWARE)

- **Names:** Mini `W1.D.FW_UPGRADE_EXIT`, Square SID ordinal 19, Wide
  `SID.FW_UPGRADE_EXIT`. It is in all three enums.
- **Builder/constant:** none in any app (enum and parse only).
- **Payload hint:** none.
- **Firmware handler:** **no table entry** in any of the five images
  (`in_commands_json_not_handled`). External mini Link 0104 analysis: not in
  its 28-command table either. RX: no entry in either table. **Disagreement
  (SUNP):** the SQUARE firmware-download switch compares subtypes
  `0, 1, 2, 3, 0x10, 0x80, 0x81`, so SQUARE **likely** handles `2003`, unlike
  every STM32 and RX image. Its behaviour is unknown.
- **Related:** `2000`–`2002` main firmware START/DATA/END, `2080` FW_DATA_BACKUP,
  `2081` FW_UPDATE_REQUEST.
- **How to investigate:** check whether the unhandled-opcode path of the
  dispatcher returns `0x80` SID_NOT_SUPPORTED ([`docs/re/firmware/firmware.md`](../re/firmware/firmware.md)). For
  SQUARE, trace subtype 3 of the firmware-download switch (`0x5710d0` area,
  [square.md](../re/firmware/square.md)). Do not probe during an update.

### `20a6` BT_FW_UPGRADE_EXIT (FIRMWARE)

- **Names:** Mini `W1.D.BT_FW_UPGRADE_EXIT` only. It is not in the Square or Wide enums.
- **Builder/constant:** none (enum and parse only).
- **Payload hint:** firmware tables: a 7-byte frame (empty body). The table
  entry marks it as a transfer command that re-arms the upload watchdog.
- **Firmware handler:** Link 3 `0x080820d0` and Link+ `0x0807c868`. Not present
  on SP-4, Link 2 or WIDE, which have no BT-module updater. RX: LINK3
  `0xfff9632c`, not in BO23. SUNP: not reported.
- **Related:** `20a0`–`20a2` BT firmware START/DATA/END, `20a7`, `20a8` BT
  program number, `0100 [00]` BT abort (sent by Mini `U1.a`).
- **How to investigate:** trace the handler. It probably leaves the
  BT-update state without committing. Static only.

### `20a9` BT_FW_PROGRAM_EXIT (FIRMWARE)

- **Names:** Mini `W1.D.BT_FW_PROGRAM_EXIT` only.
- **Builder/constant:** none.
- **Payload hint:** none.
- **Firmware handler:** **no table entry** in any analysed image, Link 3 and
  Link+ included, and none in either RX table (LINK3 has `20a0`–`20a8`,
  `20b0`, `20b1` only). SUNP: not reported.
- **Related:** `20a8` BT_FW_PROGRAM_INFO, `20b0`/`20b1`.
- **How to investigate:** RX is now checked (no entry). Confirm that it is a
  leftover enum value. Static only.

### Camera opcodes `8000`–`8408` (CAMERA)

Common facts for all 33:

- **Names:** the same names in Mini `W1.D` and Wide `SID`. In Square they are
  SID ordinals ≥ 33 in `W1.i.v`. All three enums contain them.
- **Callers:** builders exist only in the Wide library
  (`com.fujifilm.bluetooth.data.request.*`), used by tasks that only
  `InstaxBo19Service` and `InstaxDm1Service` construct. No Wide app code
  references those services. Mini and Square have only the constants.
- **Firmware handler:** **no table entry** in any of the five Link printer images
  or the two RX tables. This is expected, because these are camera functions.
  SQUARE and WIDE 2 contain camera-family switch names (Liveview, URL upload,
  Frame picture, Camera log), but the external analysis treats them as generic
  framework strings, not as reachable printer features. WIDE Evo has named
  `HceBluetooth_*` handlers for live view, uploads and logs
  ([wide-evo.md](../re/firmware/wide-evo.md)).
- **Hybrid-camera apps:** [`docs/re/apps/hybrid-cameras.md`](../re/apps/hybrid-cameras.md) §1. WIDE Evo emits
  `8200–8202 8210 8220–8222 8230 8400–8402`. It does not emit
  `8000 8001 8100–8108 8300–8304 8403–8408` (check mini Evo and LiPlay there).
- **How to investigate:** capture with the official hybrid-camera app against
  the camera model (WIDE Evo, mini Evo, LiPlay). On a Link printer a probe can
  show only the unhandled-opcode reply. It is pointless and not recommended.

Payload hints are paraphrased from the Wide library builders and response
parsers. Field names come from their getters, and the order follows the parser.

| Opcode | Name | Wide builder (class) | Request payload | Response hint (parser) | Task | Extra risk |
|---|---|---|---|---|---|---|
| `8000` | CAMERA_SETTINGS | `CameraSettingsRequest(flash, timer)` | `flash:u8, timer:u8, 0, 0` | result only (not traced) | LiveViewTask | WRITE |
| `8001` | CAMERA_SETTINGS_GET | `CameraSettingsGetRequest(type)` | `type:u8` | `CameraSettingsGetResponse`: 8 single bytes (type, favourite-experienced flag, …) | CameraLogTask | |
| `8100` | URL_UPLOAD_INFO | `UrlUploadRequest` | empty | `UrlUploadInfoResponse`: flag byte (image/audio/address/print-image available), 20-byte file name, `db_index:u32` | URLUploadTask | |
| `8101` | URL_PICTURE_UPLOAD_START | `UrlPictureUploadStartRequest(dbIndex)` | `db_index:u32` | `ImageReceiveStartResponse`: `picture_type:u8, picture_size:u32, frame_size:u32` | URLUploadTask | |
| `8102` | URL_PICTURE_UPLOAD | `UrlPictureUploadDataRequest(seq)` | `seq:u32` | `ImageReceiveDataResponse`: `seq:u32, data` | URLUploadTask | |
| `8103` | URL_PICTURE_UPLOAD_END | `UrlUploadRequest` | empty | result only | URLUploadTask | |
| `8104` | URL_AUDIO_UPLOAD_START | `UrlAudioUploadStartRequest(dbIndex)` | `db_index:u32` | `AudioReceiveStartResponse`: `audio_type:u8, audio_size:u32, frame_size:u32` | URLUploadTask | |
| `8105` | URL_AUDIO_UPLOAD | `UrlAudioUploadDataRequest(seq)` | `seq:u32` | seq-indexed data (parser not traced) | URLUploadTask | |
| `8106` | URL_AUDIO_UPLOAD_END | `UrlUploadRequest` | empty | result only | URLUploadTask | |
| `8107` | URL_UPLOAD_ADDRESS | `UrlAddressDataRequest(dbIndex)` | `db_index:u32` | `UrlUploadAddressResponse`: `len:u8, address[len]` | URLUploadTask | |
| `8108` | URL_UPLOAD_DATA_COMPLETE | `UrlUploadCompleteRequest(dbIndex, 4 booleans)` | `db_index:u32, flags:u8` (bit order not traced) | result only | URLUploadTask | WRITE |
| `8200` | LIVE_VIEW_START | `LiveViewStartRequest` | `00` | result only | LiveViewTask | |
| `8201` | LIVE_VIEW_RECEIVE | `LiveViewRequest` | empty | `LiveViewDataResponse`: `transfer_type:u8, frame_type:u8, len:u16, data` | LiveViewTask | |
| `8202` | LIVE_VIEW_STOP | `LiveViewStopRequest` | `00` | result only | LiveViewTask | |
| `8210` | LIVE_VIEW_TAKE_PICTURE | `LiveViewTakePictureRequest` | `00` | result only | LiveViewTask | PHYSICAL (shutter) |
| `8220` | POST_VIEW_UPLOAD_START | `LiveViewRequest` | empty | `ImageReceiveStartResponse`: `pic_type:u8, size:u32, block:u32` | LiveViewTask | |
| `8221` | POST_VIEW_UPLOAD | `LiveViewImageDownloadDataRequest(seq)` | `seq:u32` | `seq:u32, data` | LiveViewTask | |
| `8222` | POST_VIEW_UPLOAD_END | `LiveViewRequest` | empty | result only | LiveViewTask | |
| `8230` | POST_VIEW_PRINT | `LiveViewRequest` | empty | result only, then `0002/2` polling | LiveViewTask | PHYSICAL (uses film) |
| `8300` | FRAME_PICTURE_DOWNLOAD_START | `FrameSetStartRequest(pictureType, frameNo, frameType)` | `pic_type:u8, frame_no:u8, frame_type:u8, 0, size:u32` | result (not traced) | FrameSetTask | WRITE |
| `8301` | FRAME_PICTURE_DOWNLOAD | `FrameSetDataRequest(seq, data)` | `seq:u32, data` | not traced | FrameSetTask | WRITE |
| `8302` | FRAME_PICTURE_DOWNLOAD_END | `FrameSetRequest` | empty | result only | FrameSetTask | WRITE |
| `8303` | FRAME_PICTURE_NAME_SETTING | `FrameSetNameRequest(frameNo, name)` | `frame_no:u8, len:u8, name[len]` | result only | FrameSetTask | WRITE |
| `8304` | FRAME_PICTURE_NAME_GET | **none in any app** | unknown | unknown | – | |
| `8400` | CAMERA_LOG_SUBTOTAL_START | `DeviceLogRequest` | empty | `SubTotalLogStartResponse`: three `u32` (data size, frame size, profile info, mapping not traced) | CameraLogTask | |
| `8401` | CAMERA_LOG_SUBTOTAL_DATA | `SubTotalLogDataRequest(seq)` | `seq:u32` | `SubTotalLogDataResponse`: `seq:u32, data` | CameraLogTask | |
| `8402` | CAMERA_LOG_SUBTOTAL_CLEAR | `DeviceLogRequest` | empty | result only | CameraLogTask | WRITE (clears log) |
| `8403` | CAMERA_LOG_DATE_START | `DateLogStartRequest(type)` | `type:u8` | `DateLogStartResponse`: `log_type:u8` + three `u32` (count, data size, frame size) | CameraLogTask | |
| `8404` | CAMERA_LOG_DATE_DATA | `DateLogDataRequest(type, seq)` | `type:u8, seq:u32` | `DateLogDataResponse`: `log_type:u8, seq:u32, content` | CameraLogTask | |
| `8405` | CAMERA_LOG_DATE_CLEAR | `DateLogClearRequest(type)` | `type:u8` | `DateLogClearResponse`: one byte | CameraLogTask | WRITE (clears log) |
| `8406` | CAMERA_LOG_FILTER_START | `FilterLogStartRequest(type)` | `type:u8` | `FilterLogStartResponse`: `type:u8` + three `u32` | CameraLogTask | |
| `8407` | CAMERA_LOG_FILTER_DATA | `FilterLogDataRequest(type, seq)` | `type:u8, seq:u32` | `FilterLogDataResponse`: `type:u8, seq:u32, content` | CameraLogTask | |
| `8408` | CAMERA_LOG_FILTER_CLEAR | `FilterLogClearRequest(type)` | `type:u8` | `FilterLogClearResponse` | CameraLogTask | WRITE (clears log) |

## Selectors and modes of called opcodes

### `0001` selectors 6–8 (SYSTEM_ID, REGULATORY_DATA, PNP_ID): SAFE-READ

- **Names:** Mini `EnumC1888g.SYSTEM_ID/REGULATORY_DATA/PNP_ID`. The Square and Wide
  enums also contain them.
- **Builder/constant:** the existing `0001` builders (Mini `P1.g`, Square `T1.a(i,1)`,
  Wide `InstaxInfoTask`) are only ever called with 0..5 (Mini FI033: also 9 and 10).
- **Payload hint:** the same as 0..5. The request is `selector:u8` and the reply is
  `selector, len, bytes[len]`.
- **Firmware handler:** the `0001` handler is in all five images. External mini
  Link 0104 analysis: SP-4 accepts selectors 0..8, and 6–8 return zero-length
  fields in that build. RX: BO23 `0xfff56d05`, LINK3 `0xfff6a070` (selector
  range not reported).
- **Related:** `0001` 0..5, 9, 10 (`Session::identity`).
- **How to investigate:** one read per selector per model. Expect empty strings
  on SP-4, and check Link 3 for content.

### `0002` selectors 4–5 (CAMERA_FUNCTION_INFO, CAMERA_HISTORY_INFO): SAFE-READ

- **Names:** Mini `F.CAMERA_FUNCTION_INFO/CAMERA_HISTORY_INFO` and the Wide camera
  helpers. Square has the enum values.
- **Builder/constant:** Wide `task.CameraInfoTask` (`InstaxService.getCameraInfo`
  has no app caller) and a BaseTask camera helper reachable only from the camera
  services. Square `T1.a(i,3)` is used only with 1..4.
- **Payload hint:** request `selector:u8`. The camera status layout is in
  [`docs/re/apps/hybrid-cameras.md`](../re/apps/hybrid-cameras.md) (`0002/4` before live view).
- **Firmware handler:** the `0002` handler is in all five images. External mini
  Link 0104 analysis: SP-4 accepts only 0..3, so 4–5 return `0x81`. RX: BO23
  `0xfff56f18`, LINK3 `0xfff6a362` (selector range not reported).
- **Related:** `0002/0-3` (`ImageSupport`, `Battery`, `PrinterStatus`,
  `PrintHistory`) and the `0000` camera/camera-history version bytes.
- **How to investigate:** a single read on each printer model (expect `0x81`).
  Capture on a hybrid camera with its own app for the real layout.

### `0010` selectors 0, 2, 3 (SERIAL_NO_OR_SSID, BRIGHTNESS_DATA, BT_ADDRESS_OR_MAC_ADDRESS): SAFE-READ

- **Names:** Mini `W1.k` (selector 1 is LIGHT_PEN_S_OR_N, which is used). Square
  has the opcode but sends only selector 1. Wide has no `0010` in its enum.
- **Builder/constant:** Mini `P1.l(W1.k)` and Square `T1.a()`, both called only
  with selector 1 for SP-4. Rust: `outstax_protocol::identify(selector)` exists for
  selector 1, and `Session::identity` sends only 1.
- **Payload hint:** reply `selector:u8, len:u8, bytes[len]` (Mini `R1.m`).
- **Firmware handler:** the `0010` handler is in all five images. External mini
  Link 0104 analysis (SP-4): selectors 0..3 accepted. 0 returns 8 bytes of
  runtime identity. 2 returns comma-separated numeric fields (8 or 14 bytes).
  3 asks the Bluetooth layer and returns 6 bytes in reverse order (an address
  candidate, though the report does not assert MAC). Some selectors fall back to zeros
  when an inhibit flag is set. Other selectors return `0x81`. RX: BO23
  `0xfff5725f`, LINK3 `0xfff6a6ea` (selector range not reported).
- **Related:** `0010/1` head identification (P1).
- **How to investigate:** one read per selector, SP-4 first (where the layouts are
  known), then the other models. Do not treat the fields as ASCII.

### `0102` modes 1 and 4 (GET/SET_PROVITIONAL): mode 1 SAFE-READ, mode 4 WRITE

- **Names:** Mini `EnumC1884c` GET/SET_PROVITIONAL and Wide
  `AutoSleepSettingsMode.GET/SET_PROVITIONAL_SLEEP_SETTING`. Square `V1.a(2)`/`V1.a(5)`
  are never constructed. Rust: `SleepMode::Provisional`/`SetProvisional`, kept as
  names and documented as never sent.
- **Builder/constant:** the existing `0102` builders take the mode as a parameter.
  No app passes 1 or 4.
- **Payload hint:** the same frame as the other modes:
  `mode:u8, 0,0,0, four u16`. The reply body is four `u16`.
- **Firmware handler:** the `0102` handler is in all five images (19-byte exact frame).
  Link 3 dispatch notes: 1 reads the session timers, and 4 sets the session timers
  and restarts the countdown. Modes 4/5 need each timer to be 0 or ≥ 1500.
  Units are about 0.1 s. External mini Link 0104 analysis: mode 1 reads the
  active runtime structure, and mode 4 copies the supplied structure to runtime,
  then refreshes. The validator applies to every mode, so a mode-1 read also
  needs valid timer words (zeros pass). RX: BO23 `0xfff5774f`, LINK3
  `0xfff6abda` (19-byte frame), modes not reported. SUNP: an app-driven
  auto-sleep setting exists by name only (**likely**).
- **Related:** `0102` modes 0/2/3/5 (P5 helpers).
- **How to investigate:** a mode-1 read with zero timers, compared with mode 0. Mode 4
  is non-persistent, but it changes when the printer powers off. Test it only
  with values just read.

### `3020` read modes 0/1 and parameters 1, 2, 4: reads READ?, params WRITE

- **Names:** Mini `W1.x` parameters `PRINTER_POWER_MODE` (1), `LED_BRIGHTNESS_SETTINGS`
  (2), `OTHER` (4), and modes CURRENT (0) and FACTORY (1). Not in the Square or Wide enums.
- **Builder/constant:** Mini `Q1.e(x, A, int)` constructor (read/explicit-mode
  form) is never called. Only `Q1.e(ParameterCommand)` is used, with SET.
  `ParameterCommand.ledBrightnessModeSetting` has no caller. Rust:
  `settings::parameter` is now SET-only and Link 3-only (P2), and the CLI rejects
  parameters 1, 2 and 4.
- **Payload hint:** request `parameter:u8, mode:u8, dt0..dt7`. No app parses a read
  response, so its layout is **undetermined**.
- **Firmware handler:** Link 3 `0x08083910` and Link+ `0x0807df90` (17-byte exact
  frame). Not present on SP-4, Link 2 or WIDE. Dispatch notes for Link 3: the
  handler accepts parameter ids 0..3 only, so 4 (OTHER) is rejected. Link+
  firmware has a handler even though the app rejects Link+ locally. RX: LINK3
  `0xfff7a699` (17-byte frame), not in BO23.
- **Related:** `3020` SET params 0 (mode switch) and 3 (shooting light), and `3004`
  power LED.
- **How to investigate:** trace the read branch of the handler before any probe.
  Then do one mode-0 read of parameter 3 on Link 3. Parameter 1 (printer power) and
  2 (LED brightness) writes are not safe to try blind.

## Related IDs outside the never-called set

These IDs are not in the never-called list, but they are close to it, and the
firmware sources describe them.

| ID | Status in apps | Firmware handler | Notes |
|---|---|---|---|
| `1010`–`1012` | not in [`data/protocol/commands.json`](../../data/protocol/commands.json) or the Link app enums | all five images (`handled_not_in_commands_json`) and both RX tables | External mini Link 0104 analysis: **image readback**. `1010` setup reply `format, flags, 0:u16, size:u32, block:u32`. `1011 seq:u32` reply `seq:u32, data[900]`. `1012` finalizes. Read-only in effect, but it depends on the protocol phase and an image already uploaded. |
| `2080`/`2081` | called (firmware update, all three apps) | all five images | External mini Link 0104 analysis: `2080` stages and read-back-verifies the external NOR copy, and `2081` replies and then hands off to the prior update stage. FIRMWARE. Never probe. |
| `20a3`–`20a5` | not in [`data/protocol/commands.json`](../../data/protocol/commands.json) | Link 3, Link+, LINK3 RX | BT-updater group, pending [`docs/re/firmware/firmware.md`](../re/firmware/firmware.md). FIRMWARE. |
| `3005`, `3007` | not in [`data/protocol/commands.json`](../../data/protocol/commands.json) | `3005`: Link 2, Link 3, Link+, WIDE, both RX. `3007`: Link 2, BO23 RX | Pending [`docs/re/firmware/firmware.md`](../re/firmware/firmware.md). |
