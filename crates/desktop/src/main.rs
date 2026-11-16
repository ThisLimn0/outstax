#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use anyhow::Context as _;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, ColorImage, FontId, Layout, Painter, Pos2, Rect,
    RichText, Sense, Shape, Stroke, StrokeKind, Vec2,
};
use image::DynamicImage;
use log::{error, info, warn};
use outstax_client::{
    ble::{self, BleTransport, Device},
    imaging::{self, Adjustments, Crop, Fit, Processing},
    logging, PrinterInfo, Session,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const ACCENT: Color32 = Color32::from_rgb(0, 138, 128);
const ACCENT_HOVER: Color32 = Color32::from_rgb(0, 160, 149);
const FULL_UV: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

/// Last status the printer reported. The GUI connects per action, so this may be stale.
#[derive(Clone)]
struct Printer {
    battery: u8,
    charging: bool,
    film: u8,
    dims: (u32, u32),
    max_bytes: u32,
    lut: bool,
    design: bool,
}
impl Printer {
    fn new(info: &PrinterInfo) -> Self {
        Self {
            battery: info.battery.percent,
            charging: info.status.charging,
            film: info.status.film_remaining,
            dims: (info.image.width as u32, info.image.height as u32),
            max_bytes: info.image.max_bytes,
            lut: info.image.options & 8 != 0,
            design: info.image.options2.unwrap_or(0) & 1 != 0,
        }
    }
}

#[derive(Clone)]
enum Source {
    Photo {
        full: Arc<DynamicImage>,
        /// Downscaled copy for fast previews.
        small: Arc<DynamicImage>,
    },
    /// A saved print, resent byte-for-byte like the CLI's `--prepared [--lut-applied]`.
    Prepared {
        jpeg: Arc<Vec<u8>>,
        rich: bool,
        design: bool,
        dims: (u32, u32),
    },
}

#[derive(Clone, PartialEq)]
struct PreviewKey {
    source: u64,
    adjust: Adjustments,
    contain: bool,
    lut: Option<String>,
    dims: (u32, u32),
    max_bytes: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Job {
    Status,
    Test,
    Print,
}

enum Event {
    Devices(Result<Vec<Device>, String>),
    /// `printer: Some(x)` replaces the shown printer status; `None` keeps it.
    Done {
        message: String,
        error: bool,
        printer: Option<Option<Printer>>,
        gone: bool,
    },
    Loaded(Result<(String, Source), String>),
    Preview(PreviewKey, Result<ColorImage, String>),
    History(Vec<(PathBuf, ColorImage, f32)>),
}

#[derive(Clone)]
struct Notify {
    tx: Sender<Event>,
    ctx: egui::Context,
}
impl Notify {
    fn send(&self, event: Event) {
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }
}

type LutCache = Arc<Mutex<Option<(String, Arc<Vec<u8>>)>>>;
fn lut_bytes(cache: &LutCache, path: &str) -> anyhow::Result<Arc<Vec<u8>>> {
    let mut cache = cache.lock().unwrap();
    if let Some((cached, bytes)) = cache.as_ref() {
        if cached == path {
            return Ok(bytes.clone());
        }
    }
    let bytes =
        Arc::new(std::fs::read(path).with_context(|| format!("could not read LUT file {path}"))?);
    *cache = Some((path.to_owned(), bytes.clone()));
    Ok(bytes)
}

fn fit(contain: bool) -> Fit {
    if contain {
        Fit::Contain
    } else {
        Fit::Crop
    }
}

/// Per-user data folder from std env vars only.
fn data_dir() -> Option<PathBuf> {
    let var = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        var("APPDATA")?
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library/Application Support")
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share")))?
    };
    Some(base.join("outstax"))
}
/// One-time move of the folder used before the rename (`instax-desktop`) to `outstax`.
/// Returns (old, new, rename result) when a move was attempted. On failure nothing is
/// copied and the app starts with the new empty folder.
fn migrate_data_dir() -> Option<(PathBuf, PathBuf, std::io::Result<()>)> {
    let new = data_dir()?;
    let old = new.with_file_name("instax-desktop");
    if new.exists() || !old.is_dir() {
        return None;
    }
    let result = std::fs::rename(&old, &new);
    Some((old, new, result))
}
fn history_dir() -> Option<PathBuf> {
    Some(data_dir()?.join("history"))
}
/// `outstax-desktop.log` (+ `.1`) and `traces/`.
fn logs_dir() -> Option<PathBuf> {
    Some(data_dir()?.join("logs"))
}
const KEEP_TRACES: usize = 20;

fn open_folder(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map(drop)
}

/// Last-used printer as two lines: BLE id, then display name.
fn load_last_printer() -> Option<Device> {
    let text = std::fs::read_to_string(data_dir()?.join("last-printer.txt")).ok()?;
    let mut lines = text.lines();
    let id = lines.next()?.trim().to_owned();
    let name = lines.next().unwrap_or(&id).trim().to_owned();
    (!id.is_empty()).then_some(Device {
        id,
        name,
        address: String::new(),
        rssi: None,
        has_print_service: true,
    })
}
fn save_last_printer(device: &Device) -> std::io::Result<()> {
    let Some(dir) = data_dir() else {
        return Ok(());
    };
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("last-printer.txt"),
        format!(
            "{}
{}
",
            device.id, device.name
        ),
    )
}

/// The client reports a vanished printer with these sentences (see ble.rs).
// ponytail: matches message text; switch to a typed error if ble.rs grows one.
fn printer_gone(message: &str) -> bool {
    message.contains("Printer not found") || message.contains("Printer disconnected")
}

/// File names record how the JPEG was made: `<unix ms>-<natural|rich>[-design].jpg`.
fn history_flags(path: &Path) -> (bool, bool) {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    (stem.contains("-rich"), stem.contains("-design"))
}

