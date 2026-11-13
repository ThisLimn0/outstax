# Creative and image-composition features (static RE, 2026-11-12)

Scope: Mini Link 7.0.6, SQUARE Link 1.6.2 and Link WIDE 2.0.0, read from JADX output
only. No app was executed and no printer was used. Paths are relative to each app's
`decompiled/<app>/sources` (or `resources`). Obfuscated names (`E3/t`, `C2/j`) are
JADX names for 7.0.6/1.6.2 and change between builds. No vendor source, artwork,
template or font was copied here. Layout geometry is given as computed numbers.
Cloud flows are in [`cloud.md`](cloud.md).

## 1. The common print-image pipeline

Every creative mode ends in the same per-app builder. Creative features only change
what gets drawn in step 3.

| Step | Mini (600x800) | Square (800x800) | Wide (1260x840) |
|---|---|---|---|
| 1. Optional auto-enhance, per **source photo** | `X3/W` → `ImageEnhancement` MODE_NORMAL_SP2, long side capped at 800 | `U2/m.b` → `U2/x`, same engine and cap | same engine |
| 2. Crop/rotate, ColorMatrix filters | uCrop PNG, `X3/C1907t` | uCrop (max 1600²), view filters | uCrop (max 1260x840), `setAlphaWhite`, ColorMatrix |
| 3. **Composition** (text, stickers, frame, AiR drawing, QR) | `CustomImageView.t` draws overlay layers onto the crop, with a view capture as fallback | `y2/i.j()` view capture on white: content, text, FG icons, QR holder | `BaseEditFragment.processHighQualityPrintImage`, with a `getBitmapFromViews` view capture as fallback |
| 4. Resize + JPEG | `W1/AbstractC1886e`: cover-scale, q=100 stepping −5 until ≤ pref `compressionPreference` KiB (default 60) | `a.h(...,800,800)`: ≤105 KB (≤60 KB for collage) | `Compressor.compressImage(...,203 KB,1260,840)` |
| 5. 3D LUT ("instax-Rich") | pref `is3DLutOn`, `ui/i.n()`. Runs on the compressed JPEG, then re-compresses | `U2/E.b`. Needs printer capability bit 3 and the LUT licence to load | `Instax3DLut.processLut`, which also sets print option `PICINF_PICOP_3DLUT(8)` |
| Link+ extras | optional native USM (σ3/0.5/0.8) before step 4, native −0.5° rotate/crop, double density = 600x1600 @120 KB | none | none |

Consequences:
- Composition happens **after** enhancement and **before** the LUT. QR codes, text and
  frames therefore go through the LUT. If the LUT fails, the app shows a toast and
  prints without it.
- LUT exceptions: Mini collage (`E3/t`) never applies it. On Square, only `L2/g`
  (simple/AR/sketch) applies it, and collage `C2/j` and template `C2/p` ignore the flag.
  Wide passes the flag through for every mode, including collage, frame and Photo Mixer.
- Only the LUT uses a vendor asset (`20210701_3DLUT.bin`). Steps 1 and 3–4 are
  ordinary 2D work.

## 2. Collage layouts

Entry points:
- Mini: `ui/collageprint/LayoutSelectionActivity` → fragment `E3/t`. The list is
  `assets/collage_print/separate/frames.json` (`X3/C1903o.J`), shared by Link+
  (`linkplus/ui/collageprint`, fragment `c3/u`).
- Square: `ui/editableprint/EditablePrintCollageSelectActivity` and `E2/V0` → `C2/j`.
  The list is `assets/collageprint/frames.json`. Network print uses
  `assets/networkCollagePrint/frames.json`, which adds layout_00 (one cell).
- Wide: `ui/collageprint/CollageLayoutSelectActivity` → `CollagePrintFragment`. The
  list is `assets/collageprint/frames.json`.

The JSON entries name a thumbnail and a layout resource. The geometry itself is in
`res/layout/item_collage_layout_*.xml`: nested LinearLayouts with weighted
`CustomImageView` cells, `Space` gutters and a white background. The fragment
captures the on-screen view and resamples it to film size.

