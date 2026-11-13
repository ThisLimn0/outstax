# Cloud and host-side features

The printer image-transfer path is local Bluetooth. QR/video sharing, remote
network prints, account registration, analytics and Pinterest are host/cloud
features. None of them adds an encryption or authentication field to Link image
frames, so a replacement can print locally without implementing those services.
It will not have feature parity with the original applications.

[`data/http/http-endpoints.json`](../../data/http/http-endpoints.json) inventories 46 literal Retrofit route annotations
found in the acquired apps. It is a route inventory, not a complete service
specification: dynamic URL GET/PUT operations and constants used as annotation
arguments are additional cases. [`data/http/http-models.json`](../../data/http/http-models.json) records fields from 70
request/response model classes, with source paths and hashes. Field names/types and
candidate serialization annotations still need per-route reconciliation before
anyone implements these online features. No vendor service requests were sent.
Authentication values, API keys and signed upload URLs are not distributed.

| Application | Main source routes | Host-side flow |
|---|---|---|
| Mini 7.0.6 | user/authorizeSmartphone, user/registerCamera, data/getUploadUrls, data/updateImageUpload, data/updateVideoUpload, data/updateVideoDownloadCount | Register/authorize host, obtain upload URLs, transfer media, acknowledge upload/download bookkeeping |
| Square 1.6.2 | users, users/cameras, users/analytics, arprints, arprints/download, networkprints, networkprints/download, networkprints/registerNotification, networkprints/pushRequiredUser | Register host/printer, create upload records, transfer media via returned URLs, update records, retrieve/download remote/AR prints and notification state |
| Wide 2.0.0 | user/login, user/registerDevice, user/registerCamera, data/uploadImages, data/uploadAudio, data/UploadTextFile, data/updateVideoDownloadCount | Register/login, multipart media upload, URL/QR media bookkeeping |
| Wide backup/restore | store_file, complete_store, restore_file, restore, storage/api/store, storage/api/restore | Obtain storage locations/tokens, dynamic blob transfer, finalize/restore |
| Mini Pinterest | oauth/token, pins | User OAuth code/refresh exchange and authenticated pin retrieval |

Common header names include Authorization, Log, Uid and CameraId. Wide multipart
uploads also carry fields such as Password, AppVer, FwVer,
RetentionPeriod, Checksum and CameraColor. Backup/restore has product/token/API-key
headers and dynamic blob PUTs with `x-ms-blob-type: Blockblob`. These are header
names, not usable credentials. Square uses dynamic URL GETs for media download
and a short-code resolver at `/long-url/sq/{shortCode}`. Some short-URL services
are separate from the applications' account/media APIs.

The shared Bluetooth library also contains 81xx URL-picture/audio commands,
plus live view and frame-upload commands for hybrid cameras. Finding them in the
standalone printer APKs does not mean that a mini/Square/Wide printer implements
camera capture or cloud storage. [`data/protocol/commands.json`](../../data/protocol/commands.json) marks those scopes
explicitly, and firmware/model gates must be followed.

What remains is app-side RE: tracing all callers, request construction, response/error
handling, upload checksums, retention/expiry behavior, notification lifecycle and
backend authorization in enough detail for an interoperable feature. The
catalogs alone do not show that these workflows work, that a service will
accept an independent client, or that the latest backend matches every bundled
interface. A unit-test request to the user cannot settle any of this.
