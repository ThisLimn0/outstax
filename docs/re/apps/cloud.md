# Cloud flows: authentication, upload/download and response handling (static RE, 2026-11-12)

Scope: Mini Link 7.0.6, SQUARE Link 1.6.2 and Link WIDE 2.0.0 (JADX). The route map is
[`data/http/http-endpoints.json`](../../../data/http/http-endpoints.json) and the models are in [`data/http/http-models.json`](../../../data/http/http-models.json). **No vendor
request was sent.** Only host names are recorded here. Static API keys (`x-api-key`,
shortener keys, Pinterest client credentials), the embedded UID cipher key/IV and the
fixed Wide device password are in the binaries and are deliberately left out. Paths
are relative to each app's `sources`, and obfuscated names are build-specific.

Printing never depends on any of this: image frames go to the printer over local
Bluetooth ([`HTTP.md`](../../protocol/HTTP.md)).

## 1. Hosts (production, with `-stg` for staging and `.instax.cn` for CN)

| App | API | Media / viewer | Links |
|---|---|---|---|
| Mini | `api-minilink.instax.com/api/` (`u2/f.z`) | storage `resource.instax.com/minilink…/` (`x2/i`), viewer `minilink.instax.com/<uid>` (`M3/a.d`) | `deep-link.instax.com/minilink` |
| Square | `api-sqlink.instax.com/api/` (`g2/v`) | Azure SAS URLs from responses, web `sqlink.instax.com/main/` | `short-link.instax.com` (POST `/`, GET `/long-url/sq/{shortCode}`), `deep-link.instax.com/sq` |
| Wide | `api.linkwide.instax.com/api/` (staging `api-linkwide-stg`) | viewer `linkwide.instax.com/<uid>`, backup via `/storage/api/*` + blob SAS | `deep-link.instax.com/linkwide`, `short-link.instax.com`, `migration.instax.com` |

Third parties: Pinterest `api.pinterest.com/v5/` (Mini), Firebase/FCM and Baidu push
(Square), PostHog EU and Crashlytics (analytics).

## 2. Identity, tokens and headers

- **Device id:** `smartphoneId = "<prefix>_" + ANDROID_ID + UUID + UUID`, truncated to
  254 characters and generated once. Prefixes: Mini `bo23_` (`x2/k.j`), Square `bo17_`
  (`z/AbstractC0608c.m`), Wide `bo22_` (`UserAuthenticationHelper.getRandomUUID`).
- **Headers:**
  - `Authorization: Bearer <token>`.
  - `Log: {"logInfo":{applicationType, applicationVersion, os, osVersion, deviceName}}`.
  - `Uid` (media or record id) and `CameraId` (printer id).
  - A static per-region `x-api-key`, added by an OkHttp interceptor (Mini `u2/f.A`,
    Square `c2/a`, Wide `InstaxRestService.initWithBaseUrl`).
  - An app User-Agent (Square `c2/z`, Wide `UserAgentHelper`).
- **Mini token:**
  - Obtained with `POST user/authorizeSmartphone {smartphoneId, country, productId:"bo-23"}`,
    sent without Authorization. Response `data` is the bearer.
  - Stored in prefs `<production_|staging_><region>serverToken`.
  - The client tracks **no lifetime**. The server signals expiry with body `status 401`
    (`b4/a.TOKEN_EXPIRED`). The client then clears the token (`u2/f.B`), re-authorizes and
    retries the call once.
- **Square token:**
  - Obtained with `POST users` and stored in prefs `<dataserver>_<region>_serverToken`.
  - A 401 (body or HTTP) clears it and the coroutine calls itself again after
    re-authorizing. There is no explicit cap.
- **Wide token:**
  - First run: `POST user/registerDevice {deviceId, hashedPassword, productId, country}`.
    Later runs: `POST user/login {deviceId, hashedPassword}`.
  - `hashedPassword` is the SHA-256 hex of an app constant, not a user secret.
  - The bearer lives only in a static field for the process lifetime.
  - 6003 (user does not exist) re-registers the same id. Invalid credentials register a
    fresh id. No 401 handling was found.
- **User PIN ("password"):** sent as SHA-256 hex. The PIN defaults to "0000"
  (`X3/C1903o.y`). China builds use a separate pref.