fn save_history(jpeg: &[u8], options: [u8; 4]) -> anyhow::Result<()> {
    let dir = history_dir().context("no user data folder is set")?;
    std::fs::create_dir_all(&dir)?;
    let ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let mode = if options[1] & 8 != 0 {
        "rich"
    } else {
        "natural"
    };
    let design = if options[2] & 1 != 0 { "-design" } else { "" };
    std::fs::write(dir.join(format!("{ms}-{mode}{design}.jpg")), jpeg)?;
    Ok(())
}

/// The exact bytes and header options sent to the printer.
fn job_jpeg(
    source: Source,
    info: &PrinterInfo,
    adjust: Adjustments,
    contain: bool,
    lut: Option<String>,
    design: bool,
    cache: &LutCache,
) -> anyhow::Result<(Vec<u8>, [u8; 4])> {
    match source {
        Source::Photo { full, .. } => {
            let table = lut.as_deref().map(|p| lut_bytes(cache, p)).transpose()?;
            let jpeg = imaging::prepare_with_options(
                (*full).clone(),
                info.image.width as u32,
                info.image.height as u32,
                info.image.max_bytes as usize,
                fit(contain),
                Processing {
                    lut: table.as_deref().map(Vec::as_slice),
                    design,
                    adjust,
                    ..Default::default()
                },
            )?
            .jpeg;
            Ok((
                jpeg,
                [2, if table.is_some() { 8 } else { 0 }, u8::from(design), 0],
            ))
        }
        // No second LUT pass: the flag only states how the saved JPEG was made.
        Source::Prepared {
            jpeg, rich, design, ..
        } => Ok((
            (*jpeg).clone(),
            [2, if rich { 8 } else { 0 }, u8::from(design), 0],
        )),
    }
}

