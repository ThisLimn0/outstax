# App reverse-engineering record (2026-10-25)

This record marks a static-analysis milestone. It does not claim that the apps
have been completely reverse engineered. The user asked for app RE to be
completed before being asked for tests, and none of the work listed below needs
a user test or printer capture. Hardware qualification follows the remaining
app-side analysis.

## Inputs and provenance

The acquired Android applications are Mini Link 7.0.6 (336), SQUARE Link 1.6.2
(60), and Link WIDE 2.0.0 (204). Base APKs, asset splits and ARM64 splits were
retrieved and their embedded APK signatures verified. Hashes and certificate
fingerprints are in [`data/acquisition/acquisition.json`](../../../data/acquisition/acquisition.json). This verifies integrity against the
embedded signers. No trusted live Play installation was available, so publisher
identity and the latest release in every region were not independently
established. Mirror `latest` is an acquisition label, not such proof. No app was
executed.

[`data/apps/source-evidence.json`](../../../data/apps/source-evidence.json) records SHA-256 for 424 relevant decompiled source
files across the three apps. The input APK hashes are the reproducible anchors,
since JADX output may change between decompiler versions. Decompiled
implementation, vendor firmware, LUT assets and vendor keys are not
redistributed here.

Client schema ceilings in the Mini and Wide configurations are revision 0 for
device/battery/printer/history structures, with image revision 1 or 2 depending
on model. The Rust handshake requires a print-function bit and rejects newer
unknown versions before a product print flow.

## What was traced

The second pass followed request builders, response parsers, task switch tables,
model gates, firmware assets and selected ARM64 JNI routines. This extends the
first pass, which mostly catalogued opcodes. [`data/protocol/commands.json`](../../../data/protocol/commands.json) now distinguishes
68 recovered request/response layouts from six identifiers for which no emitting
builder/caller was found. A layout does not establish a usable feature or device
support. `3020` has an implemented write task, while a read response is still
undetermined. Some layouts belong to hybrid cameras, outside standalone Link
printer functionality.

In Square the R8-merged classes had to be followed through integer
discriminators rather than by searching for command names. Its request
codecs are in L1.b and T1, the response codecs in S1/U1 and the tasks in W1. Its
print request, negotiated u32 block size, padding, settings and firmware layouts
agree with the shared library. No explicit `Method not decompiled` or
`JADX ERROR` markers were found in the inspected wire packages. That does not
establish that every decompilation is structurally correct. The head-correction
branch was checked against JADX's DEX fallback because the structured output
obscured control flow.

| Area | Static finding | Implementation consequence |
|---|---|---|
| Transport | Shared frame over BLE or Android Classic SPP (WIDE2 uses BLE) | Same codec, transport-specific discovery/setup |
| Handshake | Versions, identity 0..5, image info, main firmware. Link 3 adds identity 9/10 and BT firmware | Explicit Rust handshake added |
| Printing | Status gate, optional head correction, START/DATA/END, status gate, PRINT once, status polling | Fresh status and completion polling added |
| Errors | Wire result distinct from parsed printer status | Valid negative reply no longer poisons the connection |
| LED | Per-step BGR interleaving: Link+ single, older dual, Link 3 triple | Typed variable-channel builder added |
| Axes | Signed big-endian i16 and packed switch byte | Typed parser, no little-endian assumption |
| Parameters | Ten-byte request, and the write task uses the result only | Write builder. The read response remains unknown |
| Main firmware | DAT/HEX selection, START/DATA/END, backup, poll, activate | Offline parser, packet builders and documented state flow |
| Link 3 BT firmware | Ten ordered files, complement-of-sum checksum, completion mask | Packet builders and exact manifest inventory |
| Head aging | FUTABA calendar-month correction with a common channel age limit | Independent numeric implementation |
| Native Design geometry | Cover resize, crop, +0.5 degree cubic warp, horizontal squeeze | Existing independent resampler retained |
| Native optional unsharp | Linear-light USM with thresholded/blurred luminance mask | Independent algorithm port behind an optional offline preparation flag |
| Cloud | Literal Retrofit routes plus dynamic URL operations | Catalog and model schema inventory, with no service requests sent |

## Model and connection rules

| Identity model | Standalone printer | App transport detail |
|---|---|---|
| SP-4 | mini Link | Classic Android path and BLE characteristic path |
| FI023 | mini Link 2 | Same Link codec |
| FI033 | mini Link 3 | Additional BT firmware identity/update paths |
| FI046 | mini Link+ | Double density and model-specific LED/parameter commands |
| FI017 | SQUARE Link | Same frame, runtime dimensions and block size |
| BO-22 | Link WIDE | Classic Android path and BLE characteristic path |
| FI049 | Link WIDE 2 | DeviceType WIDE2, BLE-only, `instax-P-A3` advertisement family |

