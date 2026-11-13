# Protocol enum closure, initialization and head calibration

Static analysis of Mini Link 7.0.6, SQUARE Link 1.6.2 and Link WIDE 2.0.0 JADX
output, 2026-11-12. No app was executed and no printer was contacted. Class and
method names are the obfuscated names of those builds. The machine-readable
table is [`data/protocol/enum-closure.json`](../../../data/protocol/enum-closure.json), with one row per opcode and extra rows for
selectors and modes.

## Method

- **Wide** keeps descriptive names. The app code (`com.fujifilm.instaxwide.*`)
  uses only `InstaxWideService`. `InstaxBodService`, `InstaxBo19Service` and
  `InstaxDm1Service` have no references from app code. The tasks that only
  those services construct are dead in this app: AxisInfo, DeviceSettings,
  LiveView, URLUpload, FrameSet, CameraLog and FirmwareUpdateCamera. So is
  `CameraInfoTask`, because `InstaxService.getCameraInfo` has no app caller.
- **Mini** keeps the SID enum names (`W1.D`). Every constant was searched as
  `D.NAME` outside the enum. Builders are in `P1`/`Q1`, tasks in `T1`/`U1`/`V1`,
  the service in `L1.m`/`L1.n` and the connection orchestrator in `X3.G`.
- **Square** is R8-merged. Its SIDs are 1-based ordinals (names in `W1.i.v`)
  passed to the request base `L1.b(int)` or to `super(int)` in `T1.a`…`T1.h`.
  All constructions were listed. Emitted ordinals: 2,3,4,5,6,8,10,11,12,13,14,
  16,17,18,20,21,22,24,27,29,30,31,32. Tasks are `W1.*`, the orchestrator is
  `h2.g` and the service is `Q1.g`. JADX lost the byte values of the `X1.a`
  enum. A fallback-mode decompile of that one class recovered them:
  NO_ACTION=0, RE_PRINT=1, RESERVED=2.
- **Status rule:** `called` means at least one app emits the request to a model
  that app actually operates. When a builder exists but is gated to a model the
  app never connects, or sits in a state that is never entered, the per-app
  status is `never-called` with a note.

## Counts

Opcodes from [`data/protocol/commands.json`](../../../data/protocol/commands.json): 74. **36 called, 38 never-called, 0
unknown.** With the selector and mode sub-rows the table has 78 rows.

Never-called by all three apps:

- `0101` RESET.
- `1081` REJECT_FILM_COVER: Wide `PrintImageRequest.rejectFilmCoverRequest` has no caller.
- `2003` FW_UPGRADE_EXIT, `20a6`, `20a9`.
- All 33 `8xxx` camera opcodes. Builders exist only in the Wide library, behind
  unreachable services. Mini and Square have no builders.
- `8304` (no builder anywhere).

Never-called selectors and modes:

- `0001` selectors 6–8.
- `0002` selectors 4–5.
- `0010` selectors 0, 2, 3.
- `0102` modes 1 and 4 (provisional get/set).
- `3020` read modes 0/1, and parameters 1, 2, 4.

The `3020` read-response layout therefore stays undetermined. No app reads it,
and none should be invented.

## Corrections to the existing record

1. **`3020` is Link 3 (FI033) only, not Link+.** `L1.m.C` passes the common
   gate (SP-4/FI023/FI033/FI046) and then rejects every model except FI033 with
   a local NOT_SUPPORTED, so no packet is sent. Link+ UI callers
   (`LinkPlusCameraActivity.G3`, `X3.G.B0`) hit that local rejection.
   `U1.c` builds only `Q1.e(ParameterCommand)`, so the mode is always 2 (SET).
   The parameters actually sent are:
   - 0 MODE_SWITCHING with dt0 = 0 normal, 1 air-studio/photo booth, or
     2 instax-camera.
   - 3 SHOOTING_LIGHTING with dt0 = 0 (three white lights on).
2. **`3008` FUNCTION_BUTTON_SETTING is called by Square.** `W1.d` sends
   `[02, 01]` (set RE_PRINT) on every connect. Mini and Wide never send it.
3. **`0100/00` abort.** Mini `T1.i` and Wide `FirmwareUpdateTask` main-firmware
   tasks never send it. It is sent by:
   - Mini Link 3 BT firmware (`U1.a` state BT_ABORT_FW_UPGRADE, not on
     connection loss).
   - Square's merged firmware task `W1.c.h()`, on cancel and on error state.