Gutter width:
- Mini and Square override every `Space` to `int(0.01509434 × container height)`
  (`E3/t.V0`, `C2/j` around line 693). That is **12 px** on both 800-px films, and it
  does not depend on the device.
- Wide keeps `@dimen/collage_divider` = 4 dp at screen resolution. In film pixels that
  is 4 × 1260 / (content width in dp): about **15 px** on a 360 dp phone, about 13 px
  on a 411 dp phone.

Collages have **no outer margin**. Cells run edge to edge, and the film mask hides
the edge (see §2.2).

All three apps have a frame-border toggle that hides every `Space` (gutter = 0)
(`toggleFrameBorder` in Wide, `Space.setVisibility` in Mini `E3/t` and Square `C2/j`).
Each cell holds its own photo with pan, zoom, rotate and filter. Text and stickers sit
on one layer over the whole card.

### 2.1 Cell rectangles (x,y,w,h in film px, gutter on)

The table was computed with a small LinearLayout weight solver over the layout XMLs.
The script `collage_geom.py` lives in the research scratch folder, not in this
project. Cells are listed in document order. For the borderless variant, set the
gutter to 0: weighted children then share the full length.

**Mini (600x800, gutter 12)**, `layout_01`…`layout_14`:

| Layout | Cells |
|---|---|
| 01 | 0,0,600,394 · 0,406,600,394 |
| 02 | 0,0,294,800 · 306,0,294,800 |
| 03 | three rows h=259 at y=0/271/541 |
| 04 | three columns w=192 at x=0/204/408 |
| 05 | 0,0,600,394 · 0,406,294,394 · 306,406,294,394 |
| 06 | 0,0,294,394 · 306,0,294,394 · 0,406,600,394 |
| 07 | four rows h=191 at y=0/203/406/609 |
| 08 | four columns w=141 at x=0/153/306/459 |
| 09 | 2x2: 294x394 at (0,0) (306,0) (0,406) (306,406) |
| 10 | 0,0,441,591 · 453,0,147,591 · 0,603,441,197 · 453,603,147,197 (weights 3:1) |
| 11 | left column 2x 294x394 · right column 3x 294x259 (y=0/271/541) |
| 12 | 0,0,294,194 · 306,0,294,194 · 0,206,600,388 · 0,606,294,194 · 306,606,294,194 |
| 13 | 2 columns x 3 rows, 294x259 |
| 14 | 2 columns x 4 rows, 294x191 |

Snap Collage (komadori) uses enum `o4/c` TWO=0, FOUR=8, SIX=12. These are layout
indices, so the 2-, 4- and 6-shot sequences map to layouts 01, 09 and 13.

**Square (800x800, gutter 12)**, `layout_01`…`layout_12` (network print adds
`00` = one cell, 0,0,800,800):

| Layout | Cells |
|---|---|
| 01 | 0,0,800,394 · 0,406,800,394 |
| 02 | 0,0,394,800 · 406,0,394,800 |
| 03 | 0,0,800,512 · 0,524,800,276 (weights .65/.35) |
| 04 | 0,0,512,800 · 524,0,276,800 |
| 05 | rows 263/250/263 at y=0/275/537 (weights .3388/.3225/.3388) |
| 06 | columns 263/250/263 at x=0/275/537 |
| 07 | 0,0,800,394 · 0,406,394,394 · 406,406,394,394 |
| 08 | 0,0,394,394 · 406,0,394,394 · 0,406,800,394 |
| 09 | 2x2 of 394x394 |
| 10 | 0,0,492,492 · 504,0,296,492 · 0,504,492,296 · 504,504,296,296 (.625/.375) |
| 11 | 0,0,296,296 · 308,0,492,296 · 0,308,296,492 · 308,308,492,492 |
| 12 | 3x3, outer rows/columns 263, middle 250 (x,y ∈ 0/275/537) |

**Wide (1260x840, gutter 15)**, `layout_01`…`layout_15`. The weights are written in
film pixels (420, 630, 424/412, 291/258, 322/308, 226/194):

