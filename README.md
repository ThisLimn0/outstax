# OutStax: independent client for INSTAX Link printers

An executable research implementation for INSTAX Link printers: a Rust library,
command-line application, native desktop application, and diagnostic simulator.

**Status: app RE in progress. This is not yet a complete replacement for Fujifilm's apps.**
The Mini, Square, and Wide Android apps have been acquired and examined. So far
the shared print protocol, capability negotiation, image upload, the Rich
lookup-table format and Mini Link+ Design Print geometry are implemented.
App-side RE and implementation come first and hardware qualification follows,
so no user unit/hardware test is requested at this stage. [The RE
record](docs/re/apps/APP_RE.md) lists the traced paths and the remaining work.

First hardware results (2026-11-13): on an original mini Link (SP-4, firmware
0104) over BLE on Windows 11, both "Test upload (no film)" and a real print from
the desktop app worked. The print took 30 s from connect to the printer reporting
it finished. Other models, Rich mode and Linux/macOS are not tested on hardware
yet. See [the record](docs/evidence/hardware-2026-11-13-mini-link.txt).

## Get it

Ready-made zips for Windows and Linux are on the
[releases page](https://github.com/ThisLimn0/outstax/releases). GitHub Actions
builds them from the tagged source, after the tests and a simulator run pass.

- **Windows (x64):** unzip `outstax-0.1.0-windows-x86_64.zip` and run
  `outstax-desktop.exe` (desktop app) or `outstax.exe` (command line). Its
  `README.txt` covers first steps and logs.
- **Linux (x64, glibc 2.31 or newer):** unzip `outstax-0.1.0-linux-x86_64.zip`
  and run `./outstax-desktop` or `./outstax`. The desktop app needs an X11 or
  Wayland session, and Bluetooth needs a running BlueZ service.
- **macOS, or the latest source:** build it yourself (next section). macOS can
  only be built on a Mac.

## Build it yourself

You need about 2 GB of disk space and an internet connection for the first build,
which downloads the dependencies listed in `Cargo.lock`.

1. **Install Rust** (1.88 or newer) with [rustup](https://rustup.rs):

   | System | What to install |
   |---|---|
   | Windows | Run `rustup-init.exe` from rustup.rs. When it asks, let it install the Visual Studio C++ Build Tools ("Desktop development with C++"). Rust needs their linker. |
   | macOS | `xcode-select --install`, then the rustup command from rustup.rs. |
   | Linux | A C compiler (`build-essential` on Debian/Ubuntu, `gcc` on Fedora), then the rustup command from rustup.rs. The D-Bus library is compiled from vendored source, so no D-Bus headers are needed. The desktop app needs an X11 or Wayland session. If its build reports missing system libraries, install your distribution's X11, Wayland and xkbcommon development packages. |

   Check it worked: `cargo --version` prints a version number.

2. **Open a terminal in this folder** (the one containing `Cargo.toml`).

3. **Build both programs:**

   ```sh
   cargo build --release --locked -p outstax
   cargo build --release --locked -p outstax-desktop --features desktop
   ```

   The first build takes a few minutes. After that, builds are quick.

4. **Run them.** The programs are in `target/release/` (`outstax.exe` and
   `outstax-desktop.exe` on Windows):

   ```sh
   target/release/outstax scan
   target/release/outstax-desktop
   ```

Optional:

- Run the tests with `cargo test --workspace --locked`.
- Put `outstax` on your PATH with `cargo install --path crates/cli --locked`.
- Make a release zip like the ones above with `python scripts/build/release.py`
  (Python 3.8+). It builds both programs and writes
  `outstax-<version>-<os>-<arch>.zip` to this folder (`--output-dir ..` puts it
  next to this folder instead), with the licence texts of every bundled
  dependency. It never overwrites an existing zip.
- Cross-build the Linux zip from Windows or macOS with
  `python scripts/build/release.py --target x86_64-unknown-linux-gnu.2.31`. This
  needs [zig](https://ziglang.org), `cargo install cargo-zigbuild` and
  `rustup target add x86_64-unknown-linux-gnu`.
- Publishing a release: push a tag such as `v0.1.0`. The workflow in
  `.github/workflows/release.yml` builds, tests and attaches both zips, with
  `.github/RELEASE_NOTES.md` as the release text.
- To keep build output out of this folder, set `CARGO_TARGET_DIR` to a path
  elsewhere before building. `scripts/build/package_project.py` (the source
  archive) refuses to run while a `target/` folder is inside the project.

## First steps with a printer

Close other applications connected to the printer. Use the exact BLE device ID,
address, or name returned by `scan`. macOS may show an opaque identifier instead
of a MAC address. When both `(IOS)` and `(ANDROID)` names exist, the `(IOS)` BLE
endpoint is the relevant one for this implementation.

The remaining examples assume `outstax` is on your PATH. Install it with
`cargo install --path crates/cli --locked`, or use `outstax.exe` from the Windows
release zip.

```sh
outstax --device 'EXACT_ID_FROM_SCAN' --trace status.jsonl status
outstax --device 'EXACT_ID_FROM_SCAN' --trace upload.jsonl print photo.jpg
outstax --device 'EXACT_ID_FROM_SCAN' --trace print.jsonl print photo.jpg --commit
```

The first command queries capabilities, battery and film status. The second
uploads the whole image, then cancels it before the end of the upload, so
nothing is staged and PRINT is never sent. The third sends exactly one physical
print command and polls the recovered status flow until device-reported
completion or a finite deadline.

`PrintAccepted` means the device acknowledged the request. It does not prove
that the print was successfully ejected. A print timeout is an uncertain
physical result, and this client never retries it automatically.

The [capture guide](docs/guides/HARDWARE_TESTING.md) is kept for the later
hardware phase. For now the work is app-side RE.

## Desktop application

```sh
cargo run --release --locked -p outstax-desktop --features desktop
```

The top bar shows the printer (name, battery, film count with a 10-segment bar,
or "Not connected"). Connect scans over BLE and lists printers to pick from.
Each action opens its own BLE connection and nothing polls in the background,
so the bar shows the last reported status. When an action fails because the
printer is not found or the link drops, the bar switches to "Printer off or out
of range", drops the battery and film values, and offers Reconnect. The
last-used printer is remembered in `last-printer.txt` in the per-user
`outstax` data folder that also holds History (below). Versions from before
the rename used an `instax-desktop` folder. On first start the desktop app
renames it to `outstax` if no `outstax` folder exists yet.

Open a photo with the Open button (native file dialog) or by dropping it on the
window. On Linux the dialog needs an XDG desktop portal. The preview is drawn in a film frame with the printer's
reported aspect (mini 600x800 until a printer reports otherwise) and is
re-rendered from a downscaled copy through the same pipeline used for sending
(without the Design Print warp). The side panel has Brightness, Contrast,
Saturation and Color temperature sliders, Rotate 90°, Reset, Fill/Fit,
Zoom and Natural/Rich (Rich needs a LUT file set under Settings).

In Fill mode you can crop. Drag the preview to move the photo, scroll over it
or use the Zoom slider (1.0x to 4.0x) to zoom, and double-click it to centre it
again at 1.0x. The crop keeps the film's aspect and never leaves the photo, and
centred at 1.0x it gives the same output as before. Rotate 90° and opening a
new photo reset it. Fit mode ignores the crop but keeps it for when you switch
back. Saved prints from History can't be cropped.

These adjustments are this project's own filters, applied to the decoded image
before the existing resize/LUT/JPEG pipeline. They are not vendor processing,
and the vendor's automatic correction is not offered. They work in linear light,
with color math ported from the open-source editor
[LightCraft](https://github.com/storytold/lightcraft) (MIT OR Apache-2.0, notice
in `licenses/LightCraft-MIT.txt`). Brightness is a mid-tone gamma and Contrast an
S-curve on luminance, and both keep black and white in place. Saturation scales
OkLab chroma, which leaves hue and lightness unchanged. Color temperature is a
Bradford white-balance adaptation. Settings also holds the Design Print option.

Print asks for confirmation and performs one print without retry, while
"Test upload (no film)" transfers and then cancels. After the printer reports a
print finished, the exact JPEG sent is saved locally (Windows
`%APPDATA%\outstax\history`, macOS `~/Library/Application
Support/outstax/history`, Linux `$XDG_DATA_HOME/outstax/history`,
else `~/.local/share/outstax/history`). History shows these files newest
first, and choosing one resends it unchanged, like `print --prepared` (with
`--lut-applied` if it was a Rich print). This is a local reprint list, not the
vendor app's history feature. The GUI uses the same Rust client as the CLI.
Print queues, mobile shells and installer integration remain follow-up work.

Linux needs a running BlueZ system service and Bluetooth access. D-Bus is built
with vendored headers/library code. Windows uses the OS Bluetooth API. macOS
requires Bluetooth permission for the launching terminal or an application
bundle with `NSBluetoothAlwaysUsageDescription`. An example plist is included.
The bundled Linux binaries (2026-10-25, older than the current source) need
glibc 2.38 or newer for the CLI and 2.39 or newer plus a graphical session for
the GUI.

On Windows 11 x86-64/MSVC, tests and clippy pass natively, both applications
link and launch, and the simulator round trip works. The Bluetooth backend
initializes there, but scanning with the radio on, printing on hardware,
installer and signing are still pending. For the macOS Apple Silicon target,
compile checks pass for both applications, while linking, application launch
and Bluetooth hardware tests there remain pending. The supplied CI workflow has
not been executed on hosted Windows/macOS runners.

## Image processing and quality modes

JPEG, PNG and WebP imports are supported. Orientation metadata is applied and
transparent pixels are composited over white. Crop fills the film image, while
`--fit contain` adds white margins. Dimensions and maximum JPEG bytes come from
the printer rather than its advertised name.

```sh
outstax prepare photo.png --format wide --fit contain --output wide.jpg
outstax --device 'EXACT_ID' print wide.jpg --prepared --commit
```

Natural/basic printing sends a JPEG with no LUT flag. Rich mode applies a
64 MiB RGB table and sets option bit `0x08`. The table is identical across the
three acquired app versions. Vendor assets and decompiled vendor source are
not distributed with this project. To use a locally acquired base APK:

```sh
python3 scripts/extract_lut.py path/to/base.apk --output instax-lut.bin
outstax --device 'EXACT_ID' print photo.jpg --lut instax-lut.bin --commit
```

For a JPEG already processed with the appropriate LUT, use
`--prepared --lut-applied` instead. Applying the table twice changes the image.
The exact table lookup is implemented, while Android's JPEG encoder, optional
automatic enhancement, and interpolation are not reproduced byte-for-byte.

Mini Link+ Design Print requires the printer to advertise the extended
double-density capability:

```sh
outstax --device 'EXACT_ID' print artwork.png --design --commit
```

The normal 600×800 composition becomes a 1200×1600 composition, receives a
0.5-degree alignment rotation, and is squeezed horizontally into a 600×1600
JPEG. The independent cubic resampler follows the recovered geometry, but
pixel identity with OpenCV/Android is not asserted. This mode is experimental until
validated with a Link+ capture and physical print. It is rejected when the
capability is missing. See [the protocol notes](docs/protocol/PROTOCOL.md).

HEIF/DNG decoding, vendor automatic enhancement, creative templates, AR,
online sharing, firmware updates, and legacy SHARE Wi-Fi transport are not
implemented. The catalog distinguishes field layouts and callable paths from
unused enum identifiers. Settings have typed library APIs and selected CLI
controls. Offline firmware inspection and packet builders exist, but they do
not make an updater.

## Offline diagnostics

```sh
outstax decode '6142000c10000000000384b9'
outstax trace upload.jsonl
outstax commands
outstax firmware-inspect firmware.hex
outstax prepare artwork.png --format mini --design --unsharp --output prepared.jpg
outstax --device EXACT_ID handshake
outstax --device EXACT_ID control axes
outstax --device EXACT_ID control additional 1
outstax simulate --format wide
```

The simulator listens on localhost. In another terminal:

```sh
outstax --tcp 127.0.0.1:7878 status
outstax --tcp 127.0.0.1:7878 print photo.png
```

TCP here carries the Link protocol for testing. It is not the SHARE protocol.
The simulator is a deterministic implementation oracle, not evidence of real
hardware compatibility. Packet traces contain image bytes when an upload is
recorded, while status-only traces do not include photo uploads. Trace and output
files use create-new semantics to avoid overwriting earlier evidence.

## Project map

| Path | Purpose |
|---|---|
| `crates/protocol` | No-dependency wire codec, parser and capability structures |
| `crates/client` | BLE/TCP transport, acknowledged sessions, image pipeline |
| `crates/cli` | CLI, trace decoder, offline preparation and simulator |
| `crates/desktop` | Optional Rust/egui desktop interface |
| [`docs/README.md`](docs/README.md) | Documentation index and reading order |
| [`docs/status`](docs/status) | Plan, coverage matrices, verification record |
| [`docs/protocol`](docs/protocol) | Packet/field specification, unused-command backlog, cloud/HTTP summary |
| [`docs/re/apps`](docs/re/apps) | App RE record and topic notes (connection, closure, creative, cloud, enhancement, hybrid cameras, drivers) |
| [`docs/re/firmware`](docs/re/firmware) | Printer firmware analysis and firmware-bundle summaries |
| [`docs/guides`](docs/guides) | Hardware qualification/capture guide and troubleshooting |
| [`docs/evidence`](docs/evidence) | Build, test and simulator logs |
| [`data/protocol/commands.json`](data/protocol/commands.json) | 74 identifiers, 68 layouts, evidence references and enum-only gaps |
| [`data/acquisition/acquisition.json`](data/acquisition/acquisition.json) | Versions, package hashes, signing certificate fingerprints |
| [`data`](data) | Machine-readable catalogs by topic: `acquisition`, `protocol`, `firmware`, `apps`, `drivers`, `http` |
| [`scripts/extract_lut.py`](scripts/extract_lut.py) | Extract the Rich lookup table from a locally acquired APK |
| [`scripts/acquire`](scripts/acquire) | App acquisition (mirror, ADB pull) and APK signature check |
| [`scripts/build`](scripts/build) | Project packaging and simulator smoke test |
| [`scripts/research`](scripts/research) | Static-analysis reproduction scripts, including external firmware tools |

Start with the [documentation index](docs/README.md). Read
[coverage](docs/status/COVERAGE.md) before describing any model as supported.

## Attribution

The initial public protocol reference was Jasper van Loenen's
[InstaxBLE](https://github.com/javl/InstaxBLE), MIT licensed. Its notice is retained
in `licenses/InstaxBLE-MIT.txt`. Wire layouts were then checked directly against
the acquired apps. InstaxLink and ESP32-Instax-Bridge were consulted as
independent references, but none of their implementation code is included.
The desktop photo adjustments port color math from
[LightCraft](https://github.com/storytold/lightcraft), Copyright (c) 2026 ArtCraft
Team and the LightCraft contributors, MIT OR Apache-2.0. It is used under MIT,
with the notice in `licenses/LightCraft-MIT.txt` and the ported functions marked
in `crates/client/src/lightcraft.rs`. No LightCraft assets are included.

This is an independent project. INSTAX and FUJIFILM are their respective
owners' trademarks. Source code in this project is MIT licensed.

