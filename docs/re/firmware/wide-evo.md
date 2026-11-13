# WIDE Evo firmware (Novatek Linux): printer path and Bluetooth

Source: external multi-firmware analysis, 2026-11-12. The package,
`instax-multi-firmware-analysis`, is kept outside the project at
research/external/instax-multi-firmware-analysis. Its WIDE Evo chapter is
`report/multi/evo-analysis.md` (SHA-256
`8d716641eddea4d4e4c0b6cbd11de1f5c6092e52bf3e07722c39fd16253d5a76`), and the full
report is `report/multi/REPORT.md` (SHA-256
`31f913997398dde0045201932bb7dc61c1304619b25c4481ed1f0c5527c9f35b`). All package
files matched its SHA256SUMS. The extracted file systems and symbol tables are
under material/firmware/multi-analysis-2026-11-12, outside the project. No
vendor executable was run and no device nodes were created. No camera was
contacted.

Labels are as in [square.md](square.md): **recovered**, **likely** and **unknown**.
*Our check* rows are names we read on 2026-11-12 from the package's
`libhost-symbols.json` and file catalogue (outside the project). They are
symbol names, not traced code.

## Inputs and container

| Fact | Label |
|---|---|
| Official downloads `Ver100/FPUPDATE.DAT` (51,113,791 bytes, SHA-256 `470c8234…be71f`, the same bytes as the app's unsigned `FW_0103.DAT`) and `Ver101/FPUPDATE.DAT` (`61f48235…905e8`). Both target firmware 1.03. `Ver100`/`Ver101` name the upgrade source version | recovered |
| Ver100 is an exact prefix of Ver101, which adds a 1,280-byte trailer. A 256-byte RSA check of that trailer under the bundled OTA public key failed for all six tested message candidates | recovered (negative, bounded) |
| 128-byte opaque prefix, then 953 **dependent** LZ4 blocks (64 KiB dictionary), decoding to 62,428,448 bytes | recovered |
| Eight partitions: device tree (Novatek NA51055, Cortex-A9), U-Boot 2019.04, fastboot DT, RTOS image, Linux 4.19.91 kernel (Buildroot 2019.05.2) with an LZ4 initramfs, and three UBI images (main rootfs, an empty one, resource `rootfs2`) | recovered |
| 1,542 regular files, 373 symlinks, 174 ARM ELF objects, 103 kernel modules. All UBI/UBIFS CRCs are valid | recovered |

This supersedes [firmware.md](firmware.md) §6 ("not raw LZ4 … UBI/squashfs
extraction not done") and the [firmware-contents.md](firmware-contents.md)
note that no root file system or executable was extracted.

## Printer path

- `lib/libhost.so` (34.9 MB, stripped, 7,033 dynamic symbols) holds the camera,
  Bluetooth, image and print logic. **recovered**
- Image chain with call evidence: `Jpg2PrintRgb` (`0x189510`) → JPEG→YUV decode,
  `mwPbYuvCropRotateScale`, `Yuv2PrintRGB` (`0x188d00`) → `calc_shd`,
  `FF_PrintSetPod`, `instax_3D_Exec3DLUT` (`0x188b38`), YUV→RGB888. So the
  camera decodes the received JPEG, crops/rotates/scales it, applies shading
  and a 3D LUT on the device. **recovered** (calls). LUT axes, interpolation and
  output format are **unknown**.
- Seven `res_ro/iq/KC36D00/Film_*.bin` files of 823,875 bytes = 65³·3 (named
  summer, vivid, retro, magenta, palegreen, warm, lightblue). **recovered**
  (size), with the 3D-LUT role **likely**.
- `etc/init.d/Host_PrinterInit` loads `ab_printer.ko`, and userspace opens
  `/dev/ab_printer`. The module (349 symbols) exposes line send, head reset,
  gain, exposure start/stop, motor forward/reverse and 17 V GPIO. Its ioctl uses
  the low 8 bits as an index into a 100-entry table with handlers for 0–7, 9
  and 99 (`FF_PrintGetKernelInfo` = 0, `FF_PrintDeviceNodeDeinit` = 2,
  `FF_PrintMotorStart` = 4, `FF_Printer_Stop` = 5, `FF_PrintSetKernelInfo` = 9,
  `FF_PrintSetGpio17V` = 99). **recovered**. Line format and exposure timing
  are **unknown**. None of this is reachable over Bluetooth directly.

## Bluetooth stack

- Per the package, `HceBluetooth_*` symbols cover printer capabilities, image
  download start/end/cancel, firmware download/program, film-cover reject and
  camera settings. Command byte values and framing were **not** mapped. **likely**
- *Our check (symbol names):*
  - A host stack in userspace drives a **Realtek controller over UART**:
    `BTHost_Init`, `BTHost_SetUARTBaudRate`, `Realtek_EnterHciMode_Seq`,
    `BTHost_RealtekImageDownload`, plus the libraries `usr/lib/libabhostbthost.so`
    and `usr/lib/libabhostrtk.so`. Both LE (`BTHost_BLE_*`) and BR/EDR
    (`BTHost_BREDR_*`, `HceBluetooth_Notify_BREDR_Connected`) are present.
  - LE handlers that match the Link protocol: `HceBluetooth_Command_Set_BLE_Connect_Interval`
    and `BTHost_BLE_Server_UpdateConnectionParameters` (the `0103` path),
    `HceBluetooth_Command_EXE_Print_Download_Start/End/Cancel`,
    `HceBluetooth_Command_Get_PrinterCap_Info`, `…Get_PrintImg_Info`,
    `…EXE_Print_Reject_FilmCover`, `…EXE_FW_Download_Start/End/Cancel`.
  - Advertising and name: `BTHost_BLE_SetAdvertisingData`, `…SetAdvertisingParameters`,
    `…SetAdvertisingSlowInterval`, `…SetScanResponse`, `BTHost_BLE_SetDeviceName`,
    `BTHost_SetBlueToothInitialName`.
  - **Pairing and bonding on the camera:** `BTHost_BLE_Pair`, `BTHost_PairingConfirm`,
    `HceBluetooth_NumericCompare`, `BTInterface_BredrDeviceNumericComparison`,
    a pair database with LE LTK/IRK storage (`BtPairGetLinkKey_Ble_LTK`,
    `…_IRK`, `BtPairSetConnectStatusAndSave`), and setup-menu screens
    `SetupMenuBTPairReg*` (register, found, not found, help, erase).
  - Link loss: `HceBluetooth_Notify_BLE_Device_Lost`, `BTInterface_GetDisconnectInfo_Ble`.
  - Power: `SetupMenuAutoPowerOff`, `Setup_SetAutoOffTime`, `App_AutoPowerOffStatusSet`.
  All **likely** (names only). Whether print commands require a bonded,
  encrypted link, and the advertised name format, are **unknown**.

## Update verification (bounded)

`CheckFirmwareUpgradeCheckSum` is a `return 0` stub, but `mwCardUpdateFW`
calls a real `ab_fw_signature_verify` (OpenSSL EVP verify) before updating.
The stub is therefore not evidence that updates are unauthenticated. The
rootfs contains a public OTA key and an RSA private-key file whose role is not
established. The package reports metadata only, and so do we. **recovered**
(calls). Enforcement across all update paths is **unknown**. The client does not
update cameras.

## Comparison with the app-side verdict

[hybrid-cameras.md](../apps/hybrid-cameras.md) concludes for WIDE Evo (FI028):
"model/capability gate only": BLE, the same print packets and order including
`0103`, image 1260×840.

| App-side claim | Firmware evidence | Effect |
|---|---|---|
| Same Link print commands | handler names for print download start/end/cancel, printer capability, image info, reject film cover | consistent (**likely**) |
| `0103` is sent before START | a BLE connect-interval handler and a connection-parameter update call exist | consistent (**likely**) |
| Image 1260×840 JPEG | the camera decodes the JPEG and crop/rotate/scales it before its own LUT | consistent, geometry tolerance **unknown** |
| No pairing step in the app's print task | the camera has an LE pairing database, numeric comparison and a pair-registration menu | **new caveat**: a host may have to be registered/paired on the camera first. This is not established either way |

The verdict stands: no new print packet is needed. The firmware adds one open
item, camera-side pairing, which a Windows BLE client without pairing
(btleplug, [connection.md](../apps/connection.md)) may hit. Hardware is needed to
decide it.