| Layout | Cells |
|---|---|
| 01 | 0,0,1260,412 · 0,428,1260,412 |
| 02 | 0,0,622,840 · 638,0,622,840 |
| 03 | rows 281/249/281 at y=0/296/559 |
| 04 | columns 414/402/414 at x=0/429/846 |
| 05 | 0,0,1260,412 · 0,428,622,412 · 638,428,622,412 |
| 06 | 0,0,622,412 · 638,0,622,412 · 0,428,1260,412 |
| 07 | rows 214/184/184/214 at y=0/229/428/626 |
| 08 | columns 310/297/297/310 at x=0/326/638/950 |
| 09 | 2x2 of 622x412 |
| 10 | 0,0,826,539 · 841,0,419,539 · 0,554,826,286 · 841,554,419,286 |
| 11 | top: three columns 414/402/414 x 412 · bottom: 622x412 x2 |
| 12 | 0,0,622,281 · 638,0,622,281 · 0,296,1260,249 · 0,559,622,281 · 638,559,622,281 |
| 13 | 3x2, columns 414/402/414, rows 412 |
| 14 | 4x2, columns 310/297/297/310, rows 412 |
| 15 | 4x4, columns 310/297/297/310, rows 214/184/184/214 |

Square and Wide make the outer cells of 3- and 4-way splits larger than the inner ones
(for example 291/258/291 and 226/194). This offsets the edge area that the film mask
hides, so the visible cells look equal. Mini uses equal weights.

### 2.2 Film-mask ("vignetting") margins

These are the editor's hidden-edge guides, from the card-layout guidelines
(`instax_portrait_card_layout`, `instax_square_card_layout`,
`instax_wide_card_layout`). They are converted to film pixels relative to the
content holder. The apps use them for selection borders and for placing stickers and
text inside the visible area. They are not cropped from the file.

| Film | Left/right | Top/bottom |
|---|---|---|
| Mini 600x800 | ≈14 px | ≈20 px |
| Square 800x800 | ≈25–26 px | ≈25 px |
| Wide 1260x840 | ≈38 px | ≈47 px |

These are UI estimates, not printer-qualified bleed values.

### 2.3 Mini "combination" (split) prints

These spread one photo over several films. Entry is the combination tab, fragment
`E3/G`. The list is `assets/collage_print/combination/frames.json`, with three
entries: split_01 is 2 columns x 1 row, split_02 and split_03 are 1 column x 2 rows.
The cropped source is cut into equal row×column tiles (`E3/G.V0`, files `comb<i>.png`).
Each tile becomes its own 600x800 card with its own text and filters, and the cards
print in sequence. The preview layouts (`item_combination_layout_{1,2,3}`, card ratio
180:137 or 137:180) only draw the physical film borders between tiles. The physical
gap is **not** subtracted from the image. This is Mini only.

## 3. Frames and templates

- **Mini server templates** (`serverBasedTemplates/*`, `ui/templateprint/TemplateSelectionActivity`,
  fragment `V3/g`):
  - Data model: `TemplateConfig{appVersion, platform, metaData[{type, lastModifiedDate, data[{url}]}]}`,
    then `TemplateDetails{areaSettings, templates[]}`, then
    `TemplateModel{name, frame_edit, frame_print, thumbnail, is_new, lastModifiedDate}`.
  - The art is downloaded from the vendor resource host and cached in `filesDir/Templates/`.
  - The bundled fallback JSONs the code points to are missing from the acquired assets.
  - The frame PNG is drawn full-bleed over the photo, then text goes on top.
  - **Needs the cloud.**
- **Square frames:**
  - 30 entries in the `SharedAssets.apk` asset split (`assets/templates/frames.json`).
    The split is not in the decompiled base `resources/assets`.
  - Each is a 960x960 RGBA overlay drawn full-bleed and scaled to 800
    (`C2/p.m` → `EditablePrintFrameSelectActivity`).
  - They are decorations, not window masks: 61–98 % transparent, and 26 of 30 are open
    to all four edges.
  - Bundled, no cloud.
