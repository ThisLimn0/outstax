# Auto-correction engine and optional unsharp: native RE notes (2026-11-12)

Static analysis only. Nothing was executed, and no vendor code, tables or assets
are copied here. Addresses are ARM64 virtual addresses in the Mini 7.0.6 split.
Every FDIA library is byte-identical across Mini, SQUARE and WIDE: same SHA-256
for all 13 FDIA/AIPII `.so` files. `libsharpness.so` exists only in Mini.
Helper scripts are outside the project, in the research workspace (`research/work/enhancement`: `dis2.py`
disassembler with PLT/string/constant annotation, `cg.py` call graph from
`.eh_frame`, `ffpf.py` profile directory parser, `wstr.py` UCS-4 string
extractor).

Each finding has a depth label:

- **R**: recovered. The behavior or numbers were read from instructions or data.
- **S**: stage identified. Its existence and role are known, but not its internals.
- **U**: unresolved.

## 1. Call chain (R)

**Java.** All three apps copy `assets/data.zip` into `getDir("temp")/data`.
Mini does this in `X3/W.k`, called from SplashActivity. Each app calls
`com.fujifilm.instaxshare.i2fdia.ImageEnhancement` with
`EnhanceMode.MODE_NORMAL_SP2` (code 1):

| App | Call site | Mode evidence |
|---|---|---|
| Mini | `X3/W$a.run` | mode passed by name |
| SQUARE | `U2/x.run` | `AbstractC0562t.k(2)`, which returns `2-1` |
| WIDE | `instaxwide/util/j` and `IntelligentFilterFileUtils$applyI2Filter` | mode passed by name |

In Mini, the "AUTO" filter type (`C1907t.b.AUTO`, for example in `d3/g.t`) is what triggers this call.

Before the native call, Java does the following:

- Decodes the photo.
- If the displayed height is over 800, scales the image so the displayed height
  is 800 (`createScaledBitmap`, filtered).
- Applies the EXIF orientation with a Matrix.
- Passes ARGB_8888 bytes, width, height and an "exif" byte array.
- After enhancement, scales the result back up. Mini restores the original size.
  SQUARE and WIDE restore the original width and keep the aspect ratio.
- Saves the result as `autoFilter_img.jpg`.

So enhancement always runs on an image of at most 800 rows (displayed
orientation), and is then upscaled. **That limit is part of the observable
result.**

The exif array is non-null only when the EXIF Orientation equals 0. Orientation
defaults to 1, so in practice this almost never happens. Even then, the array
must be an APP2 (0xFFE2) segment starting with `Exif` (`W.h`, `eb.b.t`), which
real JPEGs do not have. **In practice the engine always receives NULL EXIF.**

**JNI.** `Java_..._enhanceImageImpl` is at 0xc10 in `libjnii2fdiainstaxwrapper.so`:

- Converts RGBA to RGB (`I2FDIA_INSTAX_ConvertRGBA2RGB`, w*h*3 buffer).
- Builds a licence credential: GUID `E87ACC91-…;` + package name + `;` + signing
  certificate `toCharsString()`, converted UTF-8 → UCS-4 with iconv (0x104c–0x1190).
- Calls `I2FDIA_INSTAX_EnhanceImage`, then converts the result back to RGBA.
- Error codes: 0xea61..0xea65.

**Instax wrapper.** `I2FDIA_INSTAX_EnhanceImage` is at 0x473c in
`libi2fdiainstaxwrapper.so`. It validates its arguments (mode ≤9, error
50002/50003) and then calls the core at 0x4dc8:

1. `setenv("FFAIPII12_CONFIG_PATH", dataDir)`.
2. On the first call only, `AIPII_Initialize(credential)`. A static flag is kept at 0x15550.
3. Loads the YAML for the mode (0x4f28). The mode table at 0x15460 is built at
   0x4b98. Its order is: SP1, SP2, softfocus, MODE_4 … MODE_10. Only SP1, SP2 and
   softfocus ship.
4. Parses the YAML with its own parser (0x52e0) into preference/setup/structure
   structs. It then repacks them into an AIPII "extend" parameter block at
   `this+0xc0` (0x50b8 onward).
5. `AIPII_AnalyzeImage(&{type=1, rgb, w, h}, flag, &analysis, NULL, &{exif, exifLen}, NULL)` (0x4eb0).
6. `AIPII_EnhanceImageExtend(&in, &out, &analysis, &extendParam, NULL)` (0x4eec).

