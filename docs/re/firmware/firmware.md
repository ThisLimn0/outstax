# Printer firmware: dispatcher, upload, BLE, power, name

This is a static analysis, done on 2026-11-12, of the main-controller images in the
user-supplied firmware research bundle (`instax-firmware-research`). The bundle is kept
outside the project, in material/zips and material/firmware/bundle-2026-11-11, and
[firmware-contents.md](firmware-contents.md) summarises it. All files match its
SHA256SUMS. The image SHA-256s are in [`data/firmware/firmware-dispatch.json`](../../../data/firmware/firmware-dispatch.json) and
[`data/firmware/firmware-assets.json`](../../../data/firmware/firmware-assets.json). Nothing was executed or flashed, and no printer was connected.
Firmware bytes and long disassembly are not copied here. Addresses are Thumb
code/data addresses in the image loaded at `0x08020000`, given as evidence anchors.

Labels: **recovered** means read directly from a table or handler, **likely** means
inferred from code structure, and **unknown** means not established.

| Image (asset) | Model | Model ID string | Command table |
|---|---|---|---|
| `fw/LINK3/STMicro/0102.hex` | mini Link 3 | `FI033` | `0x080364d0` |
| `Firmware/BO22/0101.hex` | Link WIDE | `FI022` / `BO-22` | `0x0802d928` |
| `fw/LINKPLUS/STMicro/0100.hex` | mini Link+ | `FI046` | `0x080313c8` |
| `fw/BO23/STMicro/0100.hex` | mini Link 2 | `FI023` | `0x0802d0e8` |
| `fw/BOD/0104.hex` | mini Link | `SP-4` | `0x0802cb14` |

The images below are not analysed here. The external multi-firmware analysis
(2026-11-12) has since covered them, and this document stays STM32-only:
- The two Renesas RX images: [renesas-mini2-mini3.md](renesas-mini2-mini3.md).
  **Agrees:** the RX descriptor tables match the STM32 tables of the same model
  entry for entry (opcodes, lengths, flags), and START/DATA/END and the
  firmware block sizes agree. The BLE, timer and power findings below were
  **not** re-derived for RX.
- WIDE 2 `BO49/F100.DAT`: [wide2.md](wide2.md). Confirmed as one raw LZ4 block →
  SUNP container, MIPS32 code, plus an appended Realtek Bluetooth package.
  **Differs:** no fixed 911-byte DATA frame (block size from the START reply),
  and DATA checks the expected sequence.
- SQUARE `FW_0102.DAT`: [square.md](square.md). The same SUNP/MIPS32 family as
  WIDE 2. The START reply carries the block size, and `0103` support is **unknown**.
- WIDE Evo: [wide-evo.md](wide-evo.md) (§6 below is superseded).
Client consequences: [README-implications.md](README-implications.md).

Tools and scratch output are outside the project, in the research workspace (`research/work/firmware/scripts`):
- `hex2bin.py` converts HEX to a flat binary.
- `sweep.py` does a linear Thumb sweep with literal-pool annotation.
- `optable.py` extracts the command table.
- `namechar.py` and `wdog.py` pull the name character and the watchdog constants.
- `dispatch_json.py` and `facts.py` generate [`data/firmware/firmware-dispatch.json`](../../../data/firmware/firmware-dispatch.json).
- `lz4raw.py` is a pure-Python raw LZ4 decoder with a self-check.

## 3. BLE link: architecture, pairing, 0x0103 (the live problem)

**Architecture (recovered).**
- Link 3 and Link+ use an external Realtek RTL8753BFR module. It talks to the STM32 over UART.
  - Evidence: debug strings `HCI_EVT_LE_CONNECTION_COMPLETE`, `LE_DATA_SEND`, `MP Loader`, `new baudrate`.
  - The module runs the BLE/BR-EDR stack. The STM32 receives framed events (byte 0 = `02`, u16 LE event id, u16 LE length) in the switch at `0x0806cc22`:

| Event | Meaning (from strings/handler) |
|---|---|
| `0x22` | GATT write received (`HCI_USER_EVT_WRITE_RX`) → BLE channel buffer `0x200007c4` |
| `0x50` | SPP data → SPP channel buffer `0x200007dc` |
| `0x52` / `0x51` | SPP connected / disconnected |
| `0x83` / `0x84` | LE connected / disconnected (the disconnect payload byte, likely the reason, is ignored) |
| `0x8b` | 8 bytes, stored as current interval/latency/timeout at `0x200005dc+0xe4..0xec` (likely the LE parameter-update event) |