4. **Head calibration is transport-gated.** Mini `T1.x` enters
   GET_PRINTER_HEAD_LIGHT_CORRECT_INFO only when `!isBle`. SP-4 and FI023 use
   Classic SPP, while Link+ and Link 3 use BLE. In Wide 2.0.0 that state is never
   assigned (PRINTER_STATUS goes straight to IMAGE_START), so it is dead. In
   Square the path exists but is gated to SP-4, which that app never operates.
5. `3000` and `3002` are Mini-only in practice. Wide has the library tasks but
   no reachable caller, and Square never constructs them.
6. Square never sends `0103`. It is Classic-only.
7. The Wide power-on LED (`3004`) UI is shown only for INSTAX_LINK_WIDE1 (BO-22).

## Post-connect sequence per model

Errors are handled the same way in all three orchestrators (Wide
`InstaxBluetoothService.handleResponseWithNextAction`, Mini `X3.G.g0`, Square
`h2.g.a`):

- OK: go to the next action.
- Connection lost: reconnect after 1 s.
- INSTAX_ERROR NOW_PRINTING (C2): run the next action after 10 s.
- Any other failure: run the next action after 100 ms. A failed step does not
  stop the chain.

One exception: in Mini GET_PRINTER_INFO1 (`X3.G.j`), a non-OK result other
than CONCURRENT_REQUEST stops the chain.

### Common identity task

| App | Order and gates |
|---|---|
| Mini `T1.n` | `0000` → `0001` 0..5 → FI033: `0001` 9, 10 → `0002/0` (reads option2) → `2010` if model ∈ {SP-4, FI019, BO-22, FI023, FI017, FI033, FI046} → FI033: `20a8` → SP-4: `0010/1` → done |
| Square `W1.a.i` | `0000` → `0001` 0..5 → `0002/0` → `2010` if model ∈ {SP-4, FI019, BO-22, FI023, FI017} → SP-4: `0010/1` → done |
| Wide `InstaxInfoTask` | `0000` → `0001` 0..5 → `0002/0` → `2010` if model ∈ {SP-4, FI019, BO-22, FI049} → done |

Every result other than OK ends the task with INVALID_RESPONSE or INSTAX_ERROR,
including `0010`. No app checks the manufacturer string, so Rust's FUJIFILM check
is stricter than the app. Print readiness is checked later
(`checkInstaxInfo`): print-function bit 0 plus a model allowlist.

### Mini (X3.G.P)

1. INIT_INSTAX (`T1.n`, above).
2. GET_PRINTER_INFO1 (`T1.v`): `0002/2`, `0002/3`, `0002/1`.
3. Link+ and Link 3 only: SET_SHOOTING_LIGHT, `3020 [03 02 00…]`. In effect
   this is sent to FI033 only (see correction 1).
4. GET_PRINTER_COLOR: `3010/0` (voltage, temperature), then `3010/1` (print
   attempts, battery type, colour, film-PI, CPU). Gated to FI023/FI033/FI046,
   so SP-4 skips it.
5. SET_AXIS_CONFIG: `3002 [02, b | b<<1]`. `b` is one app preference
   (`h().b()`, logged as "accelerationLinkage"). All Mini models get it.
6. SET_COLOR: `3001`, only on SP-4/FI023 and only when a UI pattern is cached.
7. GET_PRINTER_INFO2, then a polling loop:
   - normal mode: GET_AXIS_INFO, `3000` about 20 times.
   - AR mode: GET_AR_INFO, `3006 [en, intensity, 0, 0]` (FI023 only).

   The loop then returns to printer info.

Mini never extends auto-sleep (`0102` mode 3 is not used).

### Square (h2.g.e)

INIT_INSTAX → SET_FUNCTION_BTN_SETTING `3008 [02 01]` → GET_PRINTER_COLOR
`3010/0`, `3010/1` → GET_AUTO_SLEEP `0102` mode 0 (auto-sleep is off if
timer1 = 0) → GET_PRINTER_INFO `0002/2,3,1` → EXTEND_AUTOSLEEP `0102` mode 3 →
back to printer info. Most tasks are gated to FI017 (`Q1.g.g`).

### Wide (InstaxBluetoothService.doAction)

INIT_INSTAX → GET_PRINTER_COLOR `3010/0`, `3010/1` (BO-22/FI049) →
GET_AUTO_SLEEP `0102` mode 0 → GET_PRINTER_INFO → EXTEND_AUTOSLEEP `0102`
mode 3, sent 2 s after each poll. Extend is skipped while the app is in the
background or auto-sleep is off. If `fwProgramNumber` equals the stored
just-updated number, the app posts FwUpdateCompleteEvent.

## Head identification and calibration (resolved)

