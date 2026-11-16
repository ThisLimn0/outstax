OutStax 0.1.1, an independent client for INSTAX Link printers. This release improves print quality.

**What's new**

- The JPEG quality search now goes up to 100 instead of 95, so simple photos use more of the printer's size limit.
- `outstax prepare` defaults to each printer's own size limit, and the desktop preview uses the same limit as the print, so it shows the real compression.
- Photos with an embedded colour profile (Display P3 from an iPhone, Adobe RGB from a camera) are converted to sRGB first, so their colours no longer look washed out.
- Resizing works in linear light, which keeps fine detail and bright edges cleaner. Large photos also open faster and use less memory.

These changes are tested against reference images but have not been printed on film yet.

**Downloads**

| File | For |
|---|---|
| `outstax-0.1.1-windows-x86_64.zip` | Windows 10/11 on Intel and AMD PCs |
| `outstax-0.1.1-windows-aarch64.zip` | Windows 11 on ARM (Snapdragon laptops) |
| `outstax-0.1.1-macos-universal.zip` | macOS 11 or newer, Apple Silicon and Intel |
| `outstax-0.1.1-linux-x86_64.zip` | 64-bit Linux PCs with glibc 2.31 or newer (Ubuntu 20.04, Debian 11 and later) |
| `outstax-0.1.1-linux-aarch64.zip` | 64-bit ARM Linux, such as a Raspberry Pi 4 or 5 on a 64-bit OS |

Each zip holds the command-line tool `outstax`, the desktop app (`outstax-desktop`, or `OutStax.app` on macOS), a short README, a troubleshooting guide and the licence texts of every bundled dependency. Every zip was built and tested by GitHub Actions on its own platform from this tag, including a run against the printer simulator. On Linux the desktop app needs an X11 or Wayland session and BlueZ. The macOS app is not notarised by Apple, so the first start has to be confirmed under System Settings > Privacy & Security ("Open Anyway").

**What it does**

Scan for a printer, check battery and film, open a photo, crop it to the film by dragging and zooming, adjust brightness, contrast, saturation and colour temperature, then do a test upload or print. Printed photos are kept in a local history for reprinting. The command-line tool covers the same printer functions plus trace and simulator tools.

**Hardware status**

Tested on one printer so far: an original mini Link (firmware 0104) over Bluetooth LE on Windows 11. A test upload without film and a real print both worked, and the printer reported the print finished. No other model has been tried on hardware, and neither has Rich mode or Linux. Reports from other printers are very welcome. A print uses one sheet of film and is never retried automatically.

**First steps**

1. Turn Bluetooth on, switch the printer on and close other printer apps.
2. Start `outstax-desktop`, press Connect, pick your printer and open a photo.
3. Run "Test upload (no film)" before printing anything.

If something fails, the log and a protocol trace for each job are in the app data folder (Settings, "Open log folder"). The README in the zip says what to send with a report.

OutStax is not affiliated with Fujifilm. It contains no Fujifilm code, firmware or assets.