struct App {
    runtime: tokio::runtime::Runtime,
    notify: Notify,
    rx: Receiver<Event>,
    devices: Vec<Device>,
    device: Option<Device>,
    printer: Option<Printer>,
    /// The last action failed because the printer was off, out of range or dropped the link.
    offline: bool,
    source: Option<Source>,
    source_name: String,
    source_id: u64,
    adjust: Adjustments,
    contain: bool,
    rich: bool,
    lut_path: String,
    lut_cache: LutCache,
    design: bool,
    preview: Option<egui::TextureHandle>,
    /// Fill-mode crop (0..1 image units) the shown preview was rendered with, so a drag can move
    /// and scale it at once while the next render catches up.
    preview_uv: Option<Rect>,
    requested: Option<PreviewKey>,
    changed_at: Option<Instant>,
    rendering: bool,
    loading: bool,
    history: Vec<(PathBuf, egui::TextureHandle, f32)>,
    show_history: bool,
    show_printers: bool,
    show_settings: bool,
    confirm_print: bool,
    busy: bool,
    status: String,
    status_error: bool,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        ctx.all_styles_mut(|s| {
            s.visuals.selection.bg_fill = ACCENT;
            s.visuals.selection.stroke.color = Color32::WHITE;
            s.visuals.hyperlink_color = ACCENT;
        });
        let (tx, rx) = mpsc::channel();
        let device = load_last_printer();
        let status = match &device {
            Some(d) => format!(
                "Last used printer: {}. Click Reconnect to check it.",
                d.name
            ),
            None => "Click Connect to find your printer, then open a photo.".into(),
        };
        Self {
            runtime: tokio::runtime::Runtime::new().expect("async runtime"),
            notify: Notify {
                tx,
                ctx: ctx.clone(),
            },
            rx,
            devices: vec![],
            device,
            printer: None,
            offline: false,
            source: None,
            source_name: String::new(),
            source_id: 0,
            adjust: Adjustments::default(),
            contain: false,
            rich: false,
            lut_path: String::new(),
            lut_cache: LutCache::default(),
            design: false,
            preview: None,
            preview_uv: None,
            requested: None,
            changed_at: None,
            rendering: false,
            loading: false,
            history: vec![],
            show_history: false,
            show_printers: false,
            show_settings: false,
            confirm_print: false,
            busy: false,
            status,
            status_error: false,
        }
    }
    fn set_status(&mut self, message: impl Into<String>, error: bool) {
        self.status = message.into();
        self.status_error = error;
        if error {
            error!("status: {}", self.status);
        } else {
            info!("status: {}", self.status);
        }
    }
    fn dims(&self) -> (u32, u32) {
        self.printer.as_ref().map_or((600, 800), |p| p.dims)
    }
    /// The JPEG size limit a print would use, so the preview shows the real compression.
    fn max_bytes(&self) -> usize {
        self.printer.as_ref().map_or_else(
            || {
                let (w, h) = self.dims();
                imaging::default_max_bytes(w, h)
            },
            |p| p.max_bytes as usize,
        )
    }
    fn frame_aspect(&self) -> f32 {
        let (w, h) = match &self.source {
            Some(Source::Prepared { dims, .. }) => *dims,
            _ => self.dims(),
        };
        w as f32 / h as f32
    }
    /// The open photo's Fill-mode crop for `a`, clamped, with its source rectangle in 0..1 image
    /// units. `None` in Fit mode and for saved prints.
    fn crop(&self, a: Adjustments, contain: bool) -> Option<(Crop, Rect)> {
        let Some(Source::Photo { small, .. }) = &self.source else {
            return None;
        };
        if contain {
            return None;
        }
        let (mut w, mut h) = (small.width(), small.height());
        if a.quarter_turns % 2 == 1 {
            (w, h) = (h, w);
        }
        let (dw, dh) = self.dims();
        let (crop, [l, t, cw, ch]) = a.crop.fit(w, h, dw as f64 / dh as f64);
        let (w, h) = (w as f64, h as f64);
        let uv = Rect::from_min_size(
            pos2((l / w) as f32, (t / h) as f32),
            vec2((cw / w) as f32, (ch / h) as f32),
        );
        Some((crop, uv))
    }
    fn lut_choice(&self) -> Option<String> {
        self.rich.then(|| self.lut_path.trim().to_owned())
    }

    fn scan(&mut self) {
        info!("action: scan for printers");
        self.busy = true;
        self.set_status("Looking for printers…", false);
        let n = self.notify.clone();
        self.runtime.spawn(async move {
            let found = ble::discover(Duration::from_secs(5)).await;
            n.send(Event::Devices(
                found
                    .map(|d| d.into_iter().map(|d| d.info).collect())
                    .map_err(|e| format!("Couldn't search for printers: {e:#}")),
            ));
        });
    }

    fn run(&mut self, job: Job) {
        let Some(device) = self.device.clone() else {
            return;
        };
        let source = self.source.clone();
        let name = match (job, &source) {
            (Job::Status, _) => "status",
            (Job::Test, _) => "test-upload",
            (Job::Print, Some(Source::Prepared { .. })) => "reprint",
            (Job::Print, _) => "print",
        };
        info!("action: {name} on {:?} ({})", device.name, device.id);
        let (adjust, contain, lut, design) =
            (self.adjust, self.contain, self.lut_choice(), self.design);
        let cache = self.lut_cache.clone();
        let n = self.notify.clone();
        self.busy = true;
        self.set_status(
            match job {
                Job::Status => format!("Connecting to {}…", device.name),
                Job::Test => "Uploading the image without printing…".into(),
                Job::Print => "Sending the print job…".into(),
            },
            false,
        );
        self.runtime.spawn(async move {
            let started = Instant::now();
            let result = async {
                let transport = tokio::time::timeout(
                    Duration::from_secs(25),
                    BleTransport::connect(&device.id, Duration::from_secs(5), 0, Duration::ZERO),
                )
                .await
                .context("the printer did not answer within 25 seconds")??;
                let mut session = Session::new(transport);
                start_trace(&mut session, name);
                let outcome = async {
                    if job != Job::Status {
                        let identity = session.identity().await?;
                        // The app skips failed connect-time steps (Session logs them as warnings);
                        // a dropped link still fails at probe.
                        let _ = session.initialize(&identity).await;
                    }
                    let info = session.probe().await?;
                    let mut printer = Printer::new(&info);
                    let message = if job == Job::Status {
                        let mut m = format!("Connected to {}.", device.name);
                        if info.status.cover_open {
                            m += " The film door is open.";
                        }
                        if info.status.error {
                            m += " The printer reports an error.";
                        }
                        m
                    } else {
                        let source = source.context("open a photo first")?;
                        let (jpeg, options) =
                            job_jpeg(source, &info, adjust, contain, lut, design, &cache)?;
                        let commit = job == Job::Print;
                        session.upload(&jpeg, &info, commit, options).await?;
                        if !commit {
                            "Test upload finished: the printer accepted every block, then the upload was cancelled before its end. Nothing is staged and no film was used.".to_owned()
                        } else {
                            let status = session.wait_for_print(Duration::from_secs(180)).await?;
                            printer.film = status.film_remaining;
                            let saved = match save_history(&jpeg, options) {
                                Ok(()) => " Saved to History.".into(),
                                Err(e) => format!(" Could not save it to History: {e:#}"),
                            };
                            format!("The printer reports the print finished. Film left: {}.{saved}", status.film_remaining)
                        }
                    };
                    Ok::<_, anyhow::Error>((message, printer))
                }
                .await;
                let closed = session.disconnect().await;
                let outcome = outcome?;
                closed?;
                Ok::<_, anyhow::Error>(outcome)
            }
            .await;
            let seconds = started.elapsed().as_secs_f32();
            match &result {
                Ok(_) => info!("{name} job succeeded in {seconds:.1} s"),
                // The full error chain is logged when it reaches the status line.
                Err(_) => error!("{name} job failed after {seconds:.1} s"),
            }
            n.send(match result {
                Ok((message, printer)) => Event::Done {
                    message,
                    error: false,
                    printer: Some(Some(printer)),
                    gone: false,
                },
                Err(e) => {
                    let message = format!("Stopped: {e:#}");
                    let gone = printer_gone(&message);
                    Event::Done {
                        printer: (gone || job == Job::Status).then_some(None),
                        message,
                        error: true,
                        gone,
                    }
                }
            });
        });
    }

    fn open(&mut self, path: PathBuf) {
        info!("action: open photo {}", path.display());
        self.loading = true;
        self.show_history = false;
        self.set_status(format!("Opening {}…", path.display()), false);
        let n = self.notify.clone();
        self.runtime.spawn_blocking(move || {
            let loaded = imaging::load(&path)
                .map(|full| {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    let small = Arc::new(full.thumbnail(1600, 1600));
                    (
                        name,
                        Source::Photo {
                            full: Arc::new(full),
                            small,
                        },
                    )
                })
                .map_err(|e| format!("Couldn't open {}: {e:#}", path.display()));
            n.send(Event::Loaded(loaded));
        });
    }

    fn pick_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Photos", &["jpg", "jpeg", "png", "webp"])
            .pick_file()
        {
            self.open(path);
        }
    }

    fn reopen(&mut self, path: &Path, ctx: &egui::Context) {
        info!("action: open saved print {} for reprint", path.display());
        let read = std::fs::read(path)
            .map_err(anyhow::Error::from)
            .and_then(|jpeg| imaging::load_memory(&jpeg).map(|decoded| (jpeg, decoded)));
        match read {
            Ok((jpeg, (pixels, size))) => {
                let (rich, design) = history_flags(path);
                let dims = (size[0] as u32, size[1] as u32 / if design { 2 } else { 1 });
                self.source_id += 1;
                self.source = Some(Source::Prepared {
                    jpeg: Arc::new(jpeg),
                    rich,
                    design,
                    dims,
                });
                self.source_name = "Saved print".into();
                self.preview_uv = None;
                self.preview = Some(ctx.load_texture(
                    "preview",
                    ColorImage::from_rgb(size, &pixels),
                    Default::default(),
                ));
                self.show_history = false;
                self.set_status(
                    format!(
                        "Saved print loaded. It is resent exactly as before ({}{}).",
                        if rich { "Rich" } else { "Natural" },
                        if design { ", Design Print" } else { "" }
                    ),
                    false,
                );
            }
            Err(e) => self.set_status(format!("Couldn't open the saved print: {e:#}"), true),
        }
    }

    fn load_history(&mut self) {
        info!("action: open history");
        let n = self.notify.clone();
        self.runtime.spawn_blocking(move || {
            let mut paths: Vec<PathBuf> = history_dir()
                .and_then(|d| std::fs::read_dir(d).ok())
                .into_iter()
                .flatten()
                .filter_map(|e| Some(e.ok()?.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "jpg"))
                .collect();
            paths.sort_by(|a, b| b.cmp(a)); // millisecond names: newest first
                                            // ponytail: decodes every saved print on each visit; page or cache thumbnails if history grows large.
            let items = paths
                .into_iter()
                .filter_map(|path| {
                    let image = imaging::load(&path).ok()?;
                    let wire = if history_flags(&path).1 { 2.0 } else { 1.0 };
                    let aspect = image.width() as f32 * wire / image.height() as f32;
                    let thumb = image.thumbnail(240, 240).to_rgb8();
                    let size = [thumb.width() as usize, thumb.height() as usize];
                    Some((path, ColorImage::from_rgb(size, thumb.as_raw()), aspect))
                })
                .collect();
            n.send(Event::History(items));
        });
    }

    fn refresh_preview(&mut self, ctx: &egui::Context) {
        let Some(Source::Photo { small, .. }) = &self.source else {
            return;
        };
        let key = PreviewKey {
            source: self.source_id,
            adjust: self.adjust,
            contain: self.contain,
            lut: self.lut_choice(),
            dims: self.dims(),
            max_bytes: self.max_bytes(),
        };
        if self.requested.as_ref() == Some(&key) {
            self.changed_at = None;
            return;
        }
        // Throttle: render at most every 120 ms while a slider moves, one job at a time.
        let since = *self.changed_at.get_or_insert_with(Instant::now);
        if self.rendering || since.elapsed() < Duration::from_millis(120) {
            ctx.request_repaint_after(Duration::from_millis(40));
            return;
        }
        self.rendering = true;
        self.changed_at = None;
        self.requested = Some(key.clone());
        let (small, cache, n) = (small.clone(), self.lut_cache.clone(), self.notify.clone());
        self.runtime.spawn_blocking(move || {
            let result = (|| {
                let lut = key
                    .lut
                    .as_deref()
                    .map(|p| lut_bytes(&cache, p))
                    .transpose()?;
                // Same pipeline and byte limit as printing, minus Design Print's wire warp.
                let jpeg = imaging::prepare_with_options(
                    (*small).clone(),
                    key.dims.0,
                    key.dims.1,
                    key.max_bytes,
                    fit(key.contain),
                    Processing {
                        lut: lut.as_deref().map(Vec::as_slice),
                        adjust: key.adjust,
                        ..Default::default()
                    },
                )?
                .jpeg;
                let (pixels, size) = imaging::load_memory(&jpeg)?;
                Ok::<_, anyhow::Error>(ColorImage::from_rgb(size, &pixels))
            })()
            .map_err(|e| format!("Preview failed: {e:#}"));
            n.send(Event::Preview(key, result));
        });
    }

    fn handle(&mut self, ctx: &egui::Context, event: Event) {
        match event {
            Event::Devices(found) => {
                self.busy = false;
                // A scan that misses the selected printer (or fails with "not found") means it is off.
                let missing = match &found {
                    Ok(d) => self
                        .device
                        .as_ref()
                        .is_some_and(|s| !d.iter().any(|d| d.id == s.id)),
                    Err(e) => printer_gone(e),
                };
                if missing {
                    self.printer = None;
                    self.offline = true;
                }
                match found {
                    Ok(d) if d.is_empty() => self.set_status(
                        "No printers found. Turn the printer on, keep it close, and make sure Bluetooth is on.",
                        true,
                    ),
                    Ok(d) => {
                        self.set_status(format!("Found {} printer(s). Pick yours from the list.", d.len()), false);
                        self.devices = d;
                    }
                    Err(e) => self.set_status(e, true),
                }
            }
            Event::Done {
                message,
                error,
                printer,
                gone,
            } => {
                self.busy = false;
                self.set_status(message, error);
                if let Some(printer) = printer {
                    self.printer = printer;
                }
                self.offline = gone;
                if !error {
                    if let Some(device) = &self.device {
                        // Convenience only: failing to remember the printer is not worth an error.
                        let _ = save_last_printer(device);
                    }
                }
            }
            Event::Loaded(loaded) => {
                self.loading = false;
                match loaded {
                    Ok((name, source)) => {
                        self.set_status(format!("Opened {name}."), false);
                        self.source_name = name;
                        self.source = Some(source);
                        self.source_id += 1;
                        self.adjust = Adjustments::default();
                        self.preview = None;
                    }
                    Err(e) => self.set_status(e, true),
                }
            }
            Event::Preview(key, result) => {
                self.rendering = false;
                match result {
                    Ok(image) if key.source == self.source_id => {
                        self.preview = Some(ctx.load_texture("preview", image, Default::default()));
                        self.preview_uv = self.crop(key.adjust, key.contain).map(|c| c.1);
                    }
                    Ok(_) => {}
                    Err(e) => self.set_status(e, true),
                }
            }
            Event::History(items) => {
                self.history = items
                    .into_iter()
                    .map(|(path, image, aspect)| {
                        let texture =
                            ctx.load_texture(path.to_string_lossy(), image, Default::default());
                        (path, texture, aspect)
                    })
                    .collect();
            }
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            self.pill(ui);
            ui.add_space(8.0);
            let label = if self.device.is_some() {
                "Change printer"
            } else {
                "Connect"
            };
            if ui
                .add_enabled(
                    !self.busy,
                    egui::Button::new(label).min_size(vec2(0.0, 30.0)),
                )
                .clicked()
            {
                self.show_printers = true;
                if self.devices.is_empty() {
                    self.scan();
                }
            }
            // No live link is kept, so "Reconnect" is just a status check on demand.
            let (refresh, hint) = if self.printer.is_none() {
                ("⟳ Reconnect", "Connect to the selected printer again")
            } else {
                ("⟳", "Check printer status")
            };
            if ui
                .add_enabled(
                    !self.busy && self.device.is_some(),
                    egui::Button::new(refresh).min_size(vec2(30.0, 30.0)),
                )
                .on_hover_text(hint)
                .clicked()
            {
                self.run(Job::Status);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if round_button(ui, 18.0, "⚙", "", false, true)
                    .on_hover_text("Settings")
                    .clicked()
                {
                    self.show_settings = !self.show_settings;
                }
            });
        });
    }

    fn pill(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(400.0, 36.0), Sense::hover());
        let v = ui.visuals();
        let (text, weak) = (v.text_color(), v.weak_text_color());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 18.0, v.extreme_bg_color);
        p.rect_stroke(
            rect,
            18.0,
            v.widgets.noninteractive.bg_stroke,
            StrokeKind::Inside,
        );
        let cy = rect.center().y;
        bluetooth(
            &p,
            pos2(rect.left() + 20.0, cy),
            if self.printer.is_some() { ACCENT } else { weak },
        );
        let name: String = match &self.device {
            Some(d) => d.name.chars().take(20).collect(),
            None => "Not connected".into(),
        };
        let font = FontId::proportional(14.0);
        let mut x = p
            .text(
                pos2(rect.left() + 34.0, cy),
                Align2::LEFT_CENTER,
                name,
                font.clone(),
                text,
            )
            .right()
            + 14.0;
        let Some(printer) = &self.printer else {
            if self.device.is_some() {
                let (state, color) = if self.offline {
                    ("Printer off or out of range", v.warn_fg_color)
                } else {
                    ("Not connected", weak)
                };
                p.text(pos2(x, cy), Align2::LEFT_CENTER, state, font, color);
            }
            return;
        };
        // Battery outline with a fill proportional to the charge.
        let body = Rect::from_min_size(pos2(x, cy - 6.0), vec2(22.0, 12.0));
        p.rect_stroke(body, 2.0, Stroke::new(1.2_f32, text), StrokeKind::Inside);
        p.rect_filled(
            Rect::from_min_size(pos2(body.right(), cy - 3.0), vec2(2.0, 6.0)),
            0.0,
            text,
        );
        let level = body.shrink(2.5);
        p.rect_filled(
            level.with_max_x(level.left() + level.width() * printer.battery as f32 / 100.0),
            1.0,
            text,
        );
        let battery = format!(
            "{}%{}",
            printer.battery,
            if printer.charging { " ⚡" } else { "" }
        );
        x = p
            .text(
                pos2(body.right() + 7.0, cy),
                Align2::LEFT_CENTER,
                battery,
                font.clone(),
                text,
            )
            .right()
            + 14.0;
        x = p
            .text(
                pos2(x, cy),
                Align2::LEFT_CENTER,
                format!("Film {}/10", printer.film),
                font,
                text,
            )
            .right()
            + 8.0;
        for i in 0..10u8 {
            let seg = Rect::from_min_size(pos2(x + i as f32 * 6.0, cy - 6.0), vec2(4.0, 12.0));
            if i < printer.film {
                p.rect_filled(seg, 1.0, ACCENT);
            } else {
                p.rect_stroke(seg, 1.0, Stroke::new(1.0_f32, weak), StrokeKind::Inside);
            }
        }
    }

    fn edit_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.heading("Adjust");
        ui.add_space(4.0);
        let photo = matches!(self.source, Some(Source::Photo { .. }));
        if matches!(self.source, Some(Source::Prepared { .. })) {
            ui.label(
                "This is a saved print. It is resent exactly as it was printed, so editing is off.",
            );
        }
        if self.lut_path.trim().is_empty() {
            self.rich = false;
        }
        ui.add_enabled_ui(photo, |ui| {
            ui.spacing_mut().slider_width = ui.available_width() - 56.0;
            let a = &mut self.adjust;
            for (label, value) in [
                ("Brightness", &mut a.brightness),
                ("Contrast", &mut a.contrast),
                ("Saturation", &mut a.saturation),
                ("Color temperature", &mut a.temperature),
            ] {
                ui.add_space(6.0);
                ui.label(label);
                ui.add(egui::Slider::new(value, -100..=100));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if round_button(ui, 20.0, "⟳", "Rotate 90°", false, photo).clicked() {
                    self.adjust.quarter_turns = (self.adjust.quarter_turns + 1) % 4;
                    self.adjust.crop = Crop::default();
                }
                if round_button(ui, 20.0, "⟲", "Reset", false, photo).clicked() {
                    self.adjust = Adjustments::default();
                }
            });
            ui.add_space(12.0);
            ui.label("Framing");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.contain, false, "Fill (crop)");
                ui.selectable_value(&mut self.contain, true, "Fit (white margins)");
            });
            ui.add_space(6.0);
            let fill = !self.contain;
            ui.add_enabled(fill, egui::Label::new("Zoom"));
            let zoom = egui::Slider::new(&mut self.adjust.crop.zoom, 1.0..=Crop::MAX_ZOOM)
                .fixed_decimals(1)
                .suffix("×");
            let zoom = ui.add_enabled(fill, zoom);
            if self.contain {
                zoom.on_disabled_hover_text("Crop works in Fill mode");
            }
            ui.add_space(8.0);
            ui.label("Color mode");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.rich, false, "Natural");
                let has_lut = !self.lut_path.trim().is_empty();
                ui.add_enabled(has_lut, egui::Button::selectable(self.rich, "Rich"))
                    .on_disabled_hover_text("Set a Rich-mode LUT file in Settings first")
                    .clicked()
                    .then(|| self.rich = true);
            });
        });
        ui.add_space(12.0);
        ui.small("These adjustments are this app's own filters (color math ported from the open-source LightCraft editor), not the vendor's processing. Automatic correction is not available.");
    }

    fn settings(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .anchor(Align2::RIGHT_TOP, [-16.0, 64.0])
            .show(ctx, |ui| {
                ui.strong("Rich-mode lookup table (LUT)");
                ui.label("A 64 MiB 24-bit RGB table you supply. None is included.");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.lut_path).desired_width(220.0).hint_text("LUT file path"));
                    if ui.button("Choose…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.lut_path = path.display().to_string();
                        }
                    }
                });
                if self.printer.as_ref().is_some_and(|p| !p.lut) {
                    ui.colored_label(ui.visuals().warn_fg_color, "⚠ The printer did not advertise LUT support, so Rich jobs will be refused.");
                }
                ui.separator();
                ui.checkbox(&mut self.design, "Design Print (requires advertised double-density support)");
                if self.design {
                    let note = match &self.printer {
                        Some(p) if !p.design => Some("⚠ The printer did not advertise Design Print support, so the job will be refused."),
                        None => Some("Connect to check whether the printer advertises Design Print support."),
                        _ => None,
                    };
                    if let Some(note) = note {
                        ui.colored_label(ui.visuals().warn_fg_color, note);
                    }
                    ui.label("Experimental. The preview does not show the Design Print warp.");
                }
                if let Some(p) = &self.printer {
                    ui.separator();
                    ui.label(format!("Printer image size {}×{}, up to {} bytes.", p.dims.0, p.dims.1, p.max_bytes));
                }
                ui.separator();
                ui.strong("Logs");
                if let Some(dir) = logs_dir() {
                    ui.label(format!("Log file and protocol traces (traces include the image data): {}", dir.display()));
                    if ui.button("Open log folder").clicked() {
                        info!("action: open log folder");
                        if let Err(e) = open_folder(&dir) {
                            self.set_status(format!("Couldn't open the log folder {}: {e}", dir.display()), true);
                        }
                    }
                } else {
                    ui.label("No user data folder is set, so nothing is logged to disk.");
                }
            });
        self.show_settings &= open;
    }

    fn printers(&mut self, ctx: &egui::Context) {
        let mut open = self.show_printers;
        egui::Window::new("Printers")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::LEFT_TOP, [16.0, 64.0])
            .show(ctx, |ui| {
                ui.set_min_width(300.0);
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Scan again"))
                    .clicked()
                {
                    self.scan();
                }
                ui.add_space(4.0);
                if self.devices.is_empty() {
                    ui.label(if self.busy {
                        "Searching…"
                    } else {
                        "No printers found yet. Turn the printer on and scan."
                    });
                }
                for d in self.devices.clone() {
                    let selected = self.device.as_ref().is_some_and(|s| s.id == d.id);
                    let rssi = d.rssi.map(|r| format!(" · {r} dBm")).unwrap_or_default();
                    if ui
                        .add_enabled(
                            !self.busy,
                            egui::Button::selectable(selected, format!("{}{rssi}", d.name)),
                        )
                        .on_hover_text(&d.id)
                        .clicked()
                    {
                        info!("action: select printer {:?} ({})", d.name, d.id);
                        self.device = Some(d);
                        self.printer = None;
                        self.show_printers = false;
                        self.run(Job::Status);
                    }
                }
            });
        self.show_printers &= open;
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, Sense::click());
        let (frame, photo) = paint_frame(
            ui.painter(),
            area.shrink(24.0).with_max_y(area.bottom() - 52.0),
            self.frame_aspect(),
        );
        if let Some((mut c, uv)) = self.crop(self.adjust, self.contain) {
            let r = ui.interact(photo, ui.id().with("crop"), Sense::click_and_drag());
            if r.dragged() {
                // The photo follows the pointer, so the crop moves the other way.
                let d = r.drag_delta() * uv.size() / photo.size();
                (c.x, c.y) = (c.x - d.x, c.y - d.y);
            }
            if let Some(pos) = r.hover_pos() {
                let factor = ui.input(|i| (i.smooth_scroll_delta.y / 200.0).exp() * i.zoom_delta());
                if factor != 1.0 {
                    // Keep the image point under the cursor in place.
                    let zoom = (c.zoom * factor).clamp(1.0, Crop::MAX_ZOOM);
                    let at = (pos - photo.min) / photo.size();
                    let size = uv.size() * c.zoom / zoom;
                    let centre = uv.min + at * uv.size() + (Vec2::splat(0.5) - at) * size;
                    c = Crop {
                        x: centre.x,
                        y: centre.y,
                        zoom,
                    };
                }
            }
            if r.double_clicked() {
                c = Crop::default();
            }
            if c != self.adjust.crop {
                // Next frame requests the re-render, even if no more input arrives.
                ui.ctx().request_repaint();
                self.adjust.crop = c;
            }
            let cursor = if r.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            };
            r.on_hover_cursor(cursor);
        }
        // Clamped again after this frame's drag or zoom.
        let crop = self.crop(self.adjust, self.contain);
        if let Some((c, _)) = crop {
            self.adjust.crop = c;
        }
        let p = ui.painter();
        match &self.preview {
            Some(texture) => {
                // Until the next render arrives, move and scale the last one to the current crop.
                let shown = match (self.preview_uv, crop) {
                    (Some(old), Some((_, new))) => {
                        let s = photo.size() / new.size();
                        Rect::from_min_size(photo.min + (old.min - new.min) * s, old.size() * s)
                    }
                    _ => photo,
                };
                p.rect_filled(photo, 0.0, Color32::from_gray(228));
                p.with_clip_rect(photo)
                    .image(texture.id(), shown, FULL_UV, Color32::WHITE);
            }
            None => {
                p.rect_filled(photo, 0.0, Color32::from_gray(228));
                if self.source.is_none() && !self.loading {
                    p.text(
                        photo.center(),
                        Align2::CENTER_CENTER,
                        "Drop a photo here or click Open",
                        FontId::proportional(16.0),
                        Color32::from_gray(90),
                    );
                }
            }
        }
        if ui.ctx().input(|i| !i.raw.hovered_files.is_empty()) {
            p.rect_stroke(photo, 0.0, Stroke::new(3.0_f32, ACCENT), StrokeKind::Inside);
        }
        if self.loading || (self.rendering && self.preview.is_none()) {
            ui.put(
                Rect::from_center_size(photo.center(), Vec2::splat(28.0)),
                egui::Spinner::new(),
            );
        }
        if self.source.is_some() {
            let caption = format!(
                "{} · preview of what is sent, colors on film will differ",
                self.source_name
            );
            let (font, weak) = (FontId::proportional(12.0), ui.visuals().weak_text_color());
            let (p, at) = (ui.painter(), pos2(frame.center().x, frame.bottom() + 12.0));
            p.text(at, Align2::CENTER_TOP, caption, font.clone(), weak);
            if crop.is_some() {
                let hint = "Drag to move, scroll to zoom";
                p.text(at + vec2(0.0, 18.0), Align2::CENTER_TOP, hint, font, weak);
            }
        } else if response.clicked() {
            self.pick_file();
        }
    }

    fn history_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("⬅ Back").clicked() {
                self.show_history = false;
            }
            ui.heading("History");
        });
        ui.label("Prints this app sent that the printer reported as finished, saved on this computer only. Click one to reprint it.");
        ui.add_space(8.0);
        if self.history.is_empty() {
            ui.label("No saved prints yet.");
        }
        let mut chosen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(16.0, 16.0);
                for (path, texture, aspect) in &self.history {
                    let (cell, response) =
                        ui.allocate_exact_size(vec2(150.0, 190.0), Sense::click());
                    let (frame, photo) = paint_frame(ui.painter(), cell, *aspect);
                    ui.painter()
                        .image(texture.id(), photo, FULL_UV, Color32::WHITE);
                    if response.hovered() {
                        ui.painter().rect_stroke(
                            frame,
                            4.0,
                            Stroke::new(2.0_f32, ACCENT),
                            StrokeKind::Outside,
                        );
                    }
                    if response.on_hover_text("Open for reprint").clicked() {
                        chosen = Some(path.clone());
                    }
                }
            });
        });
        if let Some(path) = chosen {
            self.reopen(&path, ui.ctx());
        }
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if self.busy {
                ui.spinner();
            }
            let color = if self.status_error {
                ui.visuals().error_fg_color
            } else {
                ui.visuals().text_color()
            };
            ui.label(RichText::new(&self.status).color(color));
        });
        let ready = !self.busy && self.device.is_some() && self.source.is_some();
        let why = if self.device.is_none() {
            "Connect a printer first"
        } else {
            "Open a photo first"
        };
        ui.columns(3, |cols| {
            cols[0].with_layout(Layout::right_to_left(Align::Center), |ui| {
                if round_button(ui, 22.0, "🗀", "Open", false, true).clicked() {
                    self.pick_file();
                }
                ui.add_space(12.0);
                if round_button(ui, 22.0, "🕓", "History", false, true).clicked() {
                    self.show_history = !self.show_history;
                    if self.show_history {
                        self.load_history();
                    }
                }
            });
            cols[1].vertical_centered(|ui| {
                let print = round_button(ui, 34.0, "🖶", "Print", true, ready);
                if print.on_disabled_hover_text(why).clicked() {
                    self.confirm_print = true;
                }
            });
            cols[2].with_layout(Layout::left_to_right(Align::Center), |ui| {
                let test = round_button(ui, 22.0, "⬆", "Test upload (no film)", false, ready);
                if test
                    .on_hover_text(
                        "Sends the image, then cancels it before the end. No film is used",
                    )
                    .clicked()
                {
                    self.run(Job::Test);
                }
            });
        });
    }

    fn confirm(&mut self, ctx: &egui::Context) {
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("confirm print")).show(ctx, |ui| {
            ui.set_width(300.0);
            ui.heading("Print 1 photo?");
            ui.label("Uses 1 sheet of film.");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("Cancel").min_size(vec2(80.0, 28.0)))
                    .clicked()
                {
                    choice = Some(false);
                }
                let print = egui::Button::new(RichText::new("Print").color(Color32::WHITE))
                    .fill(ACCENT)
                    .min_size(vec2(80.0, 28.0));
                if ui.add(print).clicked() {
                    choice = Some(true);
                }
            });
        });
        if choice == Some(false) {
            info!("action: print cancelled at confirmation");
        }
        if modal.should_close() || choice.is_some() {
            self.confirm_print = false;
        }
        if choice == Some(true) {
            self.run(Job::Print);
        }
    }
}

