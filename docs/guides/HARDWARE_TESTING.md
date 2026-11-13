# Deferred hardware qualification and capture guide

This is a reference for the later hardware phase, not a request to run these
commands now. The active task is to complete app-side RE, and user tests will be
requested only after that. See [APP_RE.md](../re/apps/APP_RE.md) for the current work frontier.

## Test the Rust implementation

Record the model printed on the device, its firmware version if known, computer
OS and Bluetooth adapter. Close the Fujifilm app so it does not hold the link.

```sh
outstax scan
outstax --device 'EXACT_ID' --trace status.jsonl status
outstax --device 'EXACT_ID' identity 1
outstax --device 'EXACT_ID' identity 4
```

Save the status trace and console output. Check the reported dimensions and
byte limit before uploading a photo. Do not guess an `(ANDROID)` endpoint's
BLE address by changing its MAC prefix. Select the discovered BLE identity
instead.

Choose a test image with asymmetric corners, an upright label and neutral
gray/color patches. Print it with the official app first to establish a
reference, then close that app and test Rust:

```sh
outstax --device 'EXACT_ID' --trace upload.jsonl print test.png
outstax --device 'EXACT_ID' --trace print.jsonl print test.png --commit
```

The upload-only operation ends with CANCEL and should not consume film. Report
whether the device remained responsive. For the physical print, record its
orientation, margins, color differences, visible banding and whether exactly
one sheet was ejected. The CLI reports acceptance and device-reported completion
separately, and neither is optical proof of successful exposure/ejection. If
PRINT times out, inspect the device before starting another job.

An optional conservative JPEG cap can help investigate image-size limits:
`--max-bytes 56320`. This is an experiment, not a known universal printer limit.

## Capture the current official Android app

1. Install/update the corresponding app through Google Play on the phone.
2. Enable Android Developer options and Bluetooth HCI snoop logging. Restart
   Bluetooth so logging takes effect. Phone manufacturers differ in log export.
3. Capture one action at a time: connect/status, Natural print, Rich print,
   Design Print on Link+, and a settings change. Use the same source image.
4. Obtain the Bluetooth HCI log through a bug report or the phone's supported
   developer export. AOSP documents `btsnooz.py` extraction from bug reports.
5. Open the log in Wireshark. For BLE, locate the print service and track ATT
   writes/notifications for its characteristics. For Android Classic transport,
   follow the RFCOMM byte stream. The protocol frame markers remain useful.
6. Export only the relevant connection and action for analysis. Record app
   version, package signer, printer model and firmware with the capture.

The script [`scripts/acquire/pull_android_apps.py`](../../scripts/acquire/pull_android_apps.py) pulls installed base and split APKs
using an already-authorized ADB device. It does not install apps or change the
phone. Specify `--serial` when multiple devices are attached.

```sh
python3 scripts/acquire/pull_android_apps.py --output acquired-from-phone
```

Compare these signing-certificate fingerprints with [`data/acquisition/acquisition.json`](../../data/acquisition/acquisition.json).
The mirror packages' signatures verified cryptographically, but no live Play
installation was available here to independently authenticate their publisher
certificate or prove regional release freshness.

For a reproducible third-party mirror acquisition, use
`python3 scripts/acquire/acquire_mirror.py --output mirror-inputs --native`.
It reads the archive directory with HTTP range requests, downloads each base
APK and optionally its ARM64 split, and records hashes. It does not execute the
apps. Inspect the downloaded manifest versions and verify signatures, because
the mirror's `latest` URL is not an official release-freshness guarantee.

An independent verifier invocation used here was:

```sh
java --class-path apksig-8.8.2.jar scripts/acquire/VerifyApk.java path/to/base.apk
```

The verifier library comes from Google's Android Maven repository:
`https://dl.google.com/dl/android/maven2/com/android/tools/build/apksig/8.8.2/apksig-8.8.2.jar`.
Use a Java installation supporting source-file launch (Java 17 was used here).

## Without an Android phone

The Rust status and upload tests still work on a computer with Bluetooth.
iOS Bluetooth logging may be available through Apple's developer diagnostic
tools, depending on the device and access to a Mac. A Linux computer can record
its own Bluetooth session with `btmon`. That captures the Rust implementation,
not what the official iOS/Android app would have sent.

## Useful evidence to return

- Exact model, firmware, computer OS, Rust client version and Bluetooth adapter.
- `scan` output and `status.jsonl` first.
- Official-app version, relevant HCI connection, one action and its outcome.
- For a print comparison, the source test image and a photo/scan of the result.

Packet traces from printing contain the transferred photo. Full Android bug
reports contain unrelated device information, and a connection-specific
Bluetooth export is sufficient for this work.

Reference: https://source.android.com/docs/core/connect/bluetooth/verifying_debugging

