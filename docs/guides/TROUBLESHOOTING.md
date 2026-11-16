# Troubleshooting

## Where the logs are

Desktop app (`outstax-desktop`) log folder:

| OS | Folder |
| --- | --- |
| Windows | `%APPDATA%\outstax\logs\` |
| macOS | `~/Library/Application Support/outstax/logs/` |
| Linux | `$XDG_DATA_HOME/outstax/logs/` (default `~/.local/share/outstax/logs/`) |

Before the rename to OutStax the folder was called `instax-desktop`. On first start the desktop app renames it to `outstax` if no `outstax` folder exists yet. If that rename fails, the app logs a warning, starts with an empty `outstax` folder and leaves the old folder as it is. Logs written before the rename stay in `logs/instax-desktop.log`.

To open it, go to Settings (⚙) > **Open log folder**. The full path is shown there too.

The folder contains:

- `outstax-desktop.log`: the current log. At about 5 MB it is renamed to `outstax-desktop.log.1`. Only one old file is kept.
- `traces/<UTC time>-<job>.jsonl`: a protocol trace of each printer job (`status`, `test-upload`, `print`, `reprint`). Only the 20 newest are kept.
  **Traces contain the image bytes that were sent.** Don't share a print trace of a private photo. Reproduce the problem with a test image instead.

Each log line looks like this:

```
2026-11-12T10:15:30.123Z INFO  outstax_client::ble: connected; MTU 247, fragment 244 bytes, WithoutResponse
```

The time is UTC. After it come the level, the source module and the message. Panics are logged with their message and location. Set `RUST_BACKTRACE=1` to log a backtrace as well.

CLI (`outstax`): logs go to stderr. Only warnings and errors are shown by default. Add `--trace FILE.jsonl` to save a protocol trace. The CLI never overwrites an existing trace file.

## More detail

| | Default | More detail |
| --- | --- | --- |
| Desktop | `info` | Start it with `OUTSTAX_LOG=debug` (or `trace`) |
| CLI | `warn` | `-v` info, `-vv` debug, `-vvv` trace, or `OUTSTAX_LOG` |

Windows PowerShell example: `$env:OUTSTAX_LOG='debug'; & .\outstax-desktop.exe`. The old name `INSTAX_LOG` still works when `OUTSTAX_LOG` is not set.

What each level adds:

- `info`: scan results, connect, MTU and fragment size, upload start and end (bytes, duration), print start and completion, and every user action in the desktop app.
- `debug`: one line per request and reply: opcode, bytes sent, device result and reply length. Image data is never logged. Also turns on btleplug's own Bluetooth log. Other libraries stay at `warn`.

Example:

```
outstax -vv --tcp 127.0.0.1:7878 status
```

## Reading a trace

```
outstax trace 2026-11-12T101530.123Z-print.jsonl
```

This prints each reassembled frame as JSON on stdout: direction, 4-hex-digit command, payload. The `open` and `error` rows go to stderr. Every raw row has `time_unix_ms`.

Things to check:

- Time from `open` to `error`.
- The last `rx` before the error. For example, a DATA ack `1001` shows how far an upload got.
- The `error` message.

If uploads always drop after roughly the same time (about 30 s), whatever the photo, pair the printer in the operating system's Bluetooth settings first and try again. If they drop at random points, move closer to the printer.

## Common errors

| Message | Meaning / what to do |
| --- | --- |
| `Bluetooth is turned off…` | Windows reports the radio off (HRESULT `0x800710DF`). Turn it on in Settings > Bluetooth & devices. |
| `no Bluetooth adapters available` | No adapter is visible to the app. Check Device Manager and drivers. |
| `Printer not found…` | The printer was not in the 5 s scan. Switch it on, bring it closer and scan again. The log lists every device the scan saw (`found …`). |
| `Printer disconnected…` | The link dropped: printer switched off, out of range, or a radio/OS drop. The log line just before it (`BLE write failed: …`) has the raw error. On Windows `0x80000013` (RO_E_CLOSED) means the link was already gone when a write happened, so it is not a bad payload. If drops come a fixed time (about 25–35 s) after connect, pair the printer first in Windows Settings > Bluetooth & devices > Add device, then retry. An interrupted print is never retried automatically. |
| `response timed out` | The link is up but the printer sent no reply for 15 s. If the log shows a `LOW_POWER (0x90)` or `BATTERY_NOT_MOUNTED (0x91)` warning, the printer firmware stops answering after that reply until it is switched off and on. Charge it, power-cycle it, and try again. |
| `printer rejected XXXX: NAME (0x..)` | The printer refused a command. `NAME` is the device result, for example `COVER_OPEN`, `NO_FILM`, `PRINTER_BUSY`. |
| `printer is not ready: …` | A status check before the upload failed, for example film door open, no film or low battery. |
| `session is uncertain after transport failure…` | An earlier error ended the session. Reconnect. |
| `PRINT result uncertain…` / `print completion deadline reached…` | The physical print state is unknown. Check the printer before printing again. PRINT is never replayed. |
| warning `connect-time setup step failed` | One of the optional setup steps after the handshake failed. The apps ignore these, and so do the CLI and desktop. It matters only if later errors follow. |

## Reporting a problem

Attach:

1. `outstax-desktop.log` (and `.log.1` if the problem is older). For the CLI, the full command and its output with `-vv`.
2. The trace of the failing job, preferably made with a test image.
3. Printer model and firmware: the `identified … firmware …` line in the log.
4. OS version, Bluetooth adapter, whether the printer is paired in Windows, and roughly when the problem happened (UTC time in the log).

[HARDWARE_TESTING.md](HARDWARE_TESTING.md) lists the CLI hardware checks.