/// White instant-film frame (thick bottom border) centered in `area`; returns (frame, photo).
fn paint_frame(p: &Painter, area: Rect, aspect: f32) -> (Rect, Rect) {
    let border = 0.07 * aspect.min(1.0); // in photo-height units
    let units = vec2(aspect + 2.0 * border, 1.0 + 4.5 * border);
    let scale = (area.width() / units.x)
        .min(area.height() / units.y)
        .max(1.0);
    let frame = Rect::from_center_size(area.center(), units * scale);
    p.rect_filled(
        frame.translate(vec2(2.0, 4.0)),
        4.0,
        Color32::from_black_alpha(50),
    );
    p.rect_filled(frame, 4.0, Color32::WHITE);
    p.rect_stroke(
        frame,
        4.0,
        Stroke::new(1.0_f32, Color32::from_gray(205)),
        StrokeKind::Inside,
    );
    let photo = Rect::from_min_size(
        frame.min + Vec2::splat(border * scale),
        vec2(aspect, 1.0) * scale,
    );
    (frame, photo)
}

fn bluetooth(p: &Painter, c: Pos2, color: Color32) {
    let (w, h) = (4.0, 8.0);
    let points = [
        (-w, -h / 2.0),
        (w, h / 2.0),
        (0.0, h),
        (0.0, -h),
        (w, -h / 2.0),
        (-w, h / 2.0),
    ];
    p.add(Shape::line(
        points.map(|(x, y)| c + vec2(x, y)).to_vec(),
        Stroke::new(1.6_f32, color),
    ));
}

