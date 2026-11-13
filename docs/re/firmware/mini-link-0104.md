# Original mini Link firmware 0104: external analysis merged

Source: external analysis, 2026-11-12, package `instax-mini1-firmware-re`
(kept outside the project at `research/external/instax-mini1-firmware-re`).
Its entry points are `report/mini1/client-reference.md` and
`report/mini1/continuation-de.md` (German). It also contains a standalone
crate, `rust/instax-mini1`, and a synthetic upload plan,
`examples/mini1-upload-plan`.

The input was `assets/fw/BOD/0104.hex` from the Mini Link app 7.0.6:
- HEX SHA-256 `f3d3e5d2…91b0f7`. This is the same file as the mini Link row in
  [firmware.md](firmware.md) and `data/firmware/firmware-assets.json`.
- Reconstructed BIN SHA-256 `b04e3490…efcda4c`, base `0x08020000`.
- Both hashes were re-checked against `material/firmware/mini1-0104` on
  2026-11-12.

The analysis is static. The external package executed, emulated and flashed
nothing, and contacted no printer. Neither did this merge.

What was and was not copied:
- No firmware bytes, vendor code or code from the external crate were copied.
  The Rust below is our own code, written from the facts.
- The test vectors are hand-checked frames and the synthetic gradient JPEG
  data from the upload plan. That JPEG is not vendor material.
- The command table is in `data/firmware/mini1-command-registry.json`: a
  cleaned copy of the external `command-registry.json`, with attribution, no
  absolute paths, and a per-opcode `outstax_protocol` status column.

## What the external analysis established

All of the following is for the mini Link 0104 image only. Addresses are the
external report's.

**Command table.**
- 28 distinct opcodes in 30 records. `1001` and `2001` appear twice.
- Lengths are exact, except `3001` (any length).
- The parser accepts a declared length below `0x5140`.

**Handler results versus wire results.**
- Internal `0x10` is sent as `00`, and the continuation is kept.
- Internal `0x12` and `0x13` send no frame.
- `0x7e` is sent as `0x7f`.
- A global state forces `0x90` (state 0) or `0x91` (state 5), even over a
  handler's success.

**`0001` (device info).**
- Selectors 0..8 are accepted. Larger values fail `0x81`.
- The reply is `selector, len, bytes[len]`, so the frame is `10 + len` bytes.
- Selector 0 is `FUJIFILM` and selector 1 is `SP-4`.
- Selectors 2 and 3 are opaque runtime bytes.
- Selectors 4 and 5 are the header and footer versions, as four ASCII hex
  nibbles (`0104`).
- Selectors 6..8 are empty.

**`0002` (status).**
- Selectors 0..3 are accepted.
- The reply bodies, including the selector, are 11, 5, 9 and 9 bytes.
- Selector 0 is 600×800, type `02`, options `00`, 153 600 bytes.
- In selector 2, film count `0xF` means unavailable, not 15 shots.

**`0010`.** Selectors 0..3 are accepted. Selector 1 returns 10 or 14 bytes,
depending on a hardware selector.

**Upload (`1000`–`1003`).**
- START requires type 2 and a length of 1..153 600. The `option1` low nibble
  must be below 4, and its high nibble must be one of 0, 0x10, 0x20, 0x40 or 0x80.
- The START reply is a fixed block size of 900.
- DATA is exactly 911 bytes.
- DATA has no next-sequence check: it copies `min(900, remaining)` bytes to
  `seq·900` and echoes `seq`.
- The first and last markers are set in an `if/else`. A one-block upload
  therefore needs sequence 0 twice before END succeeds. This is static and
  untested.

**Transfer timer.**
- A successful `1000` reply loads 4925 ticks (channel 0) or 5910 ticks (other
  channels).
- A successful `1001` reply does **not** reload it.
- So the timer is a whole-transfer budget: nominally 49.25 s or 59.10 s with
  the 10 ms task.

**JPEG decoder wrapper (`0x0802DA62`).**
- SOI must be at offset 0, and EOI exactly at the last two declared bytes.
- The image must have three components in libjpeg colour space 3 (YCbCr).
- In the size table, 600×800 is used as is, 608×800 is centre-cropped and
  480×640 is scaled 5/4.
- The result is 1 440 000 bytes, internal format `0x10`: rows stored
  bottom-up, each row 600 R, 600 G and 600 B.

**Readback (`1010`–`1012`).**
- `1010` replies `format, flags, 0:u16, size:u32, 900:u32`. It fails `0xb3`
  when no image is stored.
