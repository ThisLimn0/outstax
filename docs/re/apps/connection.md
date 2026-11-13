# BLE connection lifecycle, keep-alive and renaming

Static analysis of Mini Link 7.0.6, SQUARE Link 1.6.2 and Link WIDE 2.0.0
(JADX output). Class/method names refer to those snapshots. No printer was
connected for this work. Hybrid-camera apps were not needed for these findings.

## Which models use BLE in the official Android apps

| App | BLE path | Classic SPP path |
|---|---|---|
| Mini | `M1.c` (DeviceType `B033_AND_B046`: names containing `instax` + `BLE`, `instax-7` Link 3 / `instax-A2` Link+) | `L1.e` (`BOD_AND_BO23`: `instax` + `android`, mini Link / Link 2). `InstaxApplication` has the lines creating both |
| Square | none found | `Q1.c` filter `instax` + `android` + `instax-5`, sockets in `R1.a` |
| Wide | `com.fujifilm.bluetooth.ble.BleConnectionService` (WIDE2, `instax-P-A3`) | `BtcConnectionService` for WIDE 1 (`InstaxBluetoothService.isWide1`) |

So on Android, mini Link 1/2, SQUARE Link and Link WIDE (1) print over
Bluetooth Classic RFCOMM, not BLE. The Rust client uses BLE for every model.
Only Link 3, Link+ and WIDE 2 have an official BLE print path to compare
against. For the older models the BLE endpoint is real (the printer advertises a
`(BLE)` name for iOS), but the Android apps do not show how it should be paced.

## 1. Connection setup and pacing

Facts from Wide `BleGattServiceManager$btleGattCallback$1` (Mini `M1.e` is the same code):

- `connectGatt(context, autoConnect=false, …)` (`BleGattServiceManager.connect`).
- On STATE_CONNECTED: `discoverServices()` posted to the main looper (`ble.f`).
- `onServicesDiscovered`: requires the service plus OPRX (notify) and OPTX (write),
  then enables notification (`setNotification(true)`, CCCD write).
- `onDescriptorWrite` (notify enabled): if no MTU yet, `requestMtu(512)` and,
  1500 ms later (`ble.h.run`), report connected with `mtuSize`, or GATT_ERROR if
  no MTU arrived. `onMtuChanged` stores `mtu - 3`.
- Then `requestConnectionPriority(1)` (CONNECTION_PRIORITY_HIGH).
- Writes (`BleDataService.writePackets`): `setWriteType(1)` = WRITE_TYPE_NO_RESPONSE.
- Fragmenting (`BleDataService.sendMessage/split`): the frame is split into
  `mtu - 3` byte packets. Only the first is written directly. Each later one is
  written from `onReadyForWrite`, which `onCharacteristicWrite(status==0)` calls. There is no
  timer or extra delay between fragments.
- Requests are one at a time: `InstaxService.executeRequest` sends a frame and arms
  a 15 000 ms timeout (`InstaxService.DEFAULT_TIMEOUT`, runnable `bluetooth.g`). The
  task sends the next request only from `processResponse`, so every DATA block
  waits for its ACK.
- Notifications are put back together by the declared frame length (`BleDataService.onRead`).

Connection-parameter command `0103` (Wide `BleConnectionRequest.build`, Mini `Q1.a`):
four big-endian u16 `12, 12, 0, 400`, i.e. interval 15 ms (min = max), latency 0,
supervision timeout 4 s. `SendPrintImageTask.start`/Mini `T1.x` set the first state to
`BLE_CONNECT` when `isBleConnection`. The response (SID BLE_CONNECT) moves to
INSTAX_INFO_CHECK, then battery `0002/1`, printer status `0002/2`, optional head
correction, then START. So 0103 is the **first** request of every print task, and
the app waits for its reply before going on.

### Rust before this change vs app

| Item | App | Rust before | Now |
|---|---|---|---|
| Write type | without response | without response if advertised | unchanged |
| Fragment | negotiated MTU − 3 (MTU 512 requested) | fixed 20 bytes | `--write-size 0` (CLI default) = `peripheral.mtu() - 3`, clamped 20..512, read on each frame |
| Inter-write delay | none, the next write follows the write-complete callback | 2 ms `tokio::sleep` (≈15.6 ms on the default Windows timer) | CLI default 0, writes still awaited one by one |
| MTU request | `requestMtu(512)` | none (WinRT negotiates itself, and btleplug exposes `mtu()`) | read, not requested |
| Connection priority | HIGH | none (btleplug has no API) | unchanged, unknown whether needed |
| 0103 position | first, before battery/status | after status, right before START | first, before status |
| Per-request timeout | 15 s | 15 s | unchanged |
| Bonding | WIDE2 requires bond (below) | none (btleplug cannot pair) | documented only |

