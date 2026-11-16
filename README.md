# OutStax

[![CI](https://github.com/ThisLimn0/outstax/actions/workflows/ci.yml/badge.svg)](https://github.com/ThisLimn0/outstax/actions/workflows/ci.yml)
[![Release](https://github.com/ThisLimn0/outstax/actions/workflows/release.yml/badge.svg)](https://github.com/ThisLimn0/outstax/actions/workflows/release.yml)

An independent desktop app and command-line tool for Fujifilm INSTAX Link
printers, written in Rust. It prints over Bluetooth LE from Windows, macOS and
Linux, on Intel/AMD and ARM machines. OutStax is not affiliated with Fujifilm and
contains no Fujifilm code, firmware or assets.

## Status

Version 0.1.0. One printer has been tested on real hardware so far: an original
mini Link (firmware 0104) on Windows 11, where a test upload and a real print both
worked ([hardware record](docs/evidence/hardware-2026-11-13-mini-link.txt)). The
other models speak the same protocol and should work, but nobody has tried them
yet. If you own one, a test upload (no film) and a report would help a lot.

| Printer | State |
|---|---|
| mini Link (original) | Printing verified on Windows 11 (Natural mode) |
| mini Link 2, mini Link 3, mini Link+ | Implemented, not yet tested on hardware |
| SQUARE Link | Implemented, not yet tested on hardware |
| Link WIDE, Link WIDE 2 | Implemented, not yet tested on hardware |
| instax Evo, LiPlay and WIDE Evo cameras | Not supported yet |
| SHARE SP-1/2/3 (Wi-Fi) | Not supported |

## Download

Ready-made builds are on the
[releases page](https://github.com/ThisLimn0/outstax/releases). GitHub Actions
builds and tests each one on its own platform from the tagged source.

| Zip | For |
|---|---|
| `outstax-0.1.0-windows-x86_64.zip` | Windows 10/11 on Intel and AMD PCs |
| `outstax-0.1.0-windows-aarch64.zip` | Windows 11 on ARM (Snapdragon laptops) |
| `outstax-0.1.0-macos-universal.zip` | macOS 11 or newer, Apple Silicon and Intel |
| `outstax-0.1.0-linux-x86_64.zip` | 64-bit Linux with glibc 2.31 or newer (Ubuntu 20.04, Debian 11 and later) |
| `outstax-0.1.0-linux-aarch64.zip` | 64-bit ARM Linux, such as a Raspberry Pi 4 or 5 |

Unzip it and start the desktop app: `outstax-desktop.exe` on Windows,
`./outstax-desktop` on Linux, `OutStax.app` on macOS. On Linux you need an X11 or
Wayland session and a running BlueZ service. The macOS app is not notarised by
Apple, so confirm the first start under System Settings > Privacy & Security
("Open Anyway"), then allow Bluetooth when macOS asks.

## Using the desktop app

1. Turn Bluetooth on, switch the printer on and close other printer apps.
2. Press **Connect** and pick your printer. The top bar then shows its battery
   and how many shots are left.
3. Open a photo with **Open** or drop it on the window. JPEG, PNG and WebP work.
   Photos with an embedded colour profile (Display P3 or Adobe RGB, say) are
   converted to sRGB first.
4. Frame it. In Fill mode you drag the photo to move it and scroll (or use the
   Zoom slider) to zoom. Double-click puts it back in the middle. Fit mode adds
   white margins instead. Brightness, contrast, saturation and colour temperature
   sliders sit on the right, along with Rotate 90°.
5. Try **Test upload (no film)** first. It sends the whole photo and then cancels,
   so no film is used.
6. Press **Print**. It asks before using a sheet of film and never retries on its
   own.

Every finished print is kept in **History**, where one click prints it again.
The preview goes through the same pipeline as the print, so what you see is what
gets sent (the colours on film will of course differ a little).

The app keeps its log, a protocol trace for each job, the history and the last
used printer in a per-user folder: `%APPDATA%\outstax` on Windows,
`~/Library/Application Support/outstax` on macOS and `~/.local/share/outstax` on
Linux. Settings has an "Open log folder" button. If something goes wrong,
[troubleshooting](docs/guides/TROUBLESHOOTING.md) explains the common errors and
what to include in a report.

## Command line

`outstax` covers the same printer functions and adds tools for testing and
protocol work. Use the exact device ID that `scan` prints.

```sh
outstax scan
outstax --device 'ID_FROM_SCAN' status
outstax --device 'ID_FROM_SCAN' --trace upload.jsonl print photo.jpg
outstax --device 'ID_FROM_SCAN' print photo.jpg --commit
```

Without `--commit`, `print` only does a test upload and nothing is printed. With
`--commit` it prints one photo and waits until the printer reports it finished.
Add `-v` (or `-vv`, `-vvv`) for more log output, or set `OUTSTAX_LOG=debug`.

Other useful commands:

```sh
outstax prepare photo.png --format wide --fit contain --output wide.jpg
outstax simulate --format wide            # a fake printer on 127.0.0.1:7878
outstax --tcp 127.0.0.1:7878 status       # talk to the simulator
outstax trace upload.jsonl                # decode a saved protocol trace
outstax --help
```

### Rich mode and Design Print

Rich mode uses Fujifilm's colour lookup table, which is not part of this project.
If you have the official app's APK, you can extract the table yourself and pass it
in:

```sh
python3 scripts/extract_lut.py path/to/base.apk --output instax-lut.bin
outstax --device 'ID_FROM_SCAN' print photo.jpg --lut instax-lut.bin --commit
```

The desktop app takes the same file under Settings. Design Print (`--design`,
mini Link+ only) is implemented from the recovered geometry. Both modes are still
untested on hardware.

## Build from source

You need Rust 1.88 or newer, about 2 GB of disk space, and internet access for the
first build.

1. Install Rust with [rustup](https://rustup.rs).

   | System | Also needed |
   |---|---|
   | Windows | The Visual Studio C++ Build Tools ("Desktop development with C++"). The rustup installer offers them. |
   | macOS | `xcode-select --install` |
   | Linux | A C compiler (`build-essential` on Debian/Ubuntu, `gcc` on Fedora). If the desktop build reports missing system libraries, install your distribution's X11, Wayland and xkbcommon development packages. |

2. In this folder, build both programs:

   ```sh
   cargo build --release --locked -p outstax
   cargo build --release --locked -p outstax-desktop --features desktop
   ```

3. The programs are now in `target/release/`.

On macOS, Bluetooth access has to be granted to the terminal you start the app
from, or to an app bundle. [platform/macos/Info.plist](platform/macos/Info.plist)
is a starting point for a bundle.

## For developers

- `cargo test --workspace --locked` runs the tests. `scripts/build/smoke_test.py`
  runs the built CLI against the simulator for mini, square and wide.
- `python scripts/build/release.py` builds a release zip for the current system,
  including the licence texts of every bundled dependency.
  `--target x86_64-unknown-linux-gnu.2.31` cross-builds the Linux zip with
  [zig](https://ziglang.org) and `cargo-zigbuild`.
- Pushing a `v*` tag runs [the release workflow](.github/workflows/release.yml),
  which builds, tests and publishes the Windows and Linux zips with
  [the release notes](.github/RELEASE_NOTES.md).
- `python scripts/build/prose_check.py` checks the documentation style.
- [docs/guides/HARDWARE_TESTING.md](docs/guides/HARDWARE_TESTING.md) says how to
  test a printer model and what to report.

The code lives in four crates:

| Crate | What it does |
|---|---|
| [`crates/protocol`](crates/protocol) | Wire format, packet builders and parsers, with no dependencies |
| [`crates/client`](crates/client) | Bluetooth and TCP transport, print sessions, image pipeline, logging |
| [`crates/cli`](crates/cli) | The `outstax` command, trace decoder and printer simulator |
| [`crates/desktop`](crates/desktop) | The `outstax-desktop` app (egui) |

## How it was built

The protocol was worked out from Fujifilm's Android apps and the printers'
firmware by static analysis, and checked against earlier open-source work.
[docs/PROTOCOL.md](docs/PROTOCOL.md) describes the wire format as OutStax
implements it. Vendor apps, firmware and decompiled code are not part of this
repository.

## Credits and licence

OutStax is MIT licensed ([LICENSE](LICENSE)).

- [InstaxBLE](https://github.com/javl/InstaxBLE) by Jasper van Loenen (MIT) was
  the first public protocol reference. Its notice is in
  [licenses/InstaxBLE-MIT.txt](licenses/InstaxBLE-MIT.txt). InstaxLink and
  ESP32-Instax-Bridge were consulted too, but none of their code is included.
- The colour maths behind the photo sliders is ported from
  [LightCraft](https://github.com/storytold/lightcraft) (Copyright (c) 2026 ArtCraft
  Team and the LightCraft contributors, MIT OR Apache-2.0, used under MIT). The
  notice is in [licenses/LightCraft-MIT.txt](licenses/LightCraft-MIT.txt), and
  the ported functions are marked in `crates/client/src/lightcraft.rs`.

INSTAX and FUJIFILM are trademarks of FUJIFILM Corporation. This project is not
affiliated with or endorsed by Fujifilm.
