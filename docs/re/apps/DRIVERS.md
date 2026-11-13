# Official desktop-driver corroboration

All four official WIDE/WIDE2 Windows MSI and macOS PKG packages were acquired
without running their installers. Package hashes/URLs are in acquisition.json
and research provenance. [`data/drivers/driver-files.json`](../../../data/drivers/driver-files.json) inventories 284 extracted
files by size and SHA-256. Vendor bytes are not included in this project.

The Windows installers contain InstaxBLE.dll. The WIDE2 copy is 113,664 bytes
(`filAF61972469244DEDBC294BA28F413DFF` within the cabinet). Its .NET metadata and
CIL were inspected directly with dnfile/dncil, but the IL was not executed. The
older WIDE copy is 109,056 bytes. `windows-driver-identities.json` maps cabinet
IDs to assembly names and version-resource identities.

The WIDE2 driver independently corroborates these Android findings:

- Request/response signatures Ab/aB, complement-of-sum checksum and big-endian
  frame fields (`BleRequest.Build`, `BleResponse.ParseResponse`).
- The same service/write/notify UUIDs and BLE connection parameters 12,12,0,400
  (`BleConstants..cctor`, `PrintTask.SendBleConnectionSettingsCommand`).
- Eight-byte print START and Rich/LUT option bit 08
  (`PrintTask.SendDownloadStartRequest`).
- Four-byte big-endian block size in START ACK
  (`PrintTask.HandleDownloadStartResponse`).
- Big-endian sequence and final DATA block zero padding
  (`PrintTask.SendNextImageFrame`).
- Status checks before transfer and after END, before physical PRINT
  (`HandlePrePrintCheckResponse`, `HandlePostDownloadEndCheck`).
- Separate polling/completion states, busy/cover/film/battery gates, and explicit
  timeout handling (`CheckPrinterCondition`, `SchedulePrinterInfoPoll`).

The macOS applications are Swift/Mach-O. Their symbols/constant strings confirm
CoreBluetooth, the same three UUIDs, shared print command names and NOW_PRINTING
handling. The WIDE2 binary contains model FI049. Both applications bundle the same
64 MiB 20210701_3DLUT.bin asset as Android (asset hashes in the inventory).
Symbol/constant inspection alone is weaker evidence than the Windows IL
reconstruction, and this note does not claim that all Swift methods have been
reverse engineered.

The desktop drivers expose the OS print spooler and PDF/raster conversion around
the Link protocol. OS spooler integration is a separate application feature.
Nothing here points to a different WIDE2 image-transfer protocol. Installation,
native driver execution and real Bluetooth operation were not tested.
