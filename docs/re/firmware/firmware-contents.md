# Firmware bundle and contents addendum: summary

Summary of two external analyses kept outside the project:

- the firmware research bundle report (research/instax-firmware-research/REPORT.md,
  SHA-256 `4559befd8b243e7893203a09fca32a9e7ee0aa6a75b2a5977a04129f27621ca9`,
  research date 2026-11-11), whose 33 firmware files are in
  material/firmware/bundle-2026-11-11
- the contents addendum (research/external/contents-addendum/README.md,
  SHA-256 `81b4d7abeb2a5cbcac2e24a4041c4acda51d3fc5200228cdc7a87d93d24c885a`,
  2026-11-12)

A later external multi-firmware analysis, 2026-11-12
(research/external/instax-multi-firmware-analysis, report `report/multi/REPORT.md`,
SHA-256 `31f913997398dde0045201932bb7dc61c1304619b25c4481ed1f0c5527c9f35b`),
supersedes two points below. Its vendor inputs and derived file systems are in
material/firmware/multi-analysis-2026-11-12. See the notes marked **Update** and
[square.md](square.md), [wide2.md](wide2.md),
[renesas-mini2-mini3.md](renesas-mini2-mini3.md) and [wide-evo.md](wide-evo.md).

No firmware, vendor resource or decompiled code is copied here. Neither analysis
executed, emulated or flashed firmware. Hashes identify the analysed bytes. They
do not establish authenticity, signature validity or device acceptance. The
Link main-controller dispatch analysis is in [firmware.md](firmware.md), and the
original mini Link `0104` analysis is in [mini-link-0104.md](mini-link-0104.md).

## Bundle inventory

The bundle holds 33 unique files (31 binary/HEX payloads, 2 configuration files)
from 34 retrieved instances. Identical bytes are stored once. Sources were the
public APKPure XAPK entry points and public Fujifilm firmware pages. APK
publisher signatures were not reverified in that pass, and mirror "latest" does
not prove regional release freshness.

| App package | Version / code | Embedded update payloads |
|---|---|---|
| `com.fujifilm.instaxminiLink` | 7.0.6 / 336 | six main HEX images, plus the Link 3 Bluetooth component set |
| `com.fujifilm.instaxSquareLink` | 1.6.2 / 60 | `FW_0102.DAT` |
| `com.fujifilm.instaxLinkWide` | 2.0.0 / 204 | `BO22/0101.hex`, `BO49/F100.DAT` |
| `com.fujifilm.instaxBo19` | 2.0.5 / 209 | `Firmware/FW_0109.DAT`, `EC_Firmware/FW_0102.DAT` |
| `com.fujifilm.instaxminiLiPlay` | 7.2.0 / 194 | `Firmware/FW_0309.DAT`, `Firmware/LiplayPlus/FW_0101.DAT` |
| `com.fujifilm.instaxWideEvo` | 1.1.3 / 108 | `firmware/unsigned/FW_0103.DAT`, `firmware/signed/FW_0103.DAT` |

## Link printers

| Asset (in SharedAssets.apk) | Model | Bytes | SHA-256 |
|---|---|---:|---|
| `assets/fw/BOD/0104.hex` | mini Link | 2,580,791 | `f3d3e5d2b981555c6aa6b059504bc1416d17303dede8e07c2fec057fce91b0f7` |
| `assets/fw/BO23/STMicro/0100.hex` | mini Link 2 | 2,523,416 | `334fc7c9af1c938573b49d445a369bba6c31c8d246f21dbfb87065b6d45ef698` |
| `assets/fw/BO23/RenesasRX/0100.hex` | mini Link 2 | 1,932,140 | `12a678b575f1f6356a594bc7a2a07faa3aeec269e685ce4c8b95cbfa2018f6ed` |
| `assets/fw/LINK3/STMicro/0102.hex` | mini Link 3 | 2,523,428 | `9dd80bb78106df3b5e247b1597a07735c99d322bb054280267dfbb4ba580675b` |
| `assets/fw/LINK3/RenesasRX/0102.hex` | mini Link 3 | 1,932,196 | `81de808fe0a0fa58f6b3426365848fda68de8a5e6792da4ec758fca36ad7e724` |
| `assets/fw/LINKPLUS/STMicro/0100.hex` | mini Link+ | 2,523,416 | `4bd399c6a4477fff83bca961724033a7bb594a178f6d6bff816fd4c5ee9bb63f` |
| `assets/Firmware/FW_0102.DAT` | SQUARE Link | 2,486,908 | `9f6e1315e7d822bb8810e34ae691ffa93138043d8d6240ce378bd20d43ddac94` |
| `assets/Firmware/BO22/0101.hex` | Link WIDE | 2,523,428 | `8503723303489b28763f417194db6dd9e780378f41321e356719d615f856ed79` |
| `assets/Firmware/BO49/F100.DAT` | Link WIDE 2 | 3,734,383 | `16187d3f6434e3157b7ca42e919ddb505dc4bdba3527d21ad408f43291519bb4` |