**Head identification** (`T1.n`, SP-4 only):

- Request `0010` with selector 1 (`W1.k.LIGHT_PEN_S_OR_N`).
- Reply (`R1.m`): `selector:u8, len:i8, text[len]`, decoded with the default
  charset and **not trimmed**.
- Rule: if the text starts with `AD` (`Fa.o.y`, startsWith), or its length is
  not 14, the head is FUTABA. Otherwise it is TOHOKU.
- Uses of the result:
  1. A UI advisory (`ui.a.u`): SP-4 with firmware digits below 103 and a FUTABA
     head opens a prompt (`P0` → `T1` → `R1`). The prompt text was not traced.
  2. Diagnostics. The calibration decision itself uses the `3080` reply.

**Calibration** (`T1.x`), after a successful `0002/2` readiness check, before
START:

1. Gate: not BLE, model SP-4, and digits of FW revision ≥ 103. The app keeps
   only the digits (regex `[^0-9]` removed) and calls `Integer.parseInt`, which
   would throw if no digits remain. If the gate fails, go to START.
2. Send `3080`. The reply must be at least 21 bytes:
   `head:u8, judge:u8, -, year:u16, month, day, r:u16, g:u16, b:u16`. Any
   result other than OK aborts the print with INSTAX_ERROR.
3. If head = FUTABA (0) and judge = 1, compute the
   `PrinterHeadLightCorrect.funcCalc` brim and send `3081 r,g,b`. Otherwise go
   to START.
4. A `3081` OK goes to START. Any other result aborts.

`HeadCalibration::parse/correction` and `set_head_correction` in
`protocol/settings.rs` already match the wire layout and algorithm. The session
integration (P1) has since been implemented in `client/src/session.rs`.

## Settings flows

- **Auto-sleep** (`0102`, reply ≥ 16 bytes, body = four u16 timers):
  - read: mode 0.
  - disable: mode 5 with zero timers.
  - enable: mode 2 read, then mode 5 with the timers just read.
  - keep-alive (Wide/Square only): mode 3 with zero timers.
- **Power LED** (`3004`): read with mode 1, write with mode 5 and the value at
  byte 4. The reply is ≥ 20 bytes, and body[0] is the current value. Wide values:
  0 rainbow, 1 warm, 2 cool, 3 white. Gates: Wide UI BO-22, Mini FI023/FI033,
  Square FI017.
- **Function button** (`3008`, Square): request `[mode_byte, action]`. Mode int
  1/2/3 is sent as byte 0/1/2, and the app only uses 3, i.e. byte 2. The reply
  is ≥ 9 bytes. Its body[0] is 0 NO_ACTION or 1 RE_PRINT, and anything else is
  RESERVED.
- **AR / axis** (Mini): the `3006` reply body[0] is the `SwIntelligence`
  switch byte. `3000` is parsed by the existing `Axes`.
- **3010/1 CPU byte**: Mini `R1.a` reads it (`W1.u`: 0 ST_MICRO, 1 RENESAS),
  but Wide does not.

## Implementation proposals (for the coordinator)

These proposals only describe the work. All vectors are complete frames,
checksums included.

**P1: head ID and calibration (Mini SP-4).**

- `device.rs`: add `pub enum HeadType { Futaba, Tohoku }` with
  `fn from_light_pen(raw: &[u8]) -> Self`, implemented as
  `raw.starts_with(b"AD") || raw.len() != 14`, using the raw, untrimmed bytes.
- Add `pub fn identify(selector: u8) -> Frame` (`Command::IDENTIFY`).
- Add `pub fn parse_identify(&Frame) -> Result<(u8, Vec<u8>)>`, validating
  `len`.
- Add `pub fn firmware_digits(&str) -> Option<u32>`.
- `PrinterIdentity` gets `head: Option<HeadType>`. `Session::identity` sends
  `0010/1` after the program number when the model is `LinkModel::Mini`. A
  non-OK reply fails identity, as in the app.
- `Session::upload`: after the pre-transfer status check, when
  `!transport.is_ble() && model == Mini && firmware_digits >= 103`, run
  `3080` → `HeadCalibration::parse` → `correction(today)` → if `Some`, `3081`
  and expect OK. Abort before START on any failure.
- The Rust client is currently BLE-only. Following the app means the
  calibration is skipped over BLE. Keep the gate exact and don't force-calibrate.