- **Wide frames:**
  - `assets/frameprint/frame_resource.json` holds 20 `{frame, id}` pairs. The images
    are 5040x3360 webp (4x film, #12 is 3x) in the asset split.
  - `BaseEditFragment.setFrame` adds a FIT_XY ImageView over the card.
  - Openings are irregular. Examples in film px: #09 (116,116)–(1086,706),
    #18 (100,98)–(1112,742).
  - Bundled, no cloud.
- **Wide Photo Mixer** (`instaxLinkWide/photoMixerLib/step_generation`,
  `photomixer/ui/template_editor/TemplateEditorActivity`, data in base `assets/templates/`):
  - Templates:
    - 425 image templates `I_H_<A|B>_<variant>_<n>`, with 1–6 photos each.
    - 121 text templates `T_H_*`.
  - Each placement has center_x/y, width/height in a 1260x840 reference space (it can
    run off the edges) and a crop_mode: `cutout`, a subject segmentation, or
    `shape_trimming`, with a mask from `assets/maskImage`.
  - Placements also carry a random rotation_range and scale_range and a main/sub role.
  - Segmentation runs on the device: ONNX u2netp, MobileSAM and ML Kit.
  - No Retrofit route.
- **Wide ratio layouts** (`ImageRatioTemplates/layoutConfigurations.json`): 1–3 photos at
  1:1, 4:3 or 16:9 as percentage rectangles. They are used only by save-to-gallery
  (`backgroundeditor/ui/PreviewSaveToGalleryActivity`), not by printing.

## 4. Text, stickers and drawing

- **Text:**
  - Mini uses `texteditor/components/AutoResizeTextView`.
  - Square uses the shared `com.trenser.text_editor`.
  - Wide uses `instaxwide/texteditor` (outline and shadow).
  - Text is drawn on one layer over the card, so it crosses collage cells and is
    composited before the LUT.
  - Fonts: Mini downloads about 10 Google Fonts through Play Services (`X3/C1899k`) and
    bundles only Roboto and Noto Sans JP. Square and Wide read font and colour lists
    from the asset split.
- **Stickers:**
  - The list is `StickerIcons/stickerList.json`, in the asset split. It is absent from
    the acquired Mini assets.
  - Each sticker has separate low-res edit art and print-res art, plus Unicode emoji
    rendered through EmojiCompat to PNG.
  - User-made stickers: Square "sketch"/Fontgenic (`ui/sketch_edit_print`) captures a
    drawing with the camera and crops/recolours it with OpenCV into `.FGIcons`. Wide
    keeps them in the `FGIconEntity` table.
- **INSTAX AiR drawing (Mini, Link 2):**
  - `instaxair/ui/MainActivity`.
  - While the printer's AR switch is pressed (`3000` status bits 1..2), each camera frame
    goes to the native `libPictionaryWrapper.detectPoint`, which tracks the printer LED.
  - The trail is drawn with brushes from `assets/menu/menu.json`.
  - The result is recorded to mp4 and printed through the video-QR path.

## 5. QR printing

| | Mini | Square | Wide |
|---|---|---|---|
| Types | INSTAX AiR video, AiR Studio (photobooth) video | AR print only | URL 0, SOUND_CHEKI 1, MAP 2, TEXT 3 (`QRPrintType`) |
| Encoder | ZXing v5, ECC M, margin 4, 180x180 (`O3/a`) | v5, M, margin 4, 180x180, user colour on white (`E2/B`) | v5, M, margin 0, 180x180, user colour (`QRCodeGenerator`) |
| Size/position on film | position stored as film coordinates (x/w·600, y/h·800) | side 0.225 x card width (≈180 px), 5 positions, default bottom-right | 15.6 % of card width (≈197 px), free drag, stored relative |
| Content | `https://minilink.instax.{com,cn}/<uid>` | short link → `deep-link.instax.com/sq?uid=…&deepLinkType=1…` | URL/map: the user's string (shortened only to fit v5). Sound/text: `https://linkwide.instax.com/<uid>` |
| Cloud | always (upload of media under uid) | always | sound/text only |

The QR is drawn in composition step 3, so it passes through the LUT. History keeps a
copy of the image without the QR, so the QR can be regenerated.

Mini's "movie print" (`ui/movieprint/MoviePreviewActivity`) without AiR is plain frame
extraction from a video, with no QR and no cloud.

## 6. Camera, gesture and AR modes

- **Gesture actions (Mini `X3/Q`, config `assets/config/Config.json` AxisControl, pref
  `accelerationLinkage`, default on):**
  - Inputs are accelerometer axes from `3000`.
  - Orientation: x < −887 (upright) switches to fun mode, and supine2 switches to print mode.
    This is Link 2 only. The Link 3 top screen ignores it.
  - Tilt angle drives camera zoom in four bands: in 20–60°, in 60–135°, out −20 to −60°,
    out −60 to −135°. Readings with |y| ≤ 32 are a dead band.
  - Bit 0 is the remote shutter. Link 3 adds a function button (bit 3).
- **Snap Collage / komadori (Mini, Link 3 and Link+, `komadori/view/KomadoriActivity`):**
  - The printer button triggers 2, 4 or 6 phone-camera shots.
  - Shots are stored in Room `komadori(image_name, index, orientation, selected_collage_type,
    zoom_value, rotation_value, image_matrix, …)`, then passed to the collage editor
    `E3/t` (layouts 01/09/13).
  - History keeps the comma-separated source frames (`komadori_image_path`) so
    `link3/ui/CollageAnimationActivity` can replay them as an animation.
- **Photobooth / "AiR Studio" (Mini, `photobooth/unity/PhotoBoothActivity`):**
  - A Unity player with only bundled Addressables (no remote catalog URLs).
  - History table `photobooth_history_table` (PHOTO/VIDEO/LIVE) holds at most 20
    entries. The oldest unlocked one is evicted.
  - Remote live view is phone-to-phone over BLE GATT, joined by a deep link.
  - Stills go to the normal editor. Videos go through the cloud video-QR path.
- **Match test / compatibility (Mini, `ui/compatibility/*`):**
  - A questionnaire (`assets/compatibility/questions.json`) plus colour gradients
    (`assets/colors/gradientColors.json`) produce a result card.
  - The card is saved as history type COMPATIBILITY (1), which the history list hides.
  - Fully offline.
- **Square AR print (`ui/arprint/*`):**
  - AR types: 3D model, speech bubble, background filter, doodle, text, sketch.
  - The print is the photo plus a QR.
  - The viewer side (`arscan/ArDataDownloaderActivity` → `ArScannerActivity`)
    downloads the reference image and overlay data, then uses ARCore AugmentedImage on
    the physical print. Rendering uses Sceneform/Filament with bundled glTF, ktx and
    glsl assets.
  - Needs the cloud for sharing and playback ([`cloud.md`](cloud.md)).
- **Camera:** Square `ui/instaxCamera/CameraActivity` and Mini `ui/instaxcamera` are
  CameraX capture into simple print.

## 7. Print history and reprint

| | Mini | Square | Wide |
|---|---|---|---|
| Store | Room `PrintHistory(image_path, printed_at, image_type, app_region, komadori_image_path, is_double_density)` + `InstaxAirVideoEntity` | Room `ImageHistory(image_path, image_without_qr_path, printed_date, app_region, image_type NONE/AR_PRINT/DOWNLOADED)` + `ArPrint` tables | Room `ImageHistoryEntity(image_path, image_path_without_qr, qr_code_content, qr_code_position_x/y, qr_code_color, print_id, …)` + SoundCheki/CustomText/CustomUrl/MapPrint entities |
| Files | `getExternalFilesDir(".Images")/<uuid>.jpg` | `PrintedImages/<uuid>` | app storage |
| What is stored | **pre-LUT** composite (with QR) at film size, plus a no-QR copy for QR prints | pre-LUT 800x800 composite + no-QR copy | composite + no-QR copy + QR metadata |
| Reprint | stored JPEG opened as a **new source** in `EditScreenActivity`: crop, compress and LUT run again (`PrintHistoryPreview.A2`) | same, re-processed. An AR reprint starts from the no-QR copy and adds the QR back | unchanged item without QR: stored file **resent** (LUT re-applied if Rich mode is on). Anything else is re-processed with the QR regenerated from stored content |

No app replays the original BLE payload. A reprint runs the print task again.
Downloaded or AR-scan entries are count-limited, and the oldest are deleted.
Square anniversary reminders are local only (Room + AlarmManager). Wide's history
backup/restore is a cloud migration flow ([`cloud.md`](cloud.md)).

## 8. Cloud and asset dependence per creative feature

| Feature | Vendor server | Vendor asset needed | Desktop reimplementation |
|---|---|---|---|
| Collage layouts (all apps) | no | no (thumbnails only) | **Yes.** Geometry as numbers in §2 |
| Mini split/combination prints | no | no | **Yes** |
| Text layer | no (Mini fonts via Google Fonts) | fonts | **Yes**, with OFL fonts |
| Stickers / emoji | no | sticker art | **Yes**, with our own art or an open emoji set |
| Square/Wide bundled frames | no | frame art | **Yes**, with our own RGBA overlays at film size |
| Mini server templates | **yes** (catalog + art) | yes | Only as our own frames. The catalog is not reproducible |
| Wide Photo Mixer | no | templates, masks, ONNX models | **Possible** with our own placement JSON, masks and open segmentation models, at large effort |
| QR with the user's own URL/text/map | no | no | **Yes**: local string → QR |
| Mini video QR, Wide sound/text QR | **yes** | n/a | Only by pointing the QR at the user's own hosting. A Fujifilm-compatible version is impossible (vendor-minted UIDs, viewer on vendor host) |
| Square AR print | **yes** | 3D models | No. Needs the vendor backend, mobile ARCore and a viewer app |
| Mini AiR drawing | no (except video QR) | closed native LED detector | Not on desktop, because it needs a live printer-facing camera |
| Gesture zoom/shutter | no | no | Shutter only. There is no phone camera to zoom on desktop |
| Snap Collage | no | no | Possible with a webcam plus the printer button, but of low value |
| Photobooth (Unity) | only video QR | Unity bundles | No |
| Match test | no | question/colour data | Trivial, but not worth building |
| History + reprint | no | no | **Yes**: store the pre-LUT composite and re-run the pipeline |
| 3D LUT step | no | vendor LUT (licensed) | Only with our own LUT. The hook is the last step |

## 9. Recommended desktop features

1. **Collage with layout geometry.** Port the three per-film tables in §2.1 as data:
   Mini and Square use a fixed 12 px gutter. For Wide, use a fixed number, ≈15 px. Add a
   border toggle and per-cell pan, zoom and rotate. No vendor assets are involved and the
   value is high. It is a pure pre-processing step before the existing
   resize/JPEG/print path.
2. **Overlay layer: text, our own frames and stickers, and a local QR.**
   - One RGBA layer composited before resize and LUT, drawn at film resolution.
   - Use the §2.2 mask margins as a "safe area" guide.
   - The QR encodes the user's URL, text or map link directly (ECC M, auto version
     instead of fixed v5). That covers Wide URL/map prints, and Mini/Wide media prints
     when the user hosts the media themselves.
3. **Local history with reprint.** Store the pre-LUT composite plus the no-QR copy and
   the QR metadata. On reprint, re-run steps 4–5 rather than replaying bytes. This
   matches all three apps and keeps reprints correct when the printer or LUT setting
   changes.

Runner-up: Mini-style split prints, one photo spanning 2 films, which is cheap once
collage exists. Not worth building: AR, photobooth, AiR, Photo Mixer auto-layout and
the vendor template catalog.

## Open items

- Wide gutter in film pixels depends on the device. A desktop port should pick a
  constant (≈15 px).
- The per-template geometry of Mini server templates was not checked, because the
  downloaded `template.json` is not in the acquired files.
- The `PrintHistory` size cap in Mini was not checked.
- Pixel-level confirmation against official output requires hardware or app runs, and
  is part of the qualification phase.