FI044 is a hybrid-camera identifier and must not be used for Link+. A shared
product name does not prove that regional/Nintendo editions are equivalent.
Device identity/capabilities must remain authoritative.

Classic SPP uses `00001101-0000-1000-8000-00805F9B34FB`. Android tries secure or
insecure sockets and a channel-1 reflection fallback. This is not an additional
packet encryption layer. BLE service/write/notify UUIDs are in [PROTOCOL.md](../../protocol/PROTOCOL.md),
and notifications and writes carry the same complete-frame stream. The apps
serialize tasks. There is no independent transaction ID that would make
replaying PRINT safe.

The initial task in Mini T1.n requests `0000`, `0001` 0..5, image `0002/0`, and
`2010`. FI033 inserts identity 9/10 after software version, then `20a8` after
`2010`. Other models skip that BT program step even though the helper's model
allowlist is broader. SP-4 then uses `0010/1` to identify the print head. A text
containing AD, or with a length other than 14, selects FUTABA. Anything else
selects TOHOKU. The Rust handshake now includes this head identification (see
the state table at the end and [protocol-closure.md](protocol-closure.md)).

## Status and print state flow

The first film/status byte contains film in bits 0..3, battery level in 4..6,
and charging in bit 7. Readiness requires battery level 1, 2 or 3. Firmware
requires 2 or 3. This field is distinct from the detailed battery query.

The app checks errors, busy, cover, and film. During completion, film=0 is allowed
because a successful job may have consumed the last exposure. Error-bit priority
is low power, battery absent, battery overheat, charge fault, jam, print fault,
cover, software, hardware, mechanical. Unknown values remain unknown.

After a successful PRINT response, its first byte is estimated seconds. The app
polls `0002/2` every second. A frame result C2 (NOW_PRINTING) causes another poll
without parsing a success body. With result 00, printer readiness may still
report busy. Completion is when the operation/busy bit clears. Failure never
implies that PRINT can safely be replayed. Rust now follows this flow with a
finite completion deadline. It reports device-reported completion, not optical
proof of a correctly exposed/ejected photograph.

Remote shutter is host-side interpretation of `3000`'s switch bit 0. Link 3's
additional button is bit 3, and AR pressure uses bits 1..2. Mini X3.Q also
interprets orientation/axes for UI zoom/print-mode actions. These are
application controls, not a separate image-print protocol. A complete camera UI
and gesture behavior have not yet been ported.

## Main-controller firmware

The task gates function versions, battery and printer status, but allows
no-paper for updating. The sequence is:

1. `2000`: file length and write address. The reply supplies the u32 block size.
2. `2001`: u32 sequence plus negotiated block, zero padded at the end.
3. `2002`: end download. Query printer status.
4. `2080`: backup. The response contains estimated seconds.
5. Poll `0002/2` each second while the print-result field is 127.
6. `2081`: activate. The response contains program u16 and estimated seconds.
7. The higher application layer waits/reconnects and re-reads identity/version.

Errors take the abort path `0100` with a zero byte, except connection loss, which
ends the task. The enum named FW_UPGRADE_EXIT is not evidence that it is used.
The app's main transfer task does not enforce every echoed data sequence. The
independent builder preserves sequence fields and should validate ACKs.

DAT is transferred verbatim with address zero. HEX handles data, EOF, extended
segment and extended linear address records. The bundled STM32 HEX images also
contain type-05 execution entry-point records, which the app ignores rather than
treating them as firmware data. The Rust parser retains type-03/05 entry points
as metadata and never substitutes them for the write address. The app
concatenates data records in file order and takes the minimum address, without
filling holes. The Rust inspector retains record order, validates checksums, and
distinguishes this app layout from a contiguous address image.

The two Renesas HEX assets have no type-01 EOF record, and the app accepts
physical end-of-file. The Rust inspector records marker presence separately and
accepts these files after record checksum/address validation. All seven bundled
HEX assets are contiguous, have valid record checksums, and contain 917,504 data
bytes. STM32 images start at 0x08020000 and Renesas images at 0xfff00000. See
[`data/firmware/firmware-hex-layouts.json`](../../../data/firmware/firmware-hex-layouts.json). An S-record parser exists in the library but is
not selected by the inspected firmware-file loader.

Firmware CPU selection comes from the eighth byte of `3010/1` after print count,
battery type, color and film-PI: 0=STMicro, 1=Renesas. Do not infer CPU from the
retail model or hardware revision string. The corresponding asset paths and all
22 firmware-related asset hashes are in [`data/firmware/firmware-assets.json`](../../../data/firmware/firmware-assets.json).