- `1011 seq:u32` replies with `seq` plus 900 bytes, zero-padded.
- `1012` needs both markers. These use the same `if/else` as upload.
- The stored image may be the decoded RGB, not the JPEG.

**Controls and other commands.**
- `0102`: the selector, then four BE16 words.
  - Modes 0..5 behave as in firmware.md. Factory values are 6000 ×4.
  - The validator runs for every selector. Its fourth check re-reads word `a`
    instead of `d`.
- `0103`: `6 ≤ a ≤ b ≤ 3200`, `c < 500`, `10 ≤ d ≤ 3200`. The result of the
  lower-layer call is ignored.
- `0100`: power control with mode 0/1.
- `0101`: AIRCR system reset after the reply.
- `1081`: film reject (`RejectEnd>`).
- `2000`–`2081` use 1024-byte blocks and stage 1 MiB in external NOR with a
  readback compare. `2081` jumps through `0x1000FFD0`, and the installer lies
  outside the image.
- `3001`: modes 0–3, N 0–100, timing 1–100.
- `3002`: selectors 0–2.
- `3080`: a 13-byte reply.
- `3081`: with hardware selector 0, it rejects any word ≥ 64 and may persist to
  NOR `0x30000`/`0x31000`.

## What we ported

| Item | Location |
|---|---|
| `1010`–`1012` opcodes | `crates/protocol/src/lib.rs:57-60` |
| Fixed 900-byte block, 911-byte DATA rule (a constant, but the session still uses the START reply) | `crates/protocol/src/lib.rs:385-389` |
| Opt-in single-block repeat for 0104 (not used by the session) | `crates/protocol/src/lib.rs:431-442` |
| Identity, status and `0010` selector ranges for mini Link | `crates/protocol/src/device.rs:242-249` |
| Readback builders, format constants, `ReadbackInfo::parse` | `crates/protocol/src/readback.rs:7-63` |
| `Readback` collector: caller bound, per-block tracking, duplicate and padding checks | `crates/protocol/src/readback.rs:64-127` |
| Structural JPEG header parser | `crates/protocol/src/jpeg.rs:10-106` |
| `check_mini1`: firmware checks plus a baseline 8-bit policy | `crates/protocol/src/jpeg.rs:7-8`, `108-126` |

Tests added:

| Test | Location |
|---|---|
| Golden setup, finalize and last block (seq 20, 784 data bytes + 116 padding) from the external upload plan | `crates/protocol/src/lib.rs:569-620` |
| `single_block_repeat_is_opt_in` | `lib.rs:621-629` |
| `mini1_identity_and_status_vectors`: `0000` handshake and fixed reply, 19-byte `0002/0`, `0002/2` fields, selector ranges | `lib.rs:630-665` |
| Decoder: two frames at every split and bytewise, nested magic, fail-closed corruption with a clean restart | `lib.rs:666-714` |
| Readback request frames, info layout and rejections, collection | `crates/protocol/src/readback.rs:128-206` |
| JPEG dimension table, colour space and coding policy, boundary and structure | `crates/protocol/src/jpeg.rs:128-196` |
| `prepared_jpeg_passes_mini1_firmware_checks`: our encoder against the firmware rules | `crates/client/tests/session.rs:498-514` |
| `mini1_upload_uses_fixed_911_byte_data_frames`: a session upload with a 900 START ACK | `crates/client/tests/session.rs:515-571` |

## Deliberately not ported

**Firmware actions (no builders).** None of these gets a builder:
- `0100` power control
- `0101` reboot
- `1081` eject
- `2002`, `2010`, `2080`, `2081` update
- new persistent writes

They are recorded here and in the registry only.

**Pre-existing builders.** These already existed and are unchanged. The
session does not call the firmware ones:
- `settings::sleep_*` (`0102` mode 5 persists to NOR)
- `settings::set_head_correction` (`3081` may persist)
- `firmware::main_start`/`data` (`2000`/`2001`)
- `bt_start` (`20a0`)

**External decoder design.** The external `StreamDecoder` resynchronises past
corruption. Ours fails closed, and the session poisons itself. We keep that on
purpose, and a test records the difference.

**Frame-length bound.** The 20 800-byte limit applies to the printer's inbound
parser, and nothing establishes a response limit. Our encoder still allows
65 535 bytes. Every request we build is at most 1035 bytes.

**Upload planner.** The external `UploadPlan` type is not ported:
- `download_start`/`download_chunks` already pad.
- The session validates the ACK sequence.
- The `option1` nibble rule is already enforced through the advertised
  `options` byte (`00` on mini Link rejects LUT `0x08`).