/// Round icon button with a caption underneath.
fn round_button(
    ui: &mut egui::Ui,
    radius: f32,
    glyph: &str,
    caption: &str,
    primary: bool,
    enabled: bool,
) -> egui::Response {
    let caption_height = if caption.is_empty() { 0.0 } else { 18.0 };
    let width = (radius * 2.0).max(if caption.is_empty() { 0.0 } else { 72.0 });
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, radius * 2.0 + caption_height), sense);
    let label = if caption.is_empty() { glyph } else { caption };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let v = ui.visuals();
    let hovered = enabled && response.hovered();
    let (fill, fg) = match (primary, enabled) {
        (true, true) => (if hovered { ACCENT_HOVER } else { ACCENT }, Color32::WHITE),
        (false, true) => (
            if hovered {
                v.widgets.hovered.weak_bg_fill
            } else {
                v.widgets.inactive.weak_bg_fill
            },
            v.text_color(),
        ),
        (_, false) => (
            v.widgets.inactive.weak_bg_fill.gamma_multiply(0.4),
            v.weak_text_color(),
        ),
    };
    let center = pos2(rect.center().x, rect.top() + radius);
    let p = ui.painter();
    p.circle_filled(center, radius, fill);
    p.text(
        center,
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(radius * 0.9),
        fg,
    );
    if !caption.is_empty() {
        let color = if enabled {
            v.text_color()
        } else {
            v.weak_text_color()
        };
        p.text(
            pos2(rect.center().x, rect.bottom()),
            Align2::CENTER_BOTTOM,
            caption,
            FontId::proportional(12.0),
            color,
        );
    }
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        while let Ok(event) = self.rx.try_recv() {
            self.handle(ctx, event);
        }
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone())) {
            self.open(path);
        }
        self.refresh_preview(ctx);
        egui::TopBottomPanel::top("top")
            .exact_height(58.0)
            .show(ctx, |ui| self.top_bar(ui));
        egui::TopBottomPanel::bottom("bottom")
            .exact_height(128.0)
            .show(ctx, |ui| self.bottom_bar(ui));
        if !self.show_history {
            egui::SidePanel::right("edit")
                .exact_width(290.0)
                .resizable(false)
                .show(ctx, |ui| self.edit_panel(ui));
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_history {
                self.history_view(ui);
            } else {
                self.home(ui);
            }
        });
        if self.show_printers {
            self.printers(ctx);
        }
        if self.show_settings {
            self.settings(ctx);
        }
        if self.confirm_print {
            self.confirm(ctx);
        }
    }
}

