# Hardware testing guide

How to test OutStax on a printer and what to record, so the result can be added
to the model table in the README. Start with a test upload, which uses no film.

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

## Useful evidence to return

- Exact model, firmware, computer OS, Rust client version and Bluetooth adapter.
- `scan` output and `status.jsonl` first.
- For a print comparison, the source test image and a photo/scan of the result.

Packet traces from printing contain the transferred photo, so only share them
if you are happy to share that photo.

