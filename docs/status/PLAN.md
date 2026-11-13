# Work plan and delivery requirements

Goal: an independent cross-platform Rust application for the current standalone
INSTAX printers, with source-grounded protocol behavior and a clear account of
which original-app features have been reproduced. Older Link printers remain
compatibility targets. Hybrid cameras and legacy SHARE Wi-Fi are distinct
qualification/protocol work, not implied by shared enum names.

1. Acquire the latest obtainable Mini/Square/Wide apps, record versions and
   provenance, verify embedded signatures, extract DEX/assets/native libraries.
   **Done for the recorded Android mirror versions. Official per-region latest
   and independent signer identity remain unproven.**
2. Complete app-side RE: trace each reachable printer command from caller through
   packet builder, response parser, model/version gates, errors and transitions.
   Trace image processing and relevant cloud/creative paths. Use desktop drivers
   as independent corroboration. Explicitly close unused library enums without
   inventing firmware behavior. **Static pass done (2026-11-12). Every opcode is
   classified (36 called, 38 never-called, 0 unknown), and the initialization,
   head calibration, settings, connection lifecycle, creative, cloud,
   hybrid-camera and printer-firmware findings are recorded. Still open: the FDIA enhancement engine (not
   recoverable statically), unsharp numerical comparison, iOS, and items that
   need hardware or live services. See the state table in
   [APP_RE.md](../re/apps/APP_RE.md#state-of-app-side-re-2026-11-12).**
3. Integrate the recovered behavior into transport-independent Rust types,
   transports, image processing, a CLI and native application. **The core app
   exists. Several complete-app features and model-specific integration remain.**
4. Run repository checks as part of implementation. Request user printer tests
   only after the app-side RE phase has been completed. The user has a printer,
   but that does not replace unfinished source analysis. **No user test request is
   pending.**
5. Qualify one physical printer, then every model/firmware/OS combination claimed
   as supported. Compare official and independent output, then exercise
   failures, completion, settings and recovery. **Deferred.**
6. Build native release packages for supported operating systems and finish the
   feature-parity matrix. **Linux CLI/GUI builds and Windows/macOS target checks
   exist. Native cross-platform installers and mobile shells remain.**
7. When finished, deliver **the entire project folder in one ZIP**, including
   source, manifests/lockfile, build instructions, scripts, documentation, RE
   findings/evidence, included assets and available release binaries. Preserve
   hidden project files/CI configuration. Keep reproducible compiler caches and
   temporary analysis products outside the clean project directory. The package
   script verifies that every folder entry is present in the ZIP.

A codec, opcode list, simulator run or successful compile does not establish
complete RE, feature parity, physical print success or all-model support. Those
claims require their corresponding evidence. A research checkpoint must not be
labeled the finished replacement application.