/// Protocol trace per job in `logs/traces/<UTC time>-<job>.jsonl`; keeps the newest few.
fn start_trace<T: outstax_client::Transport>(session: &mut Session<T>, job: &str) {
    let Some(dir) = logs_dir().map(|d| d.join("traces")) else {
        return;
    };
    // Windows file names cannot contain ':'.
    let stamp = logging::utc_timestamp(SystemTime::now()).replace(':', "");
    let path = dir.join(format!("{stamp}-{job}.jsonl"));
    let started = std::fs::create_dir_all(&dir)
        .map_err(anyhow::Error::from)
        .and_then(|()| session.trace_to(&path));
    match started {
        Ok(()) => info!("protocol trace: {}", path.display()),
        Err(e) => warn!("could not start protocol trace {}: {e:#}", path.display()),
    }
    if let Err(e) = logging::prune(&dir, "jsonl", KEEP_TRACES) {
        warn!("could not prune old traces in {}: {e}", dir.display());
    }
}

fn init_logging() {
    let level = logging::level(log::LevelFilter::Info);
    let file = logs_dir().map(|d| d.join("outstax-desktop.log"));
    // No console in release builds: without a writable file, logs go nowhere visible.
    let target = match logging::init(level, file.as_deref()) {
        Ok(()) => file.map_or("stderr".into(), |f| f.display().to_string()),
        Err(e) => {
            let _ = logging::init(level, None);
            format!("stderr (log file failed: {e})")
        }
    };
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        error!("{panic}");
        // capture() honours RUST_BACKTRACE.
        let backtrace = std::backtrace::Backtrace::capture();
        if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
            error!("backtrace:\n{backtrace}");
        }
        default_hook(panic);
    }));
    info!(
        "outstax-desktop {} starting on {} {}, level {level}, logging to {target}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}

fn main() -> eframe::Result<()> {
    let migrated = migrate_data_dir();
    init_logging();
    match migrated {
        Some((old, new, Ok(()))) => {
            info!("moved data folder {} to {}", old.display(), new.display())
        }
        Some((old, new, Err(e))) => warn!(
            "could not move old data folder {} to {}: {e}. Starting with an empty folder",
            old.display(),
            new.display()
        ),
        None => {}
    }
    let result = eframe::run_native(
        "OutStax",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1100.0, 760.0])
                .with_min_inner_size([820.0, 600.0])
                .with_drag_and_drop(true),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx)))),
    );
    match &result {
        Ok(()) => info!("outstax-desktop exiting"),
        Err(e) => error!("window failed: {e}"),
    }
    result
}
