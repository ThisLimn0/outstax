# Firmware findings: implications for the client

This note collects code proposals that follow from the firmware analyses. It
changes no code. Sources: [firmware.md](firmware.md) (STM32, ours),
[mini-link-0104.md](mini-link-0104.md) (external, mini Link 1), and the
external multi-firmware analysis, 2026-11-12: [square.md](square.md),
[wide2.md](wide2.md), [renesas-mini2-mini3.md](renesas-mini2-mini3.md),
[wide-evo.md](wide-evo.md). Labels (**recovered** / **likely** / **unknown**)
are those of the cited doc. Nothing here was tested on hardware.

## Applied (2026-11-13)

Items 1, 2, 3, 5, 6 and 7 below are implemented in `crates/client/src/session.rs`
(plus `crates/cli/src/main.rs` for the CPU line and the `IMAGE_BLOCK_LEN` comment
in `crates/protocol/src/lib.rs`):
- 1: `upload` skips `0103` for SQUARE over BLE and logs that it did
  (test `square_over_ble_skips_connection_parameters`).
- 2: the `0x90`/`0x91` warning says "stops answering" only for STM32 models
  (not Renesas, SQUARE, WIDE 2), otherwise "may stop answering".
- 3: `initialize` logs "printer CPU: STMicro/Renesas", and CLI `status` prints it.
- 5, 6: comments updated, behaviour unchanged.
- 7: `upload` refuses option2 on mini Link 3 before anything is sent
  (test `mini3_refuses_design_print_even_if_advertised`).
Items 4, 8 to 11 stay as written (no change, or later). None is hardware-tested.

## Client implications

Ordered by expected value. "Risk" is the risk of making the change.

1. **Skip or tolerate `0103` on SQUARE.**
   - Today: `Session::upload` (`crates/client/src/session.rs`, the
     `is_ble()` block before the status refresh) sends `0103` to every model
     over BLE and aborts the upload on any non-OK reply (`exchange`), or
     poisons the session after 15 s if no reply comes.
   - Evidence: the SQUARE app never sends `0103` (Classic only,
     [COVERAGE.md](../../status/COVERAGE.md)). The SQUARE firmware's group-`0x01`
     subtypes are **unknown** ([square.md](square.md)), although an LE
     connection-parameter path exists (**likely**, strings). The STM32 proof
     that `0103` succeeds ([firmware.md](firmware.md) §3) does not cover SUNP.
   - Proposal: for `LinkModel::Square`, send it with `exchange_raw` and only
     log a non-OK result, or skip it unless a CLI flag asks for it. The same
     per-model switch serves mini Evo Cinema later ([hybrid-cameras.md](../apps/hybrid-cameras.md)).
   - Risk: low. If SQUARE does handle `0103`, skipping it only loses the
     15 ms interval request.

2. **Model-qualify the `0x90`/`0x91` warning.**
   - Today: `Session::exchange_raw` logs for every model that the firmware
     "stops answering after this until it is switched off and on".
   - Evidence: recovered for the STM32 images only ([firmware.md](firmware.md)
     §3). For SUNP (SQUARE, WIDE 2) and RX the silent path is **unknown**.
   - Proposal: keep the text for STM32 models. For `Square`/`Wide2` (and when
     `cpu_type() == Some(1)`), say "may stop answering".
   - Risk: none (message only).

3. **Report the CPU type.**
   - Today: `Initialization::cpu_type()` is parsed but not shown.
   - Evidence: a mini Link 2 or 3 runs either the STM32 or the Renesas RX
     image, and the RX analysis does not re-derive the BLE/timer behaviour
     ([renesas-mini2-mini3.md](renesas-mini2-mini3.md)).
   - Proposal: print it in the CLI `status` output (`crates/cli/src/main.rs`)
     and log it in the desktop job log, as "STMicro"/"Renesas". This tells us
     which firmware analysis applies to the user's printer when diagnosing
     link drops.
   - Risk: none.

4. **No SQUARE / WIDE 2 / RX transfer budget yet.**
   - Today: the 49.25 s whole-upload budget applies to `LinkModel::Mini`
     (SP-4) only (`Session::upload`, `check_upload_budget`).
   - Evidence: no timer constant or reload rule is reported for SQUARE or
     WIDE 2 (**unknown**). RX DATA checks a "timeout/state" but the constants
     are not given (**unknown**). The RX `f6` flags equal the STM32 ones, so the
     STM32 per-block reload (Link 2/3, firmware.md §2) is **likely** but the
     mini Link `0104` result (only START reloads) shows the flag alone does not
     decide it.
   - Proposal: no budget for these models. Keep logging the upload duration.
     If a hardware trace shows a `0x85` after a fixed time, add the measured
     value per model.
   - Risk of adding one now: false aborts on slow but valid uploads.