Model associations are the project's source-derived path mapping, not physical
verification. Equal HEX base names belong to different models and CPUs, so
selection needs the full path or hash plus the device identity.

- The seven HEX files have valid record checksums and 917,504 contiguous data
  bytes each. STM32 layouts start at `0x08020000`, Renesas layouts at
  `0xfff00000`. The two Renesas files have no EOF record. Type-05 records are
  metadata, not transfer data.
- The SQUARE and WIDE 2 DAT files are compressed payloads that contain the
  `SUNP BURN FILE` marker. Their resource file systems were not enumerated.
  **Update:** each is one raw LZ4 block holding a MIPS32 SUNP image with two
  **FAT12** volumes (SQUARE 73 files, WIDE 2 58 files) and an RSA-2048
  signature that verifies with the bundled public key. WIDE 2 also carries an
  11-component Realtek Bluetooth package ([square.md](square.md), [wide2.md](wide2.md)).
- **Update:** the two Renesas RX images were disassembled (GNU binutils RX),
  and their command tables match the STM32 builds of the same model
  ([renesas-mini2-mini3.md](renesas-mini2-mini3.md)).
- The Link 3 Bluetooth set is selected by `cfg_Ver1012.cfg`
  (`1593eca7ab8a36168be76f6d9c9c0a8046070f9a53729d799a0af3ce68b249fd`): ten
  components, IDs `0100 0200 0300 0400 0410 0500 0602 0700 0800 0a01` (CFG,
  patch, audio, APP, DSP config MT, DSP system, DSP application, FSBL, OTA
  header, boot patch). An extra non-MT DSP configuration is bundled but not
  selected. Readable headers name `RTL8753BFR`, but some patch/audio headers name
  `RTL8763BO` and the boot patch names `RTL8773B`. Those strings do not prove which
  chips a printer contains. Files labelled Encrypted/Sign were not validated.
  Component names, sizes and checksums are in
  [`data/firmware/bt-firmware-components.json`](../../../data/firmware/bt-firmware-components.json).

## Hybrid cameras

| Origin | Bytes | SHA-256 |
|---|---:|---|
| mini Evo app `assets/Firmware/FW_0109.DAT` | 12,937,099 | `9fa2e173dc620358b37c868d575c9b1d97f2defd4a03657b014aef9f5a1bf250` |
| mini Evo official 1.09 `FPUPDATE.DAT` | 97,328,468 | `4f412d08688443212c2cf350586a8c01d43d467c580eb19389a82b40929a93ed` |
| mini Evo Cinema app `assets/EC_Firmware/FW_0102.DAT` | 43,681,443 | `8e4f8671f6b0b3ede8eea8e539aef18ca25104b09701e350717a688d13f5f006` |
| mini Evo Cinema official 1.02 `FPUPDATE.DAT` | 163,431,843 | `094d275f74d6c65003909055da2846a4bea2ab6b50a4d48b80291f1242de1d48` |
| mini LiPlay app `assets/Firmware/FW_0309.DAT` | 11,764,631 | `503b62de5a958809610ae08b80e20c8567d98568d4b836c4af04eca93493662d` |
| mini LiPlay decoded (= official 3.09 download) | 58,412,324 | `8301ef6d4039f78da1195ad2ae31f64e3a798a432fad3f6905b313bdcb7a8ca2` |
| mini LiPlay+ app `assets/Firmware/LiplayPlus/FW_0101.DAT` | 31,317,310 | `2359772602edb1c419beb33e9030d97fbf597066ad0cc402c0bacee068d7e69d` |
| mini LiPlay+ decoded (= official 1.01 download) | 98,740,772 | `01fb6914cb0f5a6d4fe913670518a21e6e3e0de7557f61125d6ef0d3d0333cf7` |
| WIDE Evo app `firmware/unsigned/FW_0103.DAT` (= official Ver100 download) | 51,113,791 | `470c82342b590402a4828636426d468ab726c3339a5891f35d60c54f941be71f` |
| WIDE Evo app `firmware/signed/FW_0103.DAT` | 51,115,071 | `e872f08953615b7abe7ba9b5492becf9027be8eece434501abd38d7a170ddade` |
| WIDE Evo official Ver101 download | 51,115,071 | `61f48235177db3fcc2e6a799363fcdebed64c35f6969ad146a3e69621d0905e8` |