- The BT task's state machine is at `0x08020980`. States: 0 init, 1 idle, 2 BLE connected, 3 SPP connected, 4 both, 5 fault.
- The first link that delivers data claims the session: `0x200005dc+1` holds 1 for BLE or 2 for SPP (`0x0806a134` / `0x0806a178`).
- WIDE, Link 2 and mini Link have the same BT-task code (same message ids, for example `0x0804701a`) but no debug strings. Their module is **unknown**.

**Pairing/bonding (recovered for the MCU side).**
- The STM32 has no pairing, bond or encryption logic. The LE-connected handler (`0x08069d96`) copies the peer address, increments a connection counter, moves to state 2 and sends module command `0xbe`.
- No security state is checked before commands are dispatched. The frame parser (`0x08065920`) and the dispatcher (`0x08065da2`) only check signature, length, checksum, the table and two global flags (below).
- The STM32 arms no timer on connect that later disconnects the link. The only BT-side timer is a 300-tick (`0x12c`) module-command response timeout with 3 retries. When the retries run out the BT task goes to the fault state (`0x0806e50c..0x0806e6ac`).
- Whether the **module** requests security, or drops unencrypted links, is **unknown**:
  - The APP component (`APP_RTL8753BFR_10018…bin`) is encrypted or compressed from offset `0x3000` (entropy about 7.7).
  - Its readable parameter block holds only defaults: names `RTL8753BFR` / `RTL8753BFR LE`, `com.rtk.datapath`, `com.realtek.EADemo2`, `RTK AUDIO`, plus an `e49b83…` record.
  - The CFG component is mostly `0xff`. The `.rcfg` is a section index (`APP_`, `SYS_`, `OTA_`, …).
  - None of them has a decodable pairing-mode, IO-capability or supervision-timeout field without Realtek's schema.
  - So the WIDE 2 app's 28 s "must be bonded" rule cannot be confirmed or refuted from these files.
- The advertising data contains a Google Fast Pair–style service-data record:
  - Link 3 `0x080f8390`: `06 16 2c fe e4 9b 83`, model `e49b83`.
  - Link+: model `ebd394`.
  - The selection flag is `0x200005dc+0x84/0x85`.
  - This suggests (**likely**) that the module supports a bonding flow, but nothing on the STM32 requires it.

**Silent no-response paths (recovered), relevant to "link up but no reply".**
- The dispatcher drops a command without replying when either flag is set:
  - `0x2001e054+0x20` is set after any reply with result `0x90` LOW_POWER or `0x91` BATTERY_NOT_MOUNTED (`0x080661f2`). It is never cleared on that path, so after a low-battery reply the printer stops answering until it is reset.
  - `0x2001d0a8+8` (meaning **unknown**) (`0x08065dd4`, `0x08065df0`).
- A handler's internal code `0x13` (for example a payload-length mismatch) also produces no reply (`0x08065e98`).
- The client sees each of these as its 15 s response timeout, not as a disconnect.

**0x0103 BLE_CONNECT (recovered, same code in all five images).**
- The handler (Link 3 `0x08096478`, WIDE `0x08070c08`, Link+ `0x08090b98`) reads four BE u16 values.
- The check at `0x08096518` returns `0x81` PARAMETER_ERROR unless:
  - 6 ≤ min ≤ 3200
  - 6 ≤ max ≤ 3200
  - max ≥ min
  - latency < 500
  - 10 ≤ timeout ≤ 3200
- It does **not** enforce the Core-spec rule timeout > (1+latency)·max·2. The app values 12, 12, 0, 400 pass.
- Result 0 is returned at once. It does not wait for the radio.
- The four values are forwarded (`0x08069d5e`, BT message `0x28`):
  - With a BLE link up (states 2/4, `0x0806a250`), they are stored and sent to the module as UART command `0xba` with 8 bytes.
  - With only SPP up (states 1/3, `0x0806a242`), they are dropped.
- The module's own acceptance is **unknown**. The resulting parameters come back in event `0x8b`.
- Practical consequence: 15 ms / 4 s is merely requested. If the central (Windows) refuses or ignores the request, the printer does not notice.

**Mid-transfer timeout (recovered).** See question 2: the printer never disconnects on its own when an upload stalls. It silently discards the upload state after about 59 s on BLE (Link 3/Link+/Link 2/mini Link). A BLE drop mid-upload therefore originates in the radio/module or the host, not in the STM32 upload logic.

## 5. Device name (recovered)

The advertised name is built at boot from the serial number. No protocol command writes it.