**JPEG profile limits.** The external profile's JFIF-only and strict
exact-length response rules are not applied. JFIF is not firmware-required,
and exact lengths are proven for one model only.

**`3001` limits.** The mini Link limits (N ≤ 100, timing 1..100) are not
enforced in `led_pattern`, because the builder is shared across models.

**Other.** The CLI and analysis tools are not ported.

## Agreement with firmware.md

Both analyses agree on these facts:
- the fixed 900-byte block and 911-byte DATA frame
- the START rules for mini Link: type 2, the nibble rule, LUT rejected, 153 600
  bytes, option2/byte3 ignored
- the `0002/0` values
- DATA has no next-sequence check, and END needs both the first and last
  blocks
- the timer constants 4925/5910
- the `0103` bounds
- for `0102`: factory values 6000 ×4, the six modes, NV copies at
  `0x7000`/`0x8000`, and the "0 or ≥ 1500" rule
- `3080`/`3081` exist on mini Link only
- `1010`–`1012` are present and unused by any app
- internal `0x13` means no reply
- `0x90`/`0x91` silence
- `0001` returns `FUJIFILM` and the model
- `0010` selector 0 returns 8 bytes

The `0010` selector-1 length (10 or 14 bytes) matches the app's `HeadType`
rule.

## Disagreements and open points

1. **Upload timer reload.**
   - firmware.md (Upload, "Mid-transfer timeout") says each successful
     `1000`/`1001` reloads the countdown, based on Link 3 `0x08066244` and the
     table flag `f6`.
   - For 0104 the external analysis says only `1000` loads it. Its own
     listing (`status.json`, `0x080543ba..0x080543ec`) confirms this: `f6`
     gates the branch, but only `1000` stores `r7` at `+0x22`, and `1001`
     only sets state 2.
   - So on mini Link the 49/59 s figure is a budget for the whole upload,
     counted from the START ACK.
   - The Link 3 claim should be re-checked before anyone relies on it.
2. **`1010` reload.**
   - The external settings.md says the response path does not reload the
     counter for `1010`.
   - The same package's listing (`0x0805441a..0x08054428`) shows the `1010`
     branch storing the timeout and clearing the expiry flag. `1011` does not
     reload it.
   - We follow the listing. The external prose conflict is unresolved.
3. **Channel mapping.**
   - firmware.md maps 5910 to BLE and 4925 to SPP.
   - The external analysis only says channel 0 → 4925 and other channels →
     5910.
   - The two are consistent if channel 0 is SPP. This was not checked
     independently.
4. **`0102` validation scope.**
   - firmware.md (Link 3) applies the timer check to modes 4/5.
   - The external analysis (0104) applies it to every selector, and its
     fourth check tests `a` again.
   - This may be a model difference. Our builders send zero timers or the
     mode-2 defaults, which pass under either reading.
5. **`0100`.**
   - `firmware::main_abort`/`bt_abort` build `0100 [00]` as an update abort.
     This comes from the Square and Link 3 updater code in the apps.
   - On 0104, `0100` is a power-control request.
   - `main_abort` already returns `None` for `LinkModel::Mini`, so we never
     send it there.
6. **Single-block uploads.** This is an addition, not a conflict.
   - firmware.md does not mention the `if/else` consequence.
   - Whether Link 2, WIDE, Link 3 or Link+ share it is unknown. The option
     therefore stays off by default.
7. **Status semantics.** These are external labels. Our parsers keep their
   app-derived names.
   - `0002/2` film `0xF` means unavailable on 0104. `PrinterStatus::readiness`
     treats it as 15 shots.
   - The external analysis calls `0002/1` byte 2 the "high byte of a u16" and
     does not call it a percentage. `Battery::parse` rejects values above 100.
   - Neither point has been checked on hardware.

## Session-level follow-ups

Done (2026-11-12, `crates/client/src/session.rs`):
- For SP-4, `upload` calls `jpeg::check_mini1` before START.
- For SP-4, `upload` enforces the whole-transfer budget from the START reply
  (the shorter recovered value, 49.25 s). When it runs out, the client sends
  CANCEL and stops with a clear error instead of sending blocks the printer
  would reject with `0x85`. Test:
  `mini1_upload_stops_at_the_transfer_window_and_cancels`.

Still open:
- `session.rs` checks each DATA ACK against the chunk index. To use
  `repeat_single_block`, it would have to compare against the sequence in the
  sent frame. Only an upload of 900 bytes or less is affected.
- Readback has no session API. No app uses it, and it is untested on
  hardware.