With 20-byte fragments and the Windows sleep granularity, a 100 KB JPEG needs
about 5 000 writes and roughly 80 s. The app needs about 200 writes. A longer
transfer leaves more time for any time-based drop to happen. Fragmenting at
the MTU and dropping the sleep is what the source shows.

### Bonding (possible cause of a mid-transfer drop)

- Wide `BleConnectionService.onDeviceConnectedToGhatt` arms `pairingTimeOutRunnable`
  (`ble.d`) for 28 000 ms. If the device is still not BOND_BONDED (12), it disconnects
  with `GATT_PAIRING_TIMEOUT`. The BLE path never calls `createBond`. Only the Classic
  path does (Mini `L1.e`, `createBond` + 15 s check). So the WIDE 2 printer itself
  must start pairing (security request or encrypted attribute).
- Disconnect handling treats status 8/19 with a bonded device as `GATT_DISCONNECTED_BY_DEVICE`.
- Mini `M1.c` BOND_STATE_CHANGED receiver: a drop to BOND_NONE (10) while connected and
  not yet paired → `REQUESTED_DEVICE_NOT_PAIRED` and disconnect. That is pairing
  activity on Link 3/Link+ BLE too, although no timeout is enforced.

btleplug never pairs. If the printer, or Windows, ends an unpaired link after a
security timeout (SMP uses 30 s), the drop happens a fixed time after connect.
With the old pacing that falls inside the upload. **Not proven.** The trace
timing below tells this apart from a parameter-update or flow problem.
Mitigation the user can test: pair the printer in Windows Settings > Bluetooth &
devices > Add device first, then run the CLI.

### RO_E_CLOSED

btleplug 0.12 WinRT closes `BluetoothLEDevice`/services only in `BLEDevice::drop`
(on `Peripheral::disconnect` or the last reference going away). WinRT reports
`0x80000013` when an operation hits a closed object. The Rust session sends nothing
after a failure except the caller's final `disconnect()`, so the HRESULT shows
that the link or object was already gone when a write was attempted. It is not
a bad payload. `link_error` maps it to the friendly DISCONNECTED message (kept).

## 2. Keep-alive, auto power-off, disconnect detection

- Wide app layer `com.fujifilm.instaxwide.util.bluetooth.InstaxBluetoothService.doAction`
  keeps the connection open and loops: INIT_INSTAX → GET_PRINTER_COLOR →
  GET_AUTO_SLEEP_SETTINGS (`0102` mode 0, with `isAutoSleepTurnedOff = timer1 == 0`) →
  GET_PRINTER_INFO (`PrinterInfoTask`: `0002/2`, `0002/3`, `0002/1`) →
  EXTEND_AUTOSLEEP_TIME (`InstaxWideService.extendAutoSleepTime`: `0102` mode 3
  `EXTEND_CURRENT_SLEEP_SETTING`, timers zero) → 2000 ms later GET_PRINTER_INFO again.
  In background, or if auto-sleep is off, it only polls printer info every 2 s
  (`C08761.onCompleted`). Polling pauses while another task runs (`getIsLoading`).
  This is the keep-alive: the app pushes back the printer's auto power-off while open.
- Mini `X3.G` runs a similar loop (GET_PRINTER_INFO2 / axis info, rescheduled with
  `f14482q = 1500` ms). Mode 3 is in the enum `W1.EnumC1884c` but no Mini caller
  was found. `L1.m` uses modes 0, 2, 5.
- Disconnect detection is callback-driven: `onConnectionStateChange` (newState 0 /
  status 8, 19) → `onDisconnected` or `sendConnectionLoseCallback`
  (`GATT_DISCONNECTED_BY_DEVICE`) → `InstaxService.onDataServiceError` →
  `task.onConnectionLose()` → CONNECTION_LOSE. A missing reply is
  `onWriteTimeOut` after 15 s → OPERATION_TIMEOUT.
