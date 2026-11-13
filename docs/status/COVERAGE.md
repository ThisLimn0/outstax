# Coverage and remaining work

As of 2026-10-25, no printer was accessible from this environment. The first
hardware result came on 2026-11-13 (mini Link row). All other hardware cells
below are pending. Shared protocol implementation is evidence
for a test candidate, not proof that a particular firmware works.

## Printer matrix

| Printer | Implementation path | Evidence | Hardware verification |
|---|---|---|---|
| mini Link | Shared BLE Link, runtime capabilities | Public implementation + current app protocol. The Android app uses Classic SPP, so BLE pacing has no app reference | Test upload (no film) and a physical print (Natural mode) verified on Windows 11 over BLE, firmware 0104, 2026-11-13 ([evidence](../evidence/hardware-2026-11-13-mini-link.txt)). Rich mode not tested |
| mini Link 2 / regional 2 SE | Shared BLE Link, runtime capabilities | Current Mini app + common library. The Android app uses Classic SPP. Firmware: STM32 and Renesas RX builds exist, and the RX command table matches the STM32 one ([renesas-mini2-mini3.md](../re/firmware/renesas-mini2-mini3.md)) | Pending |
| mini Link 3 | Shared BLE Link, BLE transfer setup | Current Mini app. Firmware: STM32 and Renesas RX builds, both of which reject START option2 ≠ 0 ([renesas-mini2-mini3.md](../re/firmware/renesas-mini2-mini3.md)) | Pending |
| mini Link 3 Nintendo Switch edition | Candidate shared Link 3 path | Same product family. Variant equivalence not tested | Pending |
| mini Link+ | Shared BLE Link + optional double-density image path | Mini 7.0.6 app + ARM64 native inspection | Pending |
| SQUARE Link | Shared BLE Link, negotiated block size | Square 1.6.2 app + public implementation. The Android app is Classic-only and never sends `0103`. Firmware (SUNP/MIPS): GATT path present (likely), START/DATA layout and START-reply block size confirmed. START checks, transfer timer and `0103` support unknown ([square.md](../re/firmware/square.md)) | Pending |
| Link WIDE | Shared BLE Link, runtime image limits | Wide 2.0.0 app + public implementation. The Android app uses Classic SPP | Pending |
| Link WIDE 2 | Candidate shared BLE path | Wide 2.0.0 includes WIDE2, and the official driver has been acquired. Firmware (SUNP/MIPS + Realtek BT package): START/DATA layout confirmed, block size taken from the START reply, DATA checks the expected sequence. START checks and transfer timer unknown ([wide2.md](../re/firmware/wide2.md)) | Pending |
| SHARE SP-1 / SP-2 / SP-3 | Separate Wi-Fi protocol | Not analyzed in this implementation | Not implemented |
| WIDE Evo (FI028) | Not accepted: model gate rejects it | App: BLE, same print packets and order including `0103`. Needs only a model/capability gate. Firmware (Linux): consistent, though camera-side pairing may be required ([wide-evo.md](../re/firmware/wide-evo.md)) | Not qualified |
| mini LiPlay+ (FI044) | Not accepted: model gate rejects it | App: BLE, same print packets. Needs only a model/capability gate (START byte 3 = copies in the app) | Not qualified |
| mini Evo Cinema (FI041) | Not accepted: model gate rejects it | App: BLE, same print packets, never sends `0103`. Needs a model gate and a per-model `0103` decision | Not qualified |
| mini Evo (FI019) | Not supported: needs RFCOMM | App: Classic SPP only, same codec and print packets. BLE presence unproven | Not qualified |
| mini LiPlay (HM1) | Not supported: needs RFCOMM | App: Classic SPP only, plain JPEG print with option1 = 0 | Not qualified |

The current global standalone-printer list includes Mini Link 3, Mini Link+,
SQUARE Link and Link WIDE 2. Older Link devices are included as compatibility
candidates. Regional availability is not equivalent to a new protocol family.
The hybrid-camera verdicts come from [hybrid-cameras.md](../re/apps/hybrid-cameras.md),
and no camera needs a new print packet.

## Feature matrix