## Link 3 Bluetooth-controller firmware

The task is gated to FI033. The acquired cfg_Ver1012.cfg specifies this order:

| Component ID | File role |
|---|---|
| 0100 | CFG / OEM configuration |
| 0200 | ROM patch |
| 0300 | audio MP image |
| 0400 | APP RTL8753BFR image |
| 0410 | DSP config **MT** image |
| 0500 | DSP system image |
| 0602 | DSP application image |
| 0700 | first-stage secure bootloader |
| 0800 | OTA header |
| 0a01 | boot patch |

The UI loader skips the first two cfg lines and copies the file named in the
second column of each remaining row, requiring exactly ten copied files. The
transfer task takes component IDs from enum order and does not parse the first
column. The first-line 0x10000000 value is therefore not a START write address.
The UI compares the reported BT version lexicographically with 00010012 to offer
this bundled update. [`data/firmware/bt-firmware-components.json`](../../../data/firmware/bt-firmware-components.json) records the selected
files and actual transfer checksums. The non-MT DSP config asset is also
bundled, but this cfg selects MT. Transfer uses the signed/encrypted vendor
bytes as supplied, so describing the host protocol needs no key extraction.

For each component, `20a0` carries id u16, file length u32, checksum u32 and zero
u16. The checksum is the bitwise complement of the wrapping u32 sum of unsigned
file bytes, not an address or a CRC. The reply gives the block size. `20a1` then
sends sequence+zero-padded data and validates the echoed sequence, and `20a2`
ends that component and resets the sequence to zero. After all ten, `20a7` must
return 0x000003ff. Status, `20b0` backup, status polling and `20b1` activation
then follow the main-controller pattern. Abort sends `0100/0`. `20a6` and `20a9`
remain enum-only in the inspected call graph. Recovery after power loss is
unqualified, and packet builders do not amount to a complete updater.

## Head calibration

Original mini Link firmware digits >=103 query `3080` before uploading. Only
FUTABA head=0 with judge flag=1 causes `3081`. Base RGB intensities and factory
calendar date come from the reply. The algorithm uses channel offsets
h=(.046,-.0326,.0077), first slopes a1=(.01,.0113,.0123), second slopes
a2=(.0043,.0051,.0054), and a 28-month breakpoint. Each channel's time to an
intensity of 63 limits a common age. The actual calendar-month age is clamped
to that limit and zero. Intensity is truncated and clamped to 0..63. The
independent Rust implementation is in protocol/settings.rs. Day-of-month is not
used.

## Native optional unsharp path

Mini's ARM64 libsharpness.so exports ApplyUnsharpFilter at 0x7a14, USM Apply at
0x622c, ApplyUsm at 0x5ddc and the constructor at 0x5d50. It reads an image with
OpenCV imread(-1), invokes the filter, and writes the output with imwrite and no
explicit JPEG parameters. Default Java arguments are sigma=3, amount=.5,
threshold=.8. This path is separate from Design alignment.

The invoked Apply method converts nonlinear RGB through a LUT to floating-point
linear light, invokes ApplyUsm, then converts back and rounds/saturates to 8-bit.
Exported conversion constants are gamma=2.4, bias=.055, factor=12.92, forward
threshold=.04045 and inverse threshold=.0031308. These are the sRGB piecewise
conversion constants. Gaussian blur uses OpenCV's automatic kernel size, equal
X/Y sigma and border mode 4 (reflect 101). Let D=linear_input-blurred_input.
If threshold>0, the mask is constructed from twice abs(D), transformed using
BGR coefficients (.060608,.716904,.222488), then converted to nonlinear light.
A binary threshold produces 0/1, and a second Gaussian blur with sigma=1 smooths
that mask. D is multiplied by the mask per channel. The result is input+amount*D,
converted back to nonlinear light and saturated to 0..255.

The native library also contains a separate ApplyOnlyLuminance method, but the
JNI path above calls Apply. It would be incorrect to substitute that unused
method or a simple image-space sharpen kernel. The algorithm is recovered at
this level. An independent Rust port is supplied in client/sharpness.rs and
selected by `prepare --unsharp`. Numerical comparison against native output is
unfinished. OpenCV coefficient rounding and Android JPEG recompression also
remain sources of pixel/byte differences.

## State of app-side RE (2026-11-12)

Static analysis only. Nothing below is hardware-verified. Details and open items
are in the linked topic notes.