| Image | LE name | Classic name | Builder (LE / EDR) |
|---|---|---|---|
| mini Link | `INSTAX-1<serial>(IOS)` | `INSTAX-1<serial>(ANDROID)` | `0x0804821c` / `0x0804813c` |
| WIDE | `INSTAX-2<serial>(IOS)` | `INSTAX-2<serial>(ANDROID)` | `0x08030a28` / `0x08030944` |
| Link 2 | `INSTAX-4<serial>(IOS)` | `INSTAX-4<serial>(ANDROID)` | `0x0804918c` / `0x080490b4` |
| Link 3 | `INSTAX-7<serial>(BLE)` | `INSTAX-7<serial>(EDR)` | `0x08059d10` / `0x08059c44` |
| Link+ | `INSTAX-A<serial>(BLE)` | `INSTAX-A<serial>(EDR)` | `0x08038d0c` / `0x08038c3c` |

How the name is built:
- Prefix, suffix and their lengths come from the flash parameter block. Link 3:
  - templates `0x080f8350` (`INSTAX-`,`(EDR)`) and `0x080f8360` (`INSTAX-`,`(BLE)`)
  - lengths `07 05 07 05` at `0x080f843c`
- The model character is a literal (Link 3 `0x08059f30` = `7`).
- `<serial>` is 7 characters from a 20-byte factory record:
  - It is read from non-volatile address `0x2000` into `0x2001d8e4` at `0x08057dfa`. The characters start at +6.
  - It is validated as `[0-9A-Za-z_#-]`. If invalid, it is replaced by the default `0000000` from `0x080f8188` (`0x0805868a`).
- The LE name is placed in scan-response AD type `0x09` (`0x0806e1fe`, buffer `0x200007a4`).
- `0x0010` IDENTIFY selector 0 returns the 8-character `<model char><serial>` (`0x2001d97f`).
- `0x0001` returns `FUJIFILM` and the model ID (`0x0806f654..0x0806f682`).

No command writes the name:
- None of the 28–43 table entries writes the name or the serial.
- `3020` accepts parameter ids 0–3 only (`0x080839b4`).
- A factory/maintenance command table follows the main table (Link 3 `0x080367c0`, ids `0x00be`, `0x2b`–`0x72`). It uses a different framing that was not traced. Whether it can rewrite the factory record is **unknown**. It is not reachable through the Link protocol frames.

This confirms the app-side finding: there is no rename.

Corrections to [connection.md](../apps/connection.md):
- The older models' BLE name ends in `(IOS)`, not `(BLE)`.
- Their Classic name really ends in `(ANDROID)`. The apps' `android` name filter is a literal match, not a decompiler artifact.

Advertising data (recovered):
- flags `0x05`
- the complete 128-bit service UUID `70954782-2d83-473d-9e5f-81e1d02d5273`
- TX power (6 dBm on Link 3/Link+, 0 on the others)
- manufacturer data with company `0x04d8` and a model byte: 01 mini Link, 02 WIDE, 04 Link 2, 07 Link 3, 0b Link+.

## 1. Command dispatcher (recovered)

The frame parser and dispatcher:
- The parser (Link 3 `0x08065920`) scans a 20 800-byte (`0x5140`) ring per channel.
- It matches `0x4162` ("Ab"), takes the length (it must be < `0x5140`) and the opcode, then walks the 16-byte table entries:
  - `opcode:u16`
  - `frame_length:u16`
  - `any_length:u8`
  - `f5:u8`
  - `f6:u8`
  - pad
  - `handler:u32`
  - `continuation:u32`
- `any_length=0` requires an exact frame length. The checksum is accumulated and the low byte must be `0xff`.
- The dispatcher (`0x08065da2`) calls the handler, or the continuation on later passes (`0x08065e86`) for long-running commands. The reply is built at `0x08066128`.
- `f6=1` marks transfer commands whose success re-arms the upload watchdog (`0x0806623e`). `f5` is copied to a context byte (meaning **unknown**).
- `1001`, `2001` and `20a1` appear twice in each table with identical contents.

Full per-image lists, with handler addresses, are in [`data/firmware/firmware-dispatch.json`](../../../data/firmware/firmware-dispatch.json). Summary:

| Opcode group | mini Link | Link 2 | WIDE | Link 3 | Link+ |
|---|---|---|---|---|---|
| `0000 0001 0002 0010` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `0100 0101 0102 0103` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `1000–1003 1080 1081` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `1010 1011 1012` (not in commands.json) | ✓ | ✓ | ✓ | ✓ | ✓ |
| `2000–2002 2010 2080 2081` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `20a0–20a8 20b0 20b1` (`20a3–20a5` not in commands.json) | – | – | – | ✓ | ✓ |
| `3000` | ✓ | ✓ | – | ✓ | ✓ |
| `3001 3002` | `3001`,`3002` | ✓ | `3001` | ✓ | ✓ |
| `3003 3004 3005 3010` (`3005` not in commands.json) | – | ✓ | ✓ | ✓ | ✓ |
| `3006 3007` (`3007` not in commands.json) | – | ✓ | – | – | – |
| `3008 3020` | – | – | – | ✓ | ✓ |
| `3080 3081` | ✓ | – | – | – | – |

No image handles:
- `2003` FW_UPGRADE_EXIT and `20a9` BT_FW_PROGRAM_EXIT. This agrees with the apps, where both are enum-only.
- Any `8xxx` camera opcode.

An unknown opcode gets no table match. Whether it gets `0x80` or is silently dropped was not traced (**unknown**).

Further notes:
- `1010` (empty), `1011` (4-byte body) and `1012` (empty) open, feed and close a second transfer state (states 3/4 in `0x080662b8..0x080662ca`, cleared by `1012`). Their purpose is **unknown**. No app was found to send them.
- Cross-check: `3080`/`3081` exist only in the original mini Link image. This matches the app, which uses head correction only for mini Link firmware ≥ 103.
- Opcode lengths agree with commands.json. `0102` takes 12 body bytes, `0103` and `1000` take 8, `20a0` takes 12, `3004` takes 16 and `3020` takes 10. `3001`/`3003` take any length.

## 2. Image upload

| | mini Link | Link 2 | WIDE | Link 3 | Link+ |
|---|---|---|---|---|---|
| `0002/0` reply (recovered) | 600×800, opt `00`, 153 600 | 600×800, `7b`, 153 600 | 1260×840, `7b`, 337 920 | 600×800, `7b`, 153 600, option2 0 | 600×800, `4b`, 307 200, option2 1 |
| START length limit | 153 600 | 153 600 | 337 920 | 153 600 | 153 600, or 307 200 with option2 bit 0 |
| START validator | `0x0805b988` | `0x0805da6c` | `0x0805bbc0` | `0x08080e64` | `0x0807b5fc` |
| Print block size | 900 | 900 | 900 | 900 | 900 |
| FW block size (`2001`) | 1024 | 1024 | 1024 | 900 | 900 |

START (`1000`) checks (recovered):
- `picture_type` must be 2.
- `option1`:
  - The low 3 bits (mini Link: low nibble) must be < 4.
  - `0x80` is rejected everywhere except on mini Link.
  - Link+ also rejects `0x10` and `0x20`.
  - mini Link requires the high nibble to be one of 0/0x10/0x20/0x40/0x80. A low nibble of 8 is rejected, so mini Link rejects the LUT flag `0x08`.
  - The allowed bits match each image's advertised `options` byte.
- `option2`:
  - Link 3 requires 0.
  - Link+ allows only bit 0.
  - WIDE, Link 2 and mini Link read it into a scratch byte and ignore it.
- `byte3` is ignored by every image (scratch byte, for example Link 3 `0x20021954`). LiPlay+'s "copies" meaning does not apply to the Link printers.
- Length 0 is rejected. A START while a previous upload is still in state 0–2 is accepted as a restart. In any other state the reply is `0x82`.
- Failure codes: `0x81` PARAMETER_ERROR for a bad parameter, `0x82` SEQUENCE_ERROR for a bad state, plus the battery/temperature precheck `0x806bd96` (`0x90`–`0x93`).
- The block size returned is fixed at 900. It is not negotiated per transport (`0x08080dfa`).

DATA (`1001`, Link 3 `0x08080ef4`, recovered):
- The frame must be exactly 911 bytes: sequence plus 900 bytes. This is a fixed table length, so a shorter final block is not accepted and the last block must be padded.
- `seq` greater than the last block index → `0x81`.
- The upload state must be started or data → otherwise `0x82`.
- An expired watchdog → `0x85` TIMEOUT.
- There is **no** "next expected sequence" check. Any in-range block is copied to `seq·900`, so a duplicated block is simply overwritten.
- The ACK echoes `seq`.

END (`1002`, `0x0808104c`) requires that both the first and the last block were seen (`0x82` otherwise) and a live watchdog (`0x85`).