- **Registration and analytics:**
  - `user/registerCamera` / `users/cameras` sends `{cameraId, firmwareVersion, cameraColor,
    country, productId}`. It is date-gated by pref `<env><region>_register_<printerId>`.
  - `user/addAnalyticsStatus` / `users/analytics` syncs the consent toggle.
  - Neither has a user-facing function.

## 3. Transport behaviour and errors

- **Timeouts:**
  - Mini API: 90 s read/connect. Mini template client: 30 s.
  - Square: 90 s.
  - Wide API: 90 s read / 20 s connect. Wide backup: 180 s / 180 s.
- **Envelope:** `{status|statusCode, message, data}`, where `status 0` means OK.
- **Throwable mapping** (all apps): `HttpException` maps to the HTTP code,
  `SocketTimeoutException` to 1030, and anything else to 1012 (Wide backup uses 1013).
- **Retries:**
  - Each upload, auth or bookkeeping step retries **up to 2 times, only on 1012**, with
    no backoff.
  - 401 triggers one re-auth (Mini) or a recursive re-auth (Square).
  - Template downloads get 2 attempts per file.
- **App codes** (Mini `b4/b`, Wide `util/ApiErrorCodes`):

  | Code | Meaning |
  |---|---|
  | 1012 | server communication error |
  | 1013 | no internet |
  | 1017 | not authorised (status 0 with no data) |
  | 1030 | timeout |
  | 1060 | QR content too long |
  | 1061 | checksum creation failed |
  | 1070/1071 | data-region mismatch |
  | 1091/1092 | upload URL or info null |
  | 1101–1104 | QR uid/bitmap errors |
  | 2106 | short-link failure (Square) |

- **Server codes:** 5001–5005 registration, 6001–6003 login, 8008 no data,
  8009 duplicate, 4 checksum error (Wide), 13 partially verified.

## 4. Flows by feature

### Mini: INSTAX AiR / AiR Studio video QR (`c4/b`, `ui/movieprint`, `EditScreenActivity`)
1. **UID** (`M3/c.b`):
   - Built from the last 8 digits of the printer deviceId, the last 10 digits of the
     per-app counter `totalQRPrintCount` and UTC `yyyyMMddHHmmss`. Non-digits become 0.
   - The digits are packed in pairs into 16 bytes, reversed, then encrypted with
     AES-128-CBC using an embedded key/IV (`a4/a.d`). The result is hex-encoded.
   - The QR encodes `https://minilink.instax.com/<uid>` and is composited before the LUT
     (see [`creative.md`](creative.md)).
2. **Checksums:** MD5 of the image file bytes and of the video file bytes, each Base64
   with no wrapping (`X3/Z.a`).
3. `POST data/getUploadUrls` (headers `Uid`, `CameraId`) returns `{imageUrl, videoUrl,
   videoType}`. The URLs are Azure SAS.
4. `PUT <videoUrl>` with `application/octet-stream` and `x-ms-blob-type: BlockBlob`
   (`v2/d`).
5. `POST data/updateVideoUpload` sends:
   - `applicationVersion "Android_7.0.6"`, `cameraColor`, `cameraId`, `checksum`,
     `firmwareVersion`
   - `password` (SHA-256 of the PIN)
   - **`retentionPeriod 2`**
   - `uid`, `videoDownloadStatus`, `videoType` (0 = AiR, 1 = AiR Studio).
6. `PUT <imageUrl>`, then `POST data/updateImageUpload {checksum, uid}`.
7. **Resume and expiry:**
   - Per-leg uploaded flags in `InstaxAirVideoEntity` skip legs that already finished.
   - The UI shows expiry as upload date + 2 years (`VideoAirPreviewActivity`), which
     matches the retention value in years.
8. **Delete:** `POST data/deleteUserData` (`Uid`), only when the record's region matches
   the app region.
9. **Receiving side** (`c4/f`):
   - The deep link carries `uid`, `videoUrl`, `imageUrl` (Base64 paths), `isChina` and
     `videoType`. The region must match.
   - The app downloads the blobs and stores a DOWNLOADED history row.
   - It then sends `POST data/updateVideoDownloadCount {uid}`.

