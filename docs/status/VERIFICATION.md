# Verification performed

Date: 2026-10-25. Toolchain: Rust/Cargo 1.99.1, Linux x86-64.

| Check | Result |
|---|---|
| Workspace tests, including desktop feature | 21 tests passed, no failures |
| Clippy driver, all targets/features, warnings denied | Passed |
| Rust formatting check | Passed |
| Python helper syntax | Passed |
| Linux CLI release build | Passed |
| Linux desktop release build | Passed |
| Windows x86-64/MSVC CLI + GUI native build | Passed on Windows 11: tests, clippy, link, GUI launch, simulator smoke test ([evidence/windows-native.txt](../evidence/windows-native.txt)). BLE scan not run with the radio on |
| macOS Apple Silicon CLI + GUI target check | Passed. No native link/launch |
| Mini simulator workflow | Status, upload-only, trace decoding, simulated PRINT passed |
| Square simulator workflow | Status, upload-only, trace decoding, simulated PRINT passed |
| Wide simulator workflow | Status, upload-only, trace decoding, simulated PRINT passed |
| APK signature integrity | All nine acquired base/split APKs verified, and split signers match the base |
| Publisher identity against live official installation | Pending |
| Actual Bluetooth adapter / printer | On 2026-11-13 "Test upload (no film)" and one physical print (Natural mode) from the desktop app both passed on Windows 11 against an original mini Link (SP-4, firmware 0104) over BLE. The printer reported completion and film went from 7 to 6 ([evidence/hardware-2026-11-13-mini-link.txt](../evidence/hardware-2026-11-13-mini-link.txt)). Other models pending |
| GUI visual interaction | Not run in the headless environment |
| Firmware inspection | All nine bundled DAT/HEX main-controller images parsed, with address/contiguity/EOF behavior recorded |
| Firmware update and recovery | No complete updater or recovery qualification |

Tests cover framing golden vectors, checksums, declared lengths, fragmentation
at every boundary, coalescing, malformed/truncated streams, unknown opcodes,
frame-size bounds, capability fields, the Mini option-2 extension, image sizing
and alpha handling, Design raster dimensions, negotiated upload blocks, ACK
sequence matching, BLE transfer setup, single PRINT dispatch and timeout
poisoning, valid device-rejection recovery, C2 completion polling and last-film
completion. The end-to-end smoke test checks that upload-only never sends PRINT
and that the simulator receives a decodable JPEG in all three film formats.

The simulator's correctness is not independent evidence that every printer
firmware implements the recovered protocol identically. Cross-target checks
establish Rust compilation only, not functioning native installers or radios.

[`docs/evidence/`](../evidence/) contains test/build and simulator summaries, and
the `re-pass-*` files record the updated implementation. The commands below are reference build/check
instructions, not a request for user tests before app RE is complete:

```sh
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo build --release --locked -p outstax
cargo build --release --locked -p outstax-desktop --features desktop
python3 scripts/build/smoke_test.py target/release/outstax --output smoke-run
```

The prebuilt CLI requires glibc 2.38 or newer. The GUI requires glibc 2.39 or
newer, a working graphical session and Bluetooth permissions. Local source
builds are recommended for different operating systems and older Linux hosts.