**AIPII** (`libffaipii.so`, about 8 MB: CIP image framework, libjpeg/tiff/png,
boost, licence code, bzip2). The exports are thunks onto a singleton at 0x7b0828:

| Export | Implementation |
|---|---|
| `AIPII_AnalyzeImage` | 0x2cff94 |
| `AIPII_EnhanceImageExtend` | 0x2d4924 |
| `AIPII_Initialize` | 0x2cb060 |

`ImageEnhancer` methods were identified through their direct imports and log strings:

- **Analyze, 0x2c80bc:**
  - 0x2ba9ac: `I2FDIA_H_TAG_ImportData` and `I2FDIA_F_CheckSupport` (EXIF tag data).
  - 0x2bb8ac: `I2FDIA_H_SUP_SetProfile`, then `I2FDIA_H_SUP_AutoSetup`, or
    `…AutoSetupWithFaceInfo` when a face list is supplied. The branch is at
    0x2bc224 (`ldr x8,[x21,#0x160]; cbz`). At most 20 faces, 32-byte records.
  - The analysis is exported with `I2FDIA_H_SUP_ExportLightData`.
- **Enhance, 0x2bfd1c:**
  - 0x2bedf4: `I2FDIA_H_PRF_ImportData`, `I2FDIA_H_SUP_ImportLightData`,
    `I2FDIA_H_SUP_RestoreParameter`, then `I2FDIA_H_SUP_ManualSetup` with the
    YAML user parameters.
  - 0x2c031c: `I2FDIA_R_RenderingEx` into `I2FDIA_H_IMG_ExportDataEx`.
- **Defaults, 0x2b838c:** `I2FDIA_ST_DefaultParameter`. It is called from the
  Simple and Extend parameter classes (0x2d78c0 and 0x2dc9cc).

The log strings are UCS-4 and explain the flow. Examples:

- "Dump render setting before auto setup"
- "_afterAutoSetup"
- "_beforeManualSetup"
- "Start ImageEnhancer::RenderImage"
- "Profiles/Output/sRGB10_8.prf"
- "Mid output type: %1%"
- "AutoSetupWithFaceInfo. faceCount %1%"
- "Input profile file is not specified.(use default)"

**Faces.** The wrapper passes NULL for the face list. `data.zip` ships none of the
face-detector files named in `Param/ChangeLog.txt` (`Setup/iFace/*`). The
no-face `AutoSetup` path is therefore the one taken. This is consistent with the
evidence, but the AIPII argument→`+0x160` mapping was not traced field by field.

**Licensing.** `AipLicense.lic`/`.slic` are per-app. Apart from `ffaipii.ini`, they are the only `data.zip`
entries that differ between apps. They are checked against
the package + certificate credential, with validity period and "second
authentication" (`LicenseAuth.cpp`, `LicenseInfo.cpp`). The native engine cannot
be reused outside the signed apps. This is a fact about the vendor binary, not an
obstacle to an independent implementation.

## 2. Parameters actually used (R)

`InstaxNormalParamSP1.yml` and `SP2.yml` are identical (same SHA-256). The
non-neutral values in SP2 are:

| Block | Key | Value |
|---|---|---|
| setup | `setupSwitch` | true |
| setup | `awbLevel` | 100 |
| setup | `aeLevel` | 100 |
| setup | `aeMode` | 0 |
| setup | `algorithmLevel` | 0 |
| setup | `hypertone.mode` | 2 (keys 0) |
| preference | `colorCorrection` | true |
| preference | `colorTable.mode` | 1 |
| structure | `sharpness` | mode 1, power 35 |
| structure | `beautifulSkin`, `noiseReduction` | sw false |
| structure | `textureEnhance` | mode 0 |
| structure | `magnify` | 1.0 |

Every hue, saturation, balance, gamma, DCMY and skin offset is 0.

`MODE_softfocus.yml` differs only in these keys:

- `colorTable.mode` 0
- `colorCorrection` false
- `hypertone.mode` 0
- sharpness power 0
- `softFocus` sw true, power 59

`ffaipii.ini` sets `MidOutputType=5` and `AutoExposureMode=0`. Only its log path
differs between apps.