5. **Keep the negotiated block size and zero padding, and fix the stale comment.**
   - Today: `transfer_chunk_size` takes the START reply, and `download_chunks`
     pads every frame to that size (`crates/protocol/src/lib.rs`). The comment
     on `IMAGE_BLOCK_LEN` says SQUARE, WIDE 2 and the Renesas images are
     unanalysed.
   - Evidence: RX tables fix DATA at 911 bytes like STM32 (**recovered**). SUNP
     returns a runtime segment size and computes `ceil(len / size)` blocks. The
     WIDE 2 writer bounds data by its receive buffer, and whether trailing
     padding is truncated or rejected is **unknown** ([wide2.md](wide2.md)). The
     WIDE app pads ([PROTOCOL.md](../../protocol/PROTOCOL.md)).
   - Proposal: no behaviour change. Update the comment: "RX: same 911-byte
     table. SQUARE/WIDE 2 (SUNP): size from the START reply, no fixed frame".
   - Risk: none.

6. **Never resend a DATA block.**
   - Today: no retries (`exchange_raw` doc comment).
   - Evidence: WIDE 2 compares each sequence with the expected one and the
     block count (**recovered**). STM32 overwrites any in-range block.
   - Proposal: keep the no-retry rule and state the WIDE 2 reason in the
     comment, so a later retry feature does not resend on WIDE 2.
   - Risk: none.

7. **Hard-block option2 ≠ 0 for FI033.**
   - Today: option2 bit 0 is sent only when `options2` advertises it
     (`Session::upload`).
   - Evidence: both the STM32 and the RX mini Link 3 START validators reject
     any nonzero option2. The RX 307,200-byte branch is unreachable
     ([renesas-mini2-mini3.md](renesas-mini2-mini3.md)).
   - Proposal: an explicit `LinkModel::Mini3` guard next to the existing
     `options2` check, so a misparsed capability cannot produce a START that
     both builds reject with `0x81`.
   - Risk: none for valid printers.

8. **START options we send are not contradicted.**
   - We send `[2, 0|8, 0|1, 0]`. SQUARE copies three bytes and WIDE 2 four,
     but neither has a reported validator (**unknown**). Nothing in the new
     analysis says our START would be rejected. No change. The LUT bit stays
     gated by the advertised `options` byte.

9. **Name handling (no change needed today).**
   - Evidence (*our check*, **likely**): SQUARE names are `INSTAX-<id> (IOS)` /
     `(ANDROID)` with a space, and WIDE 2 names are `INSTAX-P-<id>(BLE)` / `(EDR)`.
   - `crates/client/src/ble.rs` accepts a device by print-service UUID or an
     `INSTAX` substring, so both match. If a name-based model hint or a
     "prefer the LE entry" rule is added later, it must accept `INSTAX-P-`,
     the space, and both `(IOS)` and `(BLE)` suffixes.
   - Risk: none.

10. **Sleep / keep-alive.**
    - SQUARE and WIDE 2 have an app-driven auto-sleep setting (**likely**,
      strings). What resets their timer is **unknown**. The STM32 rule
      (ordinary commands do not reset it) is unproven for them.
    - Proposal: if the desktop ever keeps a connection open between jobs, send
      `settings::sleep_extend()` (mode 3) periodically, as the WIDE app does.
      Today both front ends connect per job, so no change.

11. **WIDE Evo: expect camera-side pairing.**
    - When FI028 is added to `LinkModel::from_model_number`
      (`crates/protocol/src/device.rs`), document that the camera has an LE
      pairing database, numeric comparison and a pair-registration menu
      (**likely**, [wide-evo.md](wide-evo.md)). btleplug cannot pair, so an
      early disconnect on FI028 should produce a message that suggests
      registering or pairing the PC first.
    - Risk: unknown until tested.

## For the BLE link drops

No newly analysed firmware shows a disconnect timer. The evidence that matters:
- WIDE 2 carries the same ten Realtek component IDs as mini Link 3
  (**likely** the same module family), so a module-side security or
  supervision rule would affect both. It is still **unknown**.
- WIDE Evo has explicit camera-side bonding (**likely**).
- For a mini Link 2/3, item 3 tells whether the STM32 analysis even applies.

Pairing the printer in Windows before running the client remains the test to
run ([connection.md](../apps/connection.md)).
