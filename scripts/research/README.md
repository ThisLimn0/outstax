# Reproduce static analysis

These scripts operate on locally acquired vendor inputs and never install or
execute the apps or drivers. Rust application builds do not need these Python
dependencies. Install `requirements.txt` into a virtual environment, and note
that libarchive-c also needs the system libarchive shared library. JADX 1.5.6 and Google's apksig
8.8.2 were used separately for decompilation and APK integrity checks.

1. Acquire the three applications with `../acquire/acquire_mirror.py --output ...
   --native --assets`, or use `../acquire/pull_android_apps.py` with an authorized device.
   Mirror latest is not proof of official release freshness.
2. Use `extract_apk_inputs.py APK_ROOT --output NEW_DIRECTORY` to extract DEX,
   selected ARM64 libraries, firmware and image-processing assets.
3. Decompile the DEX files into `RESEARCH_ROOT/decompiled/{mini,square,wide}/sources`.
   Preserve package directories. Compare results with the input APK fingerprints
   in [`data/acquisition/acquisition.json`](../../data/acquisition/acquisition.json). The published evidence uses JADX's source paths,
   and R8 obfuscation/package names may change between versions.
4. To recreate the packet catalog/source hash inventory, run
   `enrich_protocol_catalog.py --research-root RESEARCH_ROOT`. The layout map is
   reviewed technical facts rather than inferred automatically from opcodes.
5. Retrieve the four exact official driver URLs recorded in `data/acquisition/acquisition.json` into
   `RESEARCH_ROOT/acquisition`. Run `extract_msi.py`, then `extract_pkg.py`, both
   with `--research-root RESEARCH_ROOT`. The latter uses a direct XAR parser
   because the environment's libarchive XAR pathname/data behavior was unsuitable.
6. `dump_driver_il.py` records the selected WIDE2 InstaxBLE assembly's CIL for
   local inspection. Its cabinet ID is pinned to the acquired package, so inspect
   the metadata again before using it on another release. Decompiled vendor code and
   extracted vendor assets are local analysis inputs, not part of this source
   distribution.

All paths default to the project root and the `research` workspace beside it
(`../research` relative to the project folder, outside the project). Override
`--project-root`/`--research-root` when using another layout. Catalogs are written
to the topic folders under `data/` (`protocol`, `apps`, `drivers`). Reports, catalogs and
input fingerprints are included in the project archive. The third-party APKs,
firmware and native libraries can be reproduced from the recorded inputs.

External firmware-analysis tools copied from outside the project are in
[firmware/](firmware/README.md).