**Per-printer-model profiles: none.** The parameter tree is identical in all three
apps. Every call uses mode SP2. The output profile is hard-coded to
`Profiles/Output/sRGB10_8.prf`, so the engine outputs sRGB and is not a printer
profile.

The `MAKE`/`MODL` profile tags and `Setup/Dsc/DscMdl.idx` refer to *camera*
models. `DscMdl.idx` holds Fujifilm DSC names such as FinePix and DS-xxx, encoded
as 3-digit decimal ASCII groups. The FFPF `MODL` values EZ2/RD3/FCD/LUKE are
Frontier-class devices. These are selected through
`CFdiaProfileMgr::GetProfileFromExifTag` (call at 0x16dbcc in
`libi2fdiawrapper.so`). With NULL EXIF this per-camera path cannot apply.

The fallback input profile is one of:

- `Profiles/Input/Dsc/DscInput.prf` (0x16dd28)
- `…/DscInputDisplayP3.prf` (0x16dc44, not shipped)
- `Cmn/CmnInput.prf` (0x1725f4)

Which one is used for EXIF-less images is **U**.

## 3. Stages (S unless marked)

The stage inventory comes from:

- CIP engine source paths in `libffaipii`/`libi2fdiawrapper`
- `fdia::Create*Profile` exports in `libfdiaprofile`
- `CFdiaSetupUserPrm` accessors in `libfdiasetup`
- the profile `DumpText` formats

| Stage | Engine / evidence | Data | Active in SP2 |
|---|---|---|---|
| Input linearisation 8→12 bit | `INPUT_1DTBL1`/`INPUT_8TO12_1DTBL` unit (FFPF type 1/2), `CIP_FdiaConvColor1D` | `Profiles/Input/*.prf` | yes |
| Auto setup: AE | `CFdiaSetupUserPrm::{Get,Set}AutoExposureLevel/AeMode`, `CFdiaExpCondData`/`ExpSearchData` | `Setup/Dsc/DscAeClst.dat`, `AeCorrectCoef.dat`, `Dkeyshift.dat`, `dscreso.dat` (opened in 0xcb624/0xce93c, `libfdiasetup`) | yes (level 100) |
| Auto setup: AWB / light source | `…AutoWhiteBalanceLevel`, boosted classifiers `suns-*`, `tung-*` (sunlight / tungsten, `*-bst.bin` + `*-cnf.bin`) | `Awb.dat`, `Preference/Raw/LightSource.dat` | yes (level 100) |
| Density / CMY balance, gamma, contrast | `SetDcmyCorrection`, `SetContrast`, `lutw.dat`/`lutw12.dat`, `Setup12_10.dat` | Setup | neutral user offsets, but the auto result still applies |
| HyperTone (dodging/local tone, skin channel) | `cip_fdiahypertone*`: IIR forward/backward filter, gray image, light average, make-delta, skin-channel / skin-probability map, and a hybrid variant with max filter | `ProHytPrm.dat` | mode 2 (hybrid is likely, mode→class mapping **U**) |
| Colour preference / colour correction | `CExSrgbDPreference`, `CFdiaPreferenceUserPrm` (scene curve, colour table, skin, RGBCMY), `CIP_FdiaConvColor3D` | `Preference/Common/ColorDataExSrgbD.dat` (string in `libfdiapreference`), `ColorData.dat` | yes (`colorCorrection`, `colorTable.mode` 1) |
| Sharpness ("hyper sharpness", Y-USM) | `CNormalSharpnessService`/`CNormalUsmDoubleService` (`libfdiasetup` `SharpnessService.cpp`), `CIP_FdiaYUsm`/`YUsmDouble` | `Setup/Common/SharpRefParam.dat` (text-headed table `TableType:PRM_EDHH`), profile tag `SHRP`, `Sharpness/Profiles/*` | mode 1, power 35 |
| Multi-frequency texture / noise | `cip_fdiamfp*` (rank 4/5/6 enhance, texture coefficients), `cip_fdiaanalyzenoise` | `MFPPrm.dat`, `FilterPrm.dat` | off (`textureEnhance` 0, NR off) |
| Beautiful skin | `CreateBeautifulSkinProfile` | none | off |
| Soft focus | `cip_fdiasoftfocus` (IIR) | `SoftFocusPrm.dat` | mode 2 only |
| Output | `OUTPUT_10TO8_1DTBL` in `sRGB10_8.prf` | none | yes (R: hard-coded path) |