Mid-transfer timeout (recovered):
- Each successful `1000`/`1001` reloads a countdown (`0x08066244`):
  - mini Link, Link 2, Link 3, Link+: 5910 ticks on BLE and 4925 on SPP (table `+8`/`+6`, Link 3 at `0x080f7ee4`).
  - WIDE: 13 032 / 10 860 (`0x080fb5fc`).
- The countdown runs in the T10MS task (`0x080960e4` → `0x08065f4a`). That gives about 59 s (BLE) / 49 s on Link 3 and about 130 s / 109 s on WIDE. The 10 ms period is **likely**, from the task name.
- On expiry the upload state is cleared and an internal event `0xfd85` is posted (`0x08065fc0`). Further DATA/END replies are `0x85`.
- The printer does not sleep or disconnect because of it.
- Separately, a partial frame's inter-byte counter saturates at 350 ticks (about 3.5 s, `0x08065f50`). Its consumer was not traced (**unknown**).

## 4. Power / auto sleep

`0102` (Link 3 `0x08080af8`, recovered):
- Body: mode, 3 ignored bytes, then 4 BE u16 timers. Mode ≥ 6 → `0x81`.

| Mode | Action | Reply timers from |
|---|---|---|
| 0 | read | saved settings `0x2001da40` (NV-backed) |
| 1 | read | session values `0x2001db88` |
| 2 | read | factory defaults (`0x080f82d0`) |
| 3 | extend: restart the countdown only (`0x08061284`) | session values |
| 4 | set the session values, then restart | session values |
| 5 | set, save to NV (two copies at `0x7000`/`0x8000`, `0x08060268`), then restart | session values |

- Modes 4/5 require each timer to be 0 (never) or ≥ 1500 (`0x08080ca0`).
- Factory values are 3000 ×4 on Link 3, Link+, Link 2 and WIDE, and 6000 ×4 on mini Link (`0x080fcbb8`).
- Selection `0x08061f16`:
  - Timer 1 or 3 is used normally, timer 2 or 4 when two power-state flags are set (`0x2001d01c` +0 and +6). The pair is chosen by `0x2001d0a8+0xb3c`. The meaning of these flags (charging or connected) is **unknown**.
  - The chosen value ×985/1000 is counted down every 10 calls of the 10 ms power tick (`0x080613d0`). So the units are about 0.1 s (**likely**): 3000 ≈ 5 min, the minimum 1500 ≈ 2.5 min, mini Link 6000 ≈ 10 min.
  - 0 means never.

What resets the timer:
- Mode 3/4/5 (recovered).
- Four key/event callbacks registered at `0x08061a4e`/`0x08062116`/`0x0806214e`, functions `0x0808e4bc`/`0x0808e680` (**likely** button presses).
- Print-time loads with fixed values of 5/10/30 (`0x0806148e..0x080615a0`).
- Ordinary commands (status polls, image DATA) do not reset it: there is no call from the dispatcher or the 1000/1001 handlers (recovered).
- This is why the WIDE app sends mode 3 every few seconds. A host that only polls status lets the printer power off after the timer.

## 6. Camera print path (WIDE Evo FW_0103 unsigned)

**Superseded** by [wide-evo.md](wide-evo.md): the external analysis decoded the
128-byte prefix plus dependent LZ4 block stream, extracted the UBIFS file
systems and traced `libhost.so` → `/dev/ab_printer`. The original note:
- The file is not raw LZ4. It is a Novatek NA51055 (Cortex-A9) Linux image (device-tree strings, `UBI#` and `hsqs` markers) with compressed payloads.
- The Link 16-byte dispatch table signature does not occur in the raw bytes.
- Only fragments such as `INSTAX-%` are visible.
- Comparing it would need UBI/squashfs extraction, which was not done here.

## Implications for the Rust client (for the coordinator)

- Send exactly 900-byte padded DATA blocks. Every STM32 image (and, per the external analysis, both Renesas RX images) requires the 911-byte frame regardless of the START reply. SQUARE and WIDE 2 (SUNP) take the block size from the START reply instead ([square.md](square.md), [wide2.md](wide2.md)).
- On BLE drops: the printer firmware gives no evidence of an STM32-side pairing timeout. If the drop happens at a fixed time after connect, the module or Windows is the cause. Pairing in Windows first remains the test to run.
- A `0x90`/`0x91` reply means later commands will go unanswered.
- Use `0102` mode 3 as the keep-alive between jobs, as the WIDE app does.
- `0103` succeeds on all five images. The Rust client need not special-case older models. SQUARE is not one of them, and its `0103` support is unknown ([README-implications.md](README-implications.md) item 1).