- **App update paths.** In all three camera apps the update handlers open the
  bundled asset, read it unchanged with write address 0 and pass it to the camera
  service. No firmware HTTP fetch was found in the reviewed methods.
- **mini Evo.** The app payload is a raw LZ4 block. Decoded, it has the same size
  as the official image (SHA-256
  `a1e39a71c81f7c2a8a1348d2410119e568592bf6e8c2232a58c30efb2bd882e4`) but is not
  byte-identical: 69,427,533 positions differ, from offset 508. Equal version
  labels do not make them interchangeable. The Cinema payload does not decode as
  raw LZ4, and its encoding was not identified.
- **mini LiPlay / LiPlay+.** Both app payloads are raw LZ4 blocks whose decoded
  bytes equal the official microSD images. The host transfers the compressed app
  payload, not the decoded image.
- **WIDE Evo.** The app picks the unsigned file for installed revision below
  `0101`, the signed file for `0101` up to below `0103`, and nothing from `0103`.
  Both signed files are the unsigned payload plus 1,280 appended bytes. The app
  and web signed files differ in 253 bytes within the first 256 appended bytes.
  This looks like a signature trailer, but its algorithm and keys were not
  established.

## Contents addendum

- **mini Evo official 1.09 image** (SHA-256 above). It has two FAT16 volumes
  (OEM string `ICATCH`, 512-byte sectors). Volume A is at offset `0x000B2B20`
  (72 MiB, 517 files) and volume B at `0x048B2B20` (15 MiB, 5 files). Volume A holds UI and
  print resources by type: 295 PNG, 21 TXT, 9 FDL and 5 SFN fonts, 8 WAV,
  6 JPG, 8 YUV, 11 `.Y`, 3 `.LZ4` and 154 BIN files. The largest is a
  48 MiB (50,331,648-byte) `PRT3DLUT.BIN`, whose name suggests a 3D print
  colour table, though its mapping was not traced. One PEM public key file is
  present. Its use was not traced, and no private key was recovered. Volume B holds print
  history, log and UI data files from the distributed image (two are empty), not
  user data. The program sections outside the FAT volumes were not mapped.
  The catalog of all 522 paths (sizes and first 32 bytes, no payloads) is
  research/external/contents-addendum/mini-evo-file-catalog.json, SHA-256
  `4ec1c036767ef86d8f0c3883d205704f64377d73f3c19f88edb19da4cf858c78`, made with
  `fat_contents.py`
  (`64b917bda88a77b4fcd2dc64964895d3b970081b9d5f950b4d89ac02c7477ed9`).
- **LiPlay / LiPlay+.** SUNP-family images. An earlier pass found two FAT16
  volumes in each. No file-by-file catalog was made.
- **WIDE Evo / Evo Cinema.** Embedded-Linux indicators: Novatek NA51055,
  U-Boot, BusyBox, Buildroot (2019.05.2 on WIDE Evo, 2022.08.3 on Cinema), a
  FreeRTOS source path, driver-module names, UBI/UBIFS and squashfs markers.
  The raw DAT is not a mountable disk image. ELF candidates failed a header
  check, and the container header did not match the documented Novatek format.
  No root file system, kernel or executable was extracted. String offsets for
  the WIDE Evo unsigned image (`470c8234…`) are in
  research/external/contents-addendum/wide-observations.json
  (`f2c990f99a9498804212c2b9ebc2ccb79d13237bb0abad41c322584082abcbdc`).
  **Update (WIDE Evo):** the multi-firmware analysis found the correct encoding
  (128-byte prefix plus 953 dependent LZ4 blocks), recovered eight partitions,
  the kernel and initramfs, and three UBI/UBIFS images (1,542 files, all CRCs
  valid), and traced the print path to the `ab_printer` driver
  ([wide-evo.md](wide-evo.md)). The "no root file system extracted" statement
  above is superseded. The Ver100 file is an exact prefix of Ver101.

## Limits

The work was limited to static inspection, targeted JADX decompilation, hash
comparisons and documented decompression. Updater transport for the cameras, bootloader validation,
rollback/recovery, signature checks and per-region variants are unverified.
The scripts used are copied in
[`scripts/research/firmware`](../../../scripts/research/firmware/README.md).