**Profile container FFPF (R, directory level):**

- Magic `FFPF`, u32 version 1, u32, u32 tag count.
- Then {fourcc, offset, length} entries. Tags: `DATA`, `HUBC` (hub colour space),
  `DEVC`, `MAKE`, `MODL`, `DDEP`/`HDEP` (device and hub bit depth), `SHRP`, `MATR`.
- `DATA` holds the stage count and units. Each unit is
  {u32 data type, u32 name length, name, u32 in-type, u32 out-type, payload}.
- Observed types: 1 = 1D table, 2 = coefficient, 3 = 3D LUT, 4 = matrix,
  0x10000030 = hyper-sharp.
- Example: `sRGB8_12.prf` is one COEF unit with per-channel factor 16.0 (8→12 bit).

Unit internals beyond these headers are **U**. The library includes bzip2, so
some payloads may be compressed.

**Not recovered:**

- The AE/AWB estimators: cluster, classifier and regression maths.
- HyperTone filter coefficients.
- How the scene/colour-table preference is applied.
- The sharpness-service mapping from power 35 to Y-USM radius and gain.
- The analysis-image size used inside AutoSetup.

These live in stripped C++ in `libfdiasetup` (about 1.2 MB, 123 exports, little
`.eh_frame` coverage) and in the CIP engines. None is described numerically here.

## 4. Unsharp: `libsharpness.so` against `client/src/sharpness.rs` (R)

### Native path

**Entry.** `applyUnsharpFilter(in, out, f32 sigma, f32 amount, f32 threshold)`.
The single caller is in `LinkPlusMultiPrintActivity`, which passes the defaults
3.0f/0.5f/0.8f (mask 14 at the `I3` call). The path runs only for "double density"
printing (`isDoubleDensityPrintEnabled` and `!g4()` and `bVar.l()`) and works on
a temporary file, as follows:

- The source is drawn to a fixed **1200×1600** ARGB canvas (`C1907t.q`, Paint
  flags 6). The JADX output shows `i10 / getWidth()`. Whether that is integer
  division is unverified.
- The canvas is saved as JPEG.
- `cv::imread(path, -1)` (0x7ac0) reads it: decoded by OpenCV, BGR, EXIF
  orientation not applied.
- `CV_Assert(type==CV_8UC3)` (0x6f84).
- The filter runs.
- `cv::imwrite` writes the result with no parameters (OpenCV JPEG defaults).
- The floats are widened to double (0x7ae8).

**`UnsharpMaskFilterType2` constructor (0x5d50).** It creates a Gaussian
(σ = sigma). If `threshold > 0` it also creates a mask Gaussian with σ = 1.0.

**Gaussian (0x5c44).** `cv::GaussianBlur(src, dst, Size(0,0), σ, σ, 4 /*REFLECT_101*/, ALGO_HINT_DEFAULT)`.
The AlgorithmHint signature means OpenCV ≥ 4.10. `libopencv_java4.so` is not
among the acquired inputs, so the exact OpenCV build is unknown.

**Apply (0x622c):**

1. `FromNonLinearToLinearLUT` (0x6d24) builds a 256-entry f32 LUT. Each entry is
   computed in double, with x = i/255, as x ≤ 0.04045 ? x/12.92 : pow((x+0.055)/1.055, 2.4).
   `cv::LUT` then produces CV_32FC3.
2. `ApplyUsm` (0x5ddc):
   - D = src − blur(src).
   - If a mask Gaussian exists: L = `FromLienarToLuminance`(2·|D|). That function
     (0x7348) is `cv::transform` with a 1×3 CV_32F matrix
     (0.060608f, 0.716904f, 0.222488f) on B,G,R, followed by f32
     linear→nonlinear (`FromLinearToNonLinear(const Mat&)`, near 0x7084,
     asserts CV_32F).
   - m = `cv::threshold`(L, thr, 1.0, BINARY), i.e. L > thr.
   - m is blurred with σ = 1.
   - D *= m per pixel, broadcast over channels (loop at 0x5fb8, NEON fmul).
   - result = src + amount·D (MatExpr AddEx → scaleAdd/addWeighted, f32).
3. Output loop (0x6360):
   - v = (double)f32.
   - v ≤ 0.0031308 ? 12.92·v : fmadd(1.055, pow(v, 1/2.4), −0.055).
   - The result is rounded to f32, multiplied by 255.0 and rounded with `frintx`
     (ties-to-even).
   - It is clamped to 0..255 and stored as u8 for every channel. The Mat type
     equals the input type.

