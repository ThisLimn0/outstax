# mini Link 2 / mini Link 3 Renesas RX builds

Source: external multi-firmware analysis, 2026-11-12. The package,
`instax-multi-firmware-analysis`, is kept outside the project at
research/external/instax-multi-firmware-analysis. Its Renesas chapter is
`report/multi/renesas.md` (SHA-256
`82078df2ff5365fbdb38bf53a0278a5c4ae61b80f9615ea9e986df25999a9037`), together with
`renesas-mini2-registry.md` and `renesas-mini3-registry.md`. The full report is
`report/multi/REPORT.md` (SHA-256
`31f913997398dde0045201932bb7dc61c1304619b25c4481ed1f0c5527c9f35b`). All package
files matched its SHA256SUMS. Disassembly used GNU binutils 2.44 `rx-elf-objdump`
(the binutils source is in material/references, outside the project). The work
was static only, and no image was executed or flashed.

Labels are as in [square.md](square.md): **recovered**, **likely** and **unknown**.
The STM32 comparison columns are ours, from
[`data/firmware/firmware-dispatch.json`](../../../data/firmware/firmware-dispatch.json) and [firmware.md](firmware.md).

Cleaned data:
- [`data/firmware/renesas-mini2.json`](../../../data/firmware/renesas-mini2.json) and [`data/firmware/renesas-mini3.json`](../../../data/firmware/renesas-mini3.json): full descriptor
  registries, task/RTOS strings, sentinels (firmware bytes removed)
- [`data/firmware/dex-routing.json`](../../../data/firmware/dex-routing.json): the app's model/CPU → asset routing, reduced to
  call facts (which methods, types and strings each routing method refers to), with no bytecode.

## Which printers run these images

- The mini Link app 7.0.6 routes **mini Link 2 → `fw/BO23`** and **mini Link 3 →
  `fw/LINK3`**, and a CPU enum `RENESAS` / `ST_MICRO` selects the `RenesasRX`
  or `STMicro` subdirectory (DEX code offsets `0x2e8b80`, `0x3c64f0`, `0x3c8bbc`,
  `0x3c7af8`, `0x3c7d0c`, `0x1e7ff0`, `0x3c7a50`). **recovered** (host routing)
- How a printer reports its CPU is not in the package. Our
  [APP_RE.md](../apps/APP_RE.md) has it: the eighth byte of `3010/1`, 0 = STMicro, 1 =
  Renesas (`Initialization::cpu_type`). So a given mini Link 2 or 3 runs
  **either** the STM32 image of [firmware.md](firmware.md) **or** the RX image
  here. Which one a user's unit has is only known from that byte.
- The exact RX part, flash layout and reset/boot region are **unknown**. The
  fixed reset vector `0xFFFFFFFC` lies outside the supplied interval.

## Images

| | BO23 (mini Link 2) | LINK3 (mini Link 3) |
|---|---|---|
| HEX SHA-256 | `12a678b5…18f6ed` | `81de808f…7e724` |
| Mapped bytes / interval | 917,504, `0xFFF00000`–`0xFFFE0000` | same |
| Footer version (LE at `0xFFFDFFF4`) | `0x0100` | `0x0102` |
| Header word +4 (not a version) | 1 | 6 |
| Model string | `FI023` | `FI033` |
| `0000` handshake payload | `01 00 01 00 00 00 00 00` | `01 00 02 00 00 00 00 00` |
| Descriptor table | `0xFFF1086C`, 34 records / 32 opcodes | `0xFFF1D3EC`, 46 records / 43 opcodes |

All **recovered**. Neither HEX file has an EOF record (vendor property, already
noted in [firmware-contents.md](firmware-contents.md)).

## Comparison with the STM32 builds of the same model

**Command table.** The RX descriptor has the STM32 layout (opcode u16,
frame length u16, four flag bytes, handler, continuation). *Our comparison:*
for each model, the RX table and the STM32 table have the **same opcode set,
the same frame lengths, the same any-length/`f5`/`f6` flags and the same
continuation-present pattern**, entry for entry (32 opcodes for BO23 vs
`link2_stm`, 43 for LINK3 vs `link3_stm`). The only difference is how variable-length
rows are stored (length 0 on RX, none in our JSON). **recovered**

| Topic | RX (external analysis) | STM32 (firmware.md) | Agreement |
|---|---|---|---|
| Wire framing | `Ab`, BE16 length, BE16 opcode, payload, checksum. Length < `0x5140`, sum = `0xff` | same, same limit | agree |
| Dispatch | descriptor scan, `jsr` to handler at +8, continuation at +12 | same | agree |
| `0002/0` capability | 600, 800, `0x25800` (153,600) on both | 600×800, 153,600 | agree |
| `1000` START | type 2, flags checked, length 1..153,600. Reply block size 900 | same | agree |
| START byte2 (option2), mini Link 2 | written to the same global as byte3, then overwritten and not checked | read and ignored | agree |
| START byte2, mini Link 3 | any nonzero value fails (`0xFFF6B14E`). The 307,200 (`0x4B000`) branch is unreachable on this path | "Link 3 requires 0" | agree. The larger branch is not a feature |
| `1001` DATA | 911-byte frame. Copies `min(900, remaining)` and checks against the saved last sequence and timeout/state | 911 bytes, no next-expected check | agree on framing. The RX sequence rule is stated more loosely (**likely** the same last-index bound) |
| One-block upload | `if / else if` markers: sequence 0 must be sent twice before END | same in `0104` ([mini-link-0104.md](mini-link-0104.md)) | agree (static) |
| `1002` END | needs both markers and copies in `0x1400` batches | needs both markers | agree |
| `2001` FW block | BO23 1024 (`0xFFF58B27`), LINK3 900 (`0xFFF6C021`) | Link 2 1024, Link 3 900 | agree |
| `20a0`–`20b1` BT updater | LINK3 only. `20a0` parses `u16, u32, u32` + 2 unassigned bytes | Link 3 only | agree |
| `3006`/`3007` and `3008`/`3020` | BO23 only / LINK3 only | Link 2 only / Link 3 (and Link+) | agree |

**Not covered for RX** (all **unknown**): the transfer-timer constants and
reload rule, the `0102` timer validation and factory defaults, the `0103`
bounds, the silent `0x90`/`0x91` and internal-`0x13` paths, the BT task
state machine, the advertised name and suffix, and any pairing logic.

**Hints only** (strings, **likely**): both images have `ID_BT_DRV_TASK`,
`ID_BT_TASK`, `ID_BT_ANA_TASK`, `ID_T10MS_TASK`, `ID_PRINT_TASK` and
`ID_USBD_TASK` names (an ITRON-style task set), and `INSTAX-` at three
addresses each (BO23 `0xFFF054BF`, `0xFFF054D3`, `0xFFF14264`, and LINK3
`0xFFF07BBF`, `0xFFF07BCF`, `0xFFF23E1C`). The `T10MS` task matches the 10 ms
tick that firmware.md assumes for the STM32 timer.

## What this means for the client and for link drops

- The RX builds accept the same frames and limits our client already sends
  (START `[2, opt, 0, 0]`, 911-byte DATA, 900 block). The double-density
  option2 bit is gated by the advertised `options2`, which LINK3 RX rejects
  anyway.
- The STM32 findings about BLE (no MCU pairing timer, `0103` forwarding, no
  disconnect on a stalled upload) **were not re-derived for RX**. If the
  user's mini Link 2/3 reports CPU type 1 (Renesas), firmware.md's §3 does
  not apply to it as evidence.
- Details and proposals: [README-implications.md](README-implications.md).