### Mini: server templates (`x3/a`, `serverBasedTemplates/model/*`)
- GET `resource…/Config/Android/V7.0.6/config.json`, then each `Resources/Android/V7.0.6/<path>`.
- Re-downloads when `lastModifiedDate` is newer. Cached in `filesDir/Templates`.
- `areaSettings` gates templates by region.
- Parts of the downloader are "Method not decompiled" in JADX.
- Serves template/frame printing. No auth header beyond the API key and User-Agent.

### Mini: Pinterest (`R2/h`, `S2/a..c`)
- Browser OAuth with redirect `linkplus://oauth/`.
- `POST oauth/token` uses Basic client credentials with `authorization_code` and
  `continuous_refresh=true`.
- Tokens are stored in prefs `pinterestAccessToken` and `pinterestRefreshToken`.
  `expires_in` is ignored.
- On LP_TOKEN_EXPIRED (401) the app tries one refresh. If that fails, it clears both
  tokens and forces a new login.
- `GET pins?page_size=200` is an image picker for printing.

### Square: AR print (`c2/j`, `ui/arprint/*`, `arscan/*`)
1. **Create:** `POST arprints {cameraId, arType}` (0 3D model, 1 bubble, 2 sketch,
   3 doodle, plus bgFilter/text details) returns `{refImageUploadUrl, arTypeImageUploadUrl}`.
2. **Upload:** blob PUTs (`d2/c`) of the reference image (the print without the QR) and
   the overlay.
3. **Finalize:** `PUT arprints` with MD5-Base64 checksums, `retentionPeriod 2` and
   `arDetails`.
4. **Printed QR:**
   - It encodes a short link to `deep-link…/sq?uid&appRegion&deepLinkType=1&dataserver=N`
     (`E2/B`, ZXing v5, ECC M).
   - If short-link creation fails (2106), there is no AR print.
5. **Viewer:**
   - Scanning the QR opens the deep link. The app sends `GET arprints/download` (`Uid`)
     and receives `{refImageUrl, arType, details, expirationDate}`.
   - The data is saved to `.ArPrint/<uid>`, then ARCore AugmentedImage tracks the print.
   - A missing record shows "expired or deleted".
6. **Delete:** `DELETE arprints`.

### Square: network print / inbox (`c2/k`, `networkprint/ui/inbox`, `DynamicLinkDownloaderActivity`)
1. **Sender, create:** `POST networkprints {smartphoneId, numberOfImages, cameraId}` returns
   `{uid, imageUploadUrlAndUid[]}`. Blob PUTs follow.
2. **Sender, finalize:** `PUT networkprints` sends:
   - `applicationVersion`
   - `imageDetailArray[imageUid, imageChecksum, positionInTemplate, imageUploadedBy]`
   - `reactionDetailArray`
   - `templateName "T<n>"` (collage layout) and `isBorderVisible`
   - `password` (SHA-256 of the PIN)
   - `notificationToken` and `osPlatform android|baidu`.
3. **Sharing:** the link is shared as a `deepLinkType=2` deep/short link.
4. **Recipient:**
   - The inbox holds fewer than 100 items.
   - `GET networkprints/download` (`Uid`) returns `{collageImageUrl, per-image URLs,
     reactions, uploadDate}`.
   - The recipient prints locally, then sends `PUT networkprints/download` with header
     `isPrintCount`.
5. **Expiry:** shown as uploadDate + **30 days**.
6. **Push:**
   - Registration uses `networkprints/registerNotification` and
     `PUT|DELETE networkprints/pushRequiredUser` (`q2/c,e,g,i`).
   - FCM data keys are `title, desc, type, redirect_url, event_type, uid`
     (`push/firebase/InstaxMessagingService`).
- **Anniversary reminders** are local only (Room + AlarmManager), with no route.

### Wide: QR prints (`ui/qrcode/*`, `SoundChekiUploadHandler`)
- **SOUND_CHEKI:**
  - `POST data/uploadImages`, multipart `file`, with headers `Uid`, `Password` (SHA-256 of
    the PIN), `CameraId`, `AppVer`, `FwVer`, `RetentionPeriod "2"`, `Checksum` (MD5
    Base64) and `CameraColor`.
  - Then `POST data/uploadAudio` (`Uid`, `DownloadStatus`, `Checksum`). Each leg retries.
  - The UID comes from `UIDCreationHelper` with `AESEncryption`. The QR encodes
    `https://linkwide.instax.com/<uid>`.
  - Download, count and delete use a dynamic GET, `data/updateVideoDownloadCount` and
    `data/deleteUserData`. Expiry is stored locally from the response.