| Item | State | Record |
|---|---|---|
| Enum-only and read-mode closure | **Resolved.** 74 opcodes: 36 called, 38 never-called, 0 unknown. Never-called selectors/modes are listed. `3020` is Link 3 only and write-only, and its read layout stays undetermined. `3008` is sent by Square. No firmware image handles `2003`, `20a9` or any `8xxx` opcode. The 43 unused IDs are a backlog, not implemented. | [protocol-closure.md](protocol-closure.md), [unused-commands.md](../../protocol/unused-commands.md), [firmware.md](../firmware/firmware.md) |
| Model initialization, head calibration, settings flows | **Resolved (RE), integrated in the client.** Post-connect sequence per model, head identification (`0010/1`, SP-4) and calibration (`3080`/`3081`, Classic transport, SP-4, firmware digits ≥ 103) traced. Auto-sleep, power-LED and function-button recipes recovered. The client runs `Session::initialize` and the calibration gate. Over BLE the calibration is skipped, as in the app. | [protocol-closure.md](protocol-closure.md) |
| BLE connection lifecycle | **Partly resolved.** App pacing (MTU − 3 fragments, write-complete driven), `0103` as first request, 15 s request timeout, keep-alive with `0102` mode 3 and callback-driven disconnect detection traced. Firmware: `0103` accepted on all five images, no STM32 pairing timer, a stalled upload is discarded after about 59 s on BLE (about 130 s on WIDE) without a disconnect, ordinary commands do not reset auto power-off. Open: whether the radio module or Windows drops unpaired links, and BLE pacing for models Android drives over RFCOMM. | [connection.md](connection.md), [firmware.md](../firmware/firmware.md) |
| Rename | **Resolved: not possible.** There is no app feature or command. The advertised name is built from the serial number at boot and no protocol command writes it. | [connection.md](connection.md), [firmware.md](../firmware/firmware.md) |
| Creative and composition features | **Partly resolved.** Print pipeline, collage geometry, frames/templates, text/stickers, QR, camera/gesture/AR modes and history/reprint traced, with per-feature cloud/asset dependence. Open: Mini server-template geometry, Mini history cap, pixel comparison with official output. | [creative.md](creative.md) |
| Cloud flows | **Resolved (static).** Hosts, tokens/headers, transport errors and per-feature flows traced. Features that need Fujifilm's servers are identified and not reimplemented. Open: server-side token lifetime, retention, schemas unconfirmed against live traffic. | [cloud.md](cloud.md), [HTTP.md](../../protocol/HTTP.md) |
| Automatic enhancement (FDIA) | **Open, not feasible statically.** Call chain, parameters and stages identified. The stage maths and proprietary data are not recovered or redistributable. | [enhancement.md](enhancement.md) |
| Optional unsharp | **Partly resolved.** Native algorithm recovered and ported. The app applies it only in the double-density path at 1200×1600. Numerical comparison against native output needs OpenCV execution and was not done. | [enhancement.md](enhancement.md) |
| Hybrid cameras | **Resolved (app side).** No camera needs a new print packet. WIDE Evo and mini LiPlay+ need a model/capability gate. Evo Cinema also needs a per-model `0103` decision, and mini Evo and mini LiPlay need an RFCOMM transport. WIDE Evo firmware is consistent with this. Open: camera-side pairing. | [hybrid-cameras.md](hybrid-cameras.md), [wide-evo.md](../firmware/wide-evo.md) |
| Printer firmware | **Partly resolved.** STM32 dispatcher, upload validation, BLE link, power/auto-sleep and device name recovered for five images. Original mini Link `0104` covered by an external analysis, and firmware bundle contents summarized. External multi-firmware analysis (2026-11-12): Renesas RX tables match the STM32 tables of the same model. SQUARE and WIDE 2 are Sunplus/MIPS (SUNP) images with the same frame and START/DATA layout but a runtime block size, and WIDE 2 checks the expected DATA sequence. WIDE Evo Linux file systems extracted, with the printer driver path and a camera-side pairing stack named. Open: SUNP START checks, transfer timers and `0103` on SQUARE. Also open: RX BLE/timer behaviour, BT-module security, update recovery. | [firmware.md](../firmware/firmware.md), [mini-link-0104.md](../firmware/mini-link-0104.md), [firmware-contents.md](../firmware/firmware-contents.md), [square.md](../firmware/square.md), [wide2.md](../firmware/wide2.md), [renesas-mini2-mini3.md](../firmware/renesas-mini2-mini3.md), [wide-evo.md](../firmware/wide-evo.md), [README-implications.md](../firmware/README-implications.md) |
| Desktop drivers, iOS | **Partly resolved.** Driver corroboration recorded. No iOS binary inspected. | [DRIVERS.md](DRIVERS.md) |

The remaining open items need hardware, live services or binaries that are not
available here. None of them requires inventing protocol behavior.
