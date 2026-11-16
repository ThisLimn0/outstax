First public research release of OutStax, an independent client for INSTAX Link printers.

**Downloads**

- `outstax-0.1.0-windows-x86_64.zip`: Windows 10/11, x64.
- `outstax-0.1.0-linux-x86_64.zip`: x64 Linux with glibc 2.31 or newer (Ubuntu 20.04, Debian 11 and later). The desktop app needs an X11 or Wayland session and BlueZ.

Each zip holds the command-line tool `outstax`, the desktop app `outstax-desktop`, a short README, a troubleshooting guide and the licence texts of every bundled dependency. Both were built by GitHub Actions from this tag, after the tests and a simulator run passed. There is no macOS build yet. It has to be built on a Mac with the steps in the project README.

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