- Test vectors:
  - request `0010/1`: `41 62 00 08 00 10 01 43`.
  - reply `61 42 00 18 00 10 00 01 0e 41 44 31 32 33 34 35 36 37 38 39 30 31 32 30`
    (text "AD123456789012") → Futaba.
  - the same length without "AD" → Tohoku, and length 13 → Futaba.
  - `3080`: `41 62 00 07 30 80 a5`.
  - `3081` with rgb 30/31/32: `41 62 00 0d 30 81 00 1e 00 1f 00 20 41`.

**P2: `3020` gate and helpers.**

- Allow `settings::parameter` only for `LinkModel::Mini3`, always with
  `ParameterMode::Set`.
- Add typed helpers `mode_switch(Normal = 0 | AirStudio = 1 | InstaxCamera = 2)`
  and `shooting_light(ThreeWhiteOn = 0 | Off2s = 1)`.
- Mark Current/Factory as never sent by the app, and fix the `settings.rs`
  comment that says "Link+".
- Connect step for Mini3: `shooting_light(ThreeWhiteOn)`. The reply is the
  result byte only.
- Test vectors:
  - shooting light: `41 62 00 11 30 20 03 02 00 00 00 00 00 00 00 00 f6`.
  - normal: `… 00 02 00 … f9`.
  - booth: `… 00 02 01 … f8`.
  - camera: `… 00 02 02 … f7`.

**P3: `3008` function button (Square FI017).**

- Add `pub enum ButtonAction { NoAction = 0, Reprint = 1, Reserved }`.
- Add `pub fn function_button_set(action) -> Frame`, payload `[02, action]`.
- Add `pub fn parse_function_button(&Frame) -> Result<ButtonAction>`, requiring
  a frame of at least 9 bytes.
- Connect step for Square after identity.
- Test vectors: request `41 62 00 09 30 08 02 01 18`, reply
  `61 42 00 09 30 08 00 01 1a` → Reprint.

**P4: `3004` reply parser.**

- Add `parse_power_led(&Frame) -> Result<u8>`: total frame length ≥ 20, return
  body[0].
- Model gates: Wide1 UI, Mini2/Mini3, Square.
- Test vectors:
  - read: `41 62 00 17 30 04 01 00×15 10`.
  - write 3: `41 62 00 17 30 04 05 00 00 00 03 00×11 09`.
  - reply `61 42 00 14 30 04 00 02 00×11 12` → 2.

**P5: auto-sleep recipes.**

- Add `sleep_disable()` (mode 5, zero timers).
- Add `sleep_enable()` (mode 2 read, then mode 5 with those timers).
- Add `sleep_extend()` (mode 3, zeros). Use it as the Wide/Square keep-alive
  during long sessions.
- Treat timer1 = 0 as disabled.
- Test vectors:
  - mode 0: `41 62 00 13 01 02 00 00×11 46`.
  - mode 3: `… 03 … 43`.
  - mode 5 off: `… 05 … 41`.
  - reply `61 42 00 10 01 02 00 00 b4 00 00 00 00 00 00 95` → timers
    [180, 0, 0, 0].

**P6: model initialization.**

- Add `Session::initialize(&PrinterIdentity)`. It runs the per-model post-identity
  steps above, once, without the polling loop:
  - Mini: printer info → Mini3 `3020` shooting light → Mini2/3/Plus `3010/0,1`
    → `3002 [02, flags]`.
  - Square: `3008` → `3010/0,1` → `0102/0`.
  - Wide: `3010/0,1` → `0102/0`.
- Record the CPU byte from `3010/1`, because firmware selection needs it.
- Unlike the app, surface step failures. Simply continuing silently is
  unnecessary for a CLI.
- `3000` polling and `3006` are remote-shutter/AR UI features. Add them only
  with that UI.
- Test vectors: `3010/0` `41 62 00 08 30 10 00 14`, `3010/1` `… 01 13`,
  `3002` with both bits set `41 62 00 09 30 02 02 03 1c`.

**P7: do not implement** `0101`, `1081`, `2003`, `20a6`, `20a9`, `0102` modes
1/4, `3020` reads, `0002/4-5`, any `8xxx`. No app exercises them, so their
responses are unknown. Keep them only as enum names for logging.

**P8: firmware abort semantics.**

- Main-controller updater: never send `0100` for Mini or Wide models. The
  Square app sends `0100 [00]` on error or cancel.
- Link 3 BT updater: send `0100 [00]` on any non-connection error.
- Frame: `41 62 00 08 01 00 00 53`.

## Remaining limits

- The `3006` `SwIntelligence` bit meanings beyond those in `Axes` were not
  re-derived.
- The Mini power-LED value names and the SP-4 advisory dialog text were not
  traced.
- The `X1.a` byte values come from a single-class fallback decompile.
- None of this is hardware-verified.