| Feature | State |
|---|---|
| BLE discovery and exact printer selection | Implemented, with the backend compiled on Linux. Hardware pending |
| Framing, checksum, unknown opcode preservation | Implemented and tested |
| Split/coalesced notifications | Implemented and tested |
| Image dimensions, formats, limits and options | Implemented from app layouts |
| Battery, film, cover, busy/error status | Implemented from app layouts |
| Device identity strings | Implemented. Hardware pending |
| ACK-driven, padded image upload | Implemented and simulator tested |
| Transfer block size negotiation | Implemented, with no hardcoded per-model block sizes. STM32 and RX fix 900, while SQUARE/WIDE 2 return a runtime size ([firmware.md](../re/firmware/firmware.md), [wide2.md](../re/firmware/wide2.md)) |
| BLE connection-parameter command | `0103` is sent first in each BLE print task, before the status query, as in the app. All five analysed STM32 images accept it ([firmware.md](../re/firmware/firmware.md)) and it is present in both RX tables. Unknown on SQUARE, whose app never sends it ([README-implications.md](../re/firmware/README-implications.md)) |
| BLE pacing | CLI and desktop fragment at negotiated MTU − 3 and wait for each write without extra delay, as the app does. `--write-size`/`--write-delay-ms` override this in the CLI ([connection.md](../re/apps/connection.md)) |
| Model initialization | `Session::initialize` runs the per-model post-identity steps once, without the app's polling loop. Mini Link 3 gets the shooting light, `3010` and `3002`, Square gets `3008`, `3010` and `0102/0`, and Wide gets `3010` and `0102/0`. CLI `status`/`print` and the desktop print jobs use it ([protocol-closure.md](../re/apps/protocol-closure.md)) |
| Head calibration | Original mini Link (SP-4) only, firmware digits ≥ 103: `0010/1` head identification, then `3080`/`3081` before START. The app does this only over Classic SPP and the client keeps that gate, so it is skipped over BLE, the client's only printer transport |
| Auto-sleep / keep-alive | `0102` read, disable, enable and extend (mode 3) recipes implemented, and the CLI reads the timers. No keep-alive loop: CLI and desktop connect per job. Ordinary commands do not reset the printer's power-off timer. Units are likely 0.1 s ([firmware.md](../re/firmware/firmware.md)) |
| Printer rename | Not possible. There is no app feature or command, and the advertised name is derived from the serial number at boot ([firmware.md](../re/firmware/firmware.md)) |
| Physical print command | Implemented, single dispatch. One print verified on an original mini Link (2026-11-13). Other models pending |
| Completion/ejection confirmation | Device-reported completion polling implemented from app/driver state flow. Optical/ejection validation pending |
| JPEG/PNG/WebP import, EXIF orientation, alpha, crop/contain | Implemented and tested |
| Natural/basic printing | Implemented. Physical color calibration pending |
| Rich lookup table | Reader and exact color indexing implemented. Needs a local asset |
| Mini Link+ Design Print | Header/capability/geometry implemented. Experimental |
| HEIF and DNG import | Not implemented |
| Vendor automatic enhancement | FDIA call chain, parameters and stages identified. The engine cannot be reproduced from static evidence ([enhancement.md](../re/apps/enhancement.md)) |
| Optional Link+ sharpness | Independent linear-light USM port, accepted only with Design Print (`--design --unsharp`) because the app applies it only in the double-density path. The app sharpens at 1200×1600 after its own JPEG encode, and Rust sharpens at print size. Numerical native equivalence pending ([enhancement.md](../re/apps/enhancement.md)) |
| Settings, LED/accelerometer and remote shutter | Typed packet builders/parsers and selected CLI controls. Full UX not implemented |
| Reprint and print queue | Desktop GUI keeps a local history of JPEGs the printer reported as printed and resends one unchanged on request. This is not the vendor history feature (the apps store the pre-LUT composite and re-run the pipeline, [creative.md](../re/apps/creative.md)). No print queue |
| Desktop printer presence | Printer-not-found / disconnected errors reset the status bar to "Printer off or out of range" with Reconnect. No background polling. The last-used printer is remembered locally |
| Photo adjustments (desktop GUI) | Brightness, contrast, saturation, color temperature, 90° rotation and a Fill-mode crop (drag to move, zoom 1x to 4x, film aspect, clamped to the photo, centred 1x identical to the plain fill). These are this app's own filters, applied before composition (preview and print share one pure function) and unit tested. The linear-light math is ported from LightCraft (MIT OR Apache-2.0, `licenses/LightCraft-MIT.txt`): mid-tone gamma brightness, luminance S-curve contrast, OkLab saturation, Bradford white balance. Not vendor processing |
| Firmware transfer/update | Main/BT field layouts and state flows traced, assets inventoried, offline parser and packet builders in place. Updater UX/recovery not implemented |
| Firmware recovery | Not investigated |
| AR, templates, stickers, cloud-connected features | Not implemented. Traced in [creative.md](../re/apps/creative.md) and [cloud.md](../re/apps/cloud.md), which list what needs Fujifilm's servers |
| Bluetooth Classic / RFCOMM | Used by the Android apps for mini Link 1/2, SQUARE, Link WIDE, mini Evo and mini LiPlay. Native backend not implemented |
| SHARE Wi-Fi | Not implemented |

## Platform matrix

| Platform | Evidence |
|---|---|
| Linux CLI and BLE backend | Compiled, and automated and simulator tests run. No radio or printer present |
| Linux desktop GUI | Compiled. Not run interactively in this headless environment |
| Windows x86-64/MSVC | Native on Windows 11: fmt/tests/clippy passed, CLI and GUI release binaries linked, simulator status/upload round trip passed, GUI window launched. The WinRT BLE backend reached the adapter, but the scan was refused with the radio off. Hardware print, installer and signing pending ([evidence/windows-native.txt](../evidence/windows-native.txt)) |
| macOS Apple Silicon | CLI and GUI target compile checks passed. Native linking, launch and hardware pending. Sample permission plist supplied |
| Android | Rust core reusable. Native permissions/JNI lifecycle and app shell not implemented |
| iOS | Rust core reusable. CoreBluetooth app lifecycle and native shell not implemented |

## Gates before a stable release

The app-side RE phase in [APP_RE.md](../re/apps/APP_RE.md) comes first. These
hardware steps are deferred reference criteria, not a current test request.

1. Capture an official-app session and run status/upload/one physical print on
   the user's printer. Compare the transmitted JPEG with the intended image.
2. Confirm firmware identifiers, capability extensions, frame timing and
   connection setup for every target model, and retain sanitized captures.
3. Validate Natural/Rich and Design Print on actual film, including orientation,
   margins, size limits, color handling and repeatable error recovery.
4. Qualify the implemented completion monitoring. Printer families may behave
   differently while processing. Contrary to some community claims, the
   recovered Android app polls status. Capture evidence must decide per-model
   behavior.
5. Execute Windows/macOS builds and Bluetooth tests, package signed installers,
   then implement and test Android/iOS app shells if mobile remains required.
6. Finish the feature-parity matrix, online-service design, firmware protocol
   review and recovery testing. Behavior that is unknown stays marked unknown.