- **TEXT_PRINT:** the text is saved to a file and uploaded with `data/UploadTextFile`. The
  QR points to the viewer URL for that uid.
- **URL_PRINT / MAP_PRINT:**
  - The content is the user's URL or a Google Maps search URL.
  - It is sent to the short-link service (`DeepLinkApiInterface POST /`) only to fit
    QR v5. Otherwise there is no upload.
  - `BaiduService short-urls` has no caller in 2.0.0.

### Wide: phone-to-phone backup/restore (`ui/settings/DataBackUpAndRestore`, `api/BackupRestService`)
- **Current store flow:**
  1. `GET /storage/api/store` (x-api-key, Log, productId) returns `{directoryUrl, sasToken,
     storeToken}`.
  2. Files are PUT to `directoryUrl/<name>?<sas>` (`x-ms-blob-type: Blockblob`): history
     images, no-QR copies, audio, text and a key-list JSON.
  3. `PUT storage/api/store {storeToken, productID, pageIndex, totalCount,
     fileDetails[fileName, fileChecksum]}` validates the upload page by page and returns
     `{restoreToken, validationCompleted/PendingTotalFileCount}`.
- **Current restore flow:**
  1. The old phone shows the restoreToken as a QR.
  2. The new phone sends `POST storage/api/restore` (header `restoreToken`) and gets
     `{directoryUrl, metaDataFileUrl, sasToken}`.
  3. It downloads everything, then calls `saveRestoredDataToDB`.
- **Legacy email variant:**
  - `store_file`: multipart with an MD5-hex checkSum, key, storeToken and formFile.
  - `complete_store {mailAddress, storeToken}` returns `{restorePassword, expiredDateTime}`.
  - `restore {mailAddress, restorePassword}` returns a restoreToken, and the files then come from
    `restore_file`.
- **Unused constant:** `api-kazoku.fujifilm.com/storage/api/` appears in the code but is not
  used.

## 5. Which features need Fujifilm's servers

**Impossible or pointless to reimplement as a compatible offline client:**

| Feature | Why |
|---|---|
| Mini AiR/AiR Studio video QR, Wide sound/text QR | The QR resolves to a vendor viewer. UIDs are minted with an app-embedded cipher and accepted only by the vendor backend. Recipients use the official viewer or app. |
| Square AR print (sharing + playback) | The record, the reference image and the viewer all live in the vendor backend, plus the mobile ARCore app. |
| Square network print / inbox / reactions / push | Vendor relay between official app installs. The FCM/Baidu sender identity is Fujifilm's. |
| Wide backup/restore | Only meaningful between two official app installs. |
| Mini server template catalog | Vendor-hosted, licensed artwork. |
| Registration/analytics routes | No user-facing value. Do not implement. |
| Pinterest import | Needs a separately registered Pinterest developer app. Fujifilm's client credentials must not be reused. |

Using these endpoints from an independent client would also need the vendor API keys.
That is outside this project's scope, and the latest backend may not accept it.

**Replaceable without any vendor server:**
- "Media behind a QR": encode a URL to the user's own hosting (or a local file share) in
  the printed QR. Any encoder works (ECC M, auto version).
- URL, map and text QR prints: encode the string directly.
- Square anniversary reminders, print history and reprint: local only.
- Frames, stickers and text with our own art: see [`creative.md`](creative.md).

## Open items

- Server-side token lifetime is unknown: the client only reacts to a 401.
- Square's recursive re-auth has no visible cap, so a broken server could loop it.
- Square network print expiry (30 days) and AR/media retention (`2`, shown as years in
  Mini) are client-side display assumptions. Server enforcement is unverified.
- Response schemas are only as complete as [`data/http/http-models.json`](../../../data/http/http-models.json). They have not been
  confirmed against live traffic, and no live traffic is planned.