- Reaction: `InstaxBluetoothService.onDisconnected/onError` post
  `DeviceConnectionEvent(DISCONNECTED)`, clear printer info and stop polling.
  `onError` and CONNECTION_LOSE start AUTO_CONNECT. If auto-connect is enabled,
  they retry after 1000 ms via `BleConnectionService.autoConnect` (500 ms delay).
  The GATT layer retries up to `MAX_RETRY_COUNT = 3` at 1000 ms (`ble.g`) when
  auto-connect is enabled. Bluetooth-off broadcasts (`STATE_CHANGED`) disconnect as well.

Why the desktop app misses a switch-off: it connects only for one job and then
disconnects (`crates/desktop/src/main.rs` `run`). Between jobs no link exists, so
there is nothing to detect. Inside a session, the old `BleTransport::receive`
waited on the notification stream, which does not end when the link drops. A
switch-off therefore showed up only as the 15 s "response timed out".

## 3. Renaming

There is **no rename feature**, either on the printer or in the app.

- No command in the Mini `W1.D` or Wide `SID` enums sets a device or Bluetooth
  name. `FRAME_PICTURE_NAME_SETTING` (`8303`) names a camera frame picture.
  `0001` device-info strings are read-only queries.
- No `strings.xml` (values) entry for rename, nickname or printer name. The only
  hits are placeholders `device_name_placeholder` / `a_device_name_placeholder`.
- The app shows Android's `BluetoothDevice.getName()`, or the address when the name
  is empty (Wide `BluetoothConnectionActivity` line ~304). It stores
  `Pair(name, address)` as `lastConnectedDeviceForBle/Btc` (`InstaxPreferences`).
  It does not use `setAlias` and keeps no user-editable alias.

So no protocol command was added. A desktop-only alias (keyed by BLE id or
address, stored in the app's local settings) would be a new feature, not parity.

## What changed in Rust

- `crates/client/src/ble.rs`: `write_size == 0` fragments at negotiated MTU − 3
  (`fragment_size`). `receive()` checks `is_connected()` every second and returns
  the DISCONNECTED message as soon as the OS reports the link down.
- `crates/client/src/session.rs`: 0103 moved before the pre-transfer status query.
  The trace gets an `open` row when tracing starts and an `error` row
  (`"<cmd>: <error>"`) on any exchange failure.
- `crates/cli/src/main.rs`: defaults `--write-size 0`, `--write-delay-ms 0`. The
  `trace` decoder prints `open`/`error` rows to stderr instead of rejecting them.
- `crates/client/tests/session.rs`: BLE ordering test (0103 first with exact payload),
  helper updated, `fragment_size` test.
- `crates/desktop` now passes `0` and no delay to `BleTransport::connect`
  (follow-up change), so the GUI uses the app pacing too.

## Diagnostic trace (no film)

```powershell
outstax scan
outstax --device <ID> --trace upload.jsonl print photo.jpg
outstax trace upload.jsonl
```

Without `--commit` the upload is staged and then cancelled, so PRINT is never sent.
For comparison, run again with the old pacing into a new file:
`outstax --device <ID> --write-size 20 --write-delay-ms 2 --trace upload-slow.jsonl print photo.jpg`.

What to look for in `upload.jsonl` (each row has `time_unix_ms`):

- Time from the `open` row to the `error` row. Roughly constant at 25–35 s across
  runs and image sizes → pairing/security timeout (try pairing in Windows
  Settings first). Failure only in the slow run → transfer duration.
- Failure right after the `0103` tx/rx pair → the connection-parameter update.
- The last `rx` before the error: a DATA ACK (`6142…1001 00 <seq>`) shows how far
  the transfer got. The `error` message shows whether it was a write (DISCONNECTED
  from send), a prompt DISCONNECTED from receive, or a 15 s response timeout
  (link up but printer silent).
- A fast run that succeeds confirms that pacing was the cause.

## Unknown

- Whether WIDE 2, Link 3 or Link+ firmware drops unpaired links, and after how long.
- Whether Windows honours the printer's parameter update after 0103.
  btleplug cannot request CONNECTION_PRIORITY_HIGH or preferred parameters.
- BLE pacing for mini Link 1/2, SQUARE and WIDE 1, which Android drives over RFCOMM.
- The meaning of the `0102` timer units, and whether a mode-3 extend affects
  older models (only the WIDE app sends it).
