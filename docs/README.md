# Documentation index

All paths are relative to this folder. Vendor material and the local analysis
workspace are kept outside the project (material/…, research/…) and are referred
to by name and SHA-256 only.

## Reading order

1. **Start here.** The [project README](../README.md): build, CLI, desktop app,
   image modes and project map.
2. **Status.** What works and what is missing.
   - [status/COVERAGE.md](status/COVERAGE.md): printer, feature and platform matrices. Read before calling any model supported.
   - [status/PLAN.md](status/PLAN.md): work plan and the state of each step.
   - [status/VERIFICATION.md](status/VERIFICATION.md): checks run and their results.
3. **Protocol.**
   - [protocol/PROTOCOL.md](protocol/PROTOCOL.md): transport, envelope and packet/field specification.
   - [protocol/unused-commands.md](protocol/unused-commands.md): backlog of opcodes, selectors and modes no app sends.
   - [protocol/HTTP.md](protocol/HTTP.md): cloud and host-side features in brief.
4. **Reverse engineering by topic.**
   - [re/apps/APP_RE.md](re/apps/APP_RE.md): main app RE record, with the current state of every item.
   - [re/apps/protocol-closure.md](re/apps/protocol-closure.md): enum closure, post-connect sequences, head calibration, settings flows.
   - [re/apps/connection.md](re/apps/connection.md): BLE connection lifecycle, pacing, keep-alive, renaming.
   - [re/apps/creative.md](re/apps/creative.md): collage, frames, text, QR, camera modes, history and reprint.
   - [re/apps/cloud.md](re/apps/cloud.md): authentication, upload/download and per-feature server dependence.
   - [re/apps/enhancement.md](re/apps/enhancement.md): FDIA auto-correction and the optional unsharp path.
   - [re/apps/hybrid-cameras.md](re/apps/hybrid-cameras.md): mini Evo, Evo Cinema, LiPlay/LiPlay+, WIDE Evo.
   - [re/apps/DRIVERS.md](re/apps/DRIVERS.md): official desktop-driver corroboration.
   - [re/firmware/firmware.md](re/firmware/firmware.md): Link printer firmware dispatcher, upload, BLE, power, name.
   - [re/firmware/mini-link-0104.md](re/firmware/mini-link-0104.md): original mini Link `0104` firmware (external analysis).
   - [re/firmware/firmware-contents.md](re/firmware/firmware-contents.md): firmware bundle inventory and contents addendum.
   - From the external multi-firmware analysis (2026-11-12):
     [re/firmware/square.md](re/firmware/square.md) (SQUARE Link),
     [re/firmware/wide2.md](re/firmware/wide2.md) (Link WIDE 2),
     [re/firmware/renesas-mini2-mini3.md](re/firmware/renesas-mini2-mini3.md) (mini Link 2/3 Renesas RX builds),
     [re/firmware/wide-evo.md](re/firmware/wide-evo.md) (WIDE Evo Linux firmware).
   - [re/firmware/README-implications.md](re/firmware/README-implications.md): client implications of all firmware findings.
5. **Evidence.** [evidence/](evidence/) holds build, lint, test, target-check and
   simulator logs referenced by the status documents.
6. **Guides.**
   - [guides/HARDWARE_TESTING.md](guides/HARDWARE_TESTING.md): deferred hardware qualification and capture guide.
   - [guides/TROUBLESHOOTING.md](guides/TROUBLESHOOTING.md): logging and trace how-to.

## Data and scripts

Machine-readable catalogs are under [`../data`](../data), by topic:
[`acquisition`](../data/acquisition) (app versions, hashes, signers),
[`protocol`](../data/protocol) (command catalog, enum closure, golden frames),
[`firmware`](../data/firmware) (dispatch tables, assets, HEX layouts, Link 3 BT components, Renesas RX registries, app CPU routing, SQUARE/WIDE 2 protocol evidence),
[`apps`](../data/apps) (native inputs, symbols, enhancement assets, source hashes),
[`drivers`](../data/drivers) and [`http`](../data/http).
Only [`verification.json`](../data/verification.json) stays at the root.
`outstax commands` prints [`data/protocol/commands.json`](../data/protocol/commands.json).

Scripts are under [`../scripts`](../scripts): `extract_lut.py` (user-facing),
[`acquire`](../scripts/acquire), [`build`](../scripts/build) and
[`research`](../scripts/research/README.md), with external firmware tools in
[`research/firmware`](../scripts/research/firmware/README.md).