### Static evidence for equivalence

The following all match:

- the sRGB constants, including INV_GAMMA = 1/2.4 and divisor 0.055 + 1.0 = 1.055 exactly in f64
- linear light in f32 storage
- the auto kernel size round(8σ+1)|1 for float images (25 for σ=3, 9 for σ=1)
- REFLECT_101 borders
- f32 coefficients identical to the Rust f32 literals, with a correct BGR→RGB mapping
- the factor 2 (exact)
- the strict ">" binary mask
- the σ=1 mask blur
- per-pixel mask broadcast
- `threshold == 0` disabling the mask
- the final f32 rounding, then ×255, ties-to-even, clamp

### Discrepancies and their effect

| # | Native | Rust | Effect |
|---|---|---|---|
| 1 | Applied only in the double-density path, at **1200×1600**, to a JPEG the app encoded first | Applied to the resized RGB at the target print size (`imaging.rs:273`), before the LUT | **Largest.** At 600×800, σ=3 px is twice the relative radius, and the native path also sharpens JPEG artefacts. The look differs, not just individual pixels. |
| 2 | Input decoded by OpenCV/libjpeg-turbo, output re-encoded with `imwrite` defaults | `image` crate decode/encode | Expect ±1–2 LSB before filtering and different final JPEG bytes. |
| 3 | Gaussian taps: each `exp` rounded to f32, the f32 values summed in double, then normalised and rounded again | Taps computed and normalised in f64, rounded once | Taps can differ by 1 f32 ulp. |
| 4 | OpenCV row filter (vectorised, may fuse multiply-add on NEON) and symmetric column filter (ky0·S0 + Σky·(S+k + S−k)) | Index-order multiply-accumulate in f32 | About 1e-7 relative error. Occasionally ±1 LSB in output, or a mask flip at the threshold. |
| 5 | Threshold compares the f32 nonlinear luminance against (float)0.8f | Compares the unrounded f64 against 0.8 f64 | The mask differs only for luminance in (0.8, ≈0.8000000179]. |
| 6 | Luminance summed B, G, R in f32 (possibly fused) | Summed R, G, B in f32 | Last-ulp differences. They matter only at the threshold. |
| 7 | `fmadd` (one rounding) in linear→nonlinear | mul then sub in f64 | Below f32 resolution, so almost never visible. |
| 8 | amount applied with f32 multiply in scaleAdd/addWeighted | f64 then cast to f32 | Identical for amount = 0.5 (a power of two). Other amounts differ by up to 1 f32 ulp. |
| 9 | No range checks. Non-8UC3 input raises a `cv::Exception` | Rejects σ > 32, amount > 10, threshold > 1 | API-only difference. |

With the defaults, #3–#8 should change at most a few pixels by ±1 LSB at equal
input. #1 and #2 dominate. A numeric comparison against native output still
requires executing OpenCV, which was not possible here.

## 5. Feasibility of an independent "auto correction"

**A faithful re-implementation is not feasible from this static evidence.** The
active SP2 stages are AE, AWB light-source classification, auto density/gamma,
HyperTone mode 2, colour-table preference with colour correction, and hyper
sharpness at power 35. They are driven by proprietary statistical data:

- clusters and boosted classifiers
- 3D colour tables, about 1 MB
- the sharpness reference table

Their maths sits in stripped code that was not recovered. The data files are also
vendor assets that this project must not redistribute.

The minimum faithful subset is the parts that *are* recovered:

- the Java envelope: limit displayed height to 800, enhance, upscale back with a
  bilinear filter
- EXIF ignored
- the sRGB in/out working space
- neutral user offsets
- a hook for the auto stages
- the FFPF container reader (directory level)

On their own these do not reproduce the vendor look. An honest Rust option would
be clearly labelled "auto levels/white balance (not FDIA)", and could reuse the
recovered envelope.

Recovering FDIA properly needs one of two things:

- months of `libfdiasetup` and CIP-engine decompilation, or
- dynamic tracing on a device with the signed app. That would produce
  before/after pairs per stage through the `_setup.txt` / `PRF_DumpText` debug
  dumps. Their filenames appear in `libffaipii`, and they appear to be tied to the
  `Logging` INI switch, which is not verified.
