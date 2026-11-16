use crate::Transport;
use anyhow::{bail, Context, Result};
use outstax_protocol::{
    self as wire, settings, AdditionalInfo, Battery, Command, Decoder, Direction, Frame,
    ImageSupport, Info, LinkModel, PrintHistory, PrinterStatus,
};
use std::{
    collections::VecDeque,
    fs::File,
    io::Write,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{timeout_at, Instant};

#[derive(Debug)]
pub struct PrinterInfo {
    pub image: ImageSupport,
    pub battery: Battery,
    pub status: PrinterStatus,
}
#[derive(Debug)]
pub struct PrinterIdentity {
    pub versions: wire::FunctionVersions,
    /// Manufacturer, model, serial, hardware, firmware and software, in order.
    pub fields: Vec<String>,
    pub model: wire::LinkModel,
    pub image: ImageSupport,
    pub program: u16,
    pub bluetooth_versions: Vec<String>,
    pub bt_program: Option<u16>,
    /// SP-4 only (`0010` selector 1).
    pub head: Option<wire::HeadType>,
}
/// Replies collected by [`Session::initialize`]; `None`/empty where the model skips a step.
#[derive(Debug, Default)]
pub struct Initialization {
    pub status: Option<PrinterStatus>,
    pub history: Option<PrintHistory>,
    pub battery: Option<Battery>,
    /// `3010/0` then `3010/1`.
    pub additional: Vec<AdditionalInfo>,
    pub sleep_timers: Option<[u16; 4]>,
    pub function_button: Option<settings::ButtonAction>,
}
impl Initialization {
    /// `3010/1` CPU byte (Mini `W1.u`: 0 ST_MICRO, 1 RENESAS); firmware selection needs it.
    pub fn cpu_type(&self) -> Option<u8> {
        self.additional.iter().find_map(|a| match a {
            AdditionalInfo::Color { cpu_type, .. } => *cpu_type,
            _ => None,
        })
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum TransferOutcome {
    Uploaded,
    PrintAccepted { estimated_seconds: u8 },
}

pub struct Session<T: Transport> {
    transport: T,
    decoder: Decoder,
    inbox: VecDeque<Frame>,
    timeout: Duration,
    trace: Option<File>,
    poisoned: bool,
    /// Model and firmware digits from the last `identity`, for the head-calibration gate.
    identified: Option<(LinkModel, Option<u32>)>,
    /// `3010/1` CPU byte from the last successful `initialize` (0 STMicro, 1 Renesas).
    cpu_type: Option<u8>,
}
impl<T: Transport> Session<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            decoder: Decoder::default(),
            inbox: VecDeque::new(),
            timeout: Duration::from_secs(15),
            trace: None,
            poisoned: false,
            identified: None,
            cpu_type: None,
        }
    }
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout = duration;
    }
    pub fn trace_to(&mut self, path: &Path) -> Result<()> {
        self.trace = Some(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        );
        self.note("open", "trace started on a connected transport")
    }
    fn log(&mut self, direction: &str, bytes: &[u8]) -> Result<()> {
        self.write_row(serde_json::json!({"direction":direction,"hex":wire::hex(bytes)}))
    }
    /// Non-frame rows (open, error) let a trace show when the link failed.
    fn note(&mut self, direction: &str, message: &str) -> Result<()> {
        self.write_row(serde_json::json!({"direction":direction,"message":message}))
    }
    fn write_row(&mut self, mut row: serde_json::Value) -> Result<()> {
        if let Some(file) = &mut self.trace {
            row["time_unix_ms"] =
                serde_json::json!(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());
            writeln!(file, "{row}")?;
            file.flush()?;
        }
        Ok(())
    }
    /// A valid rejection consumes the reply without corrupting stream alignment.
    pub async fn exchange(&mut self, request: Frame) -> Result<Frame> {
        let frame = self.exchange_raw(request).await?;
        if let Err(e) = frame.success_body() {
            log::warn!("{e}");
            return Err(e.into());
        }
        Ok(frame)
    }
    /// No automatic retries. Timeout/corruption poisons the session; negative
    /// device results remain available to state machines such as print polling.
    /// Never resend a DATA block: WIDE 2 checks each sequence against the expected
    /// one (WIDE 2 firmware), so a resend would desynchronise the transfer.
    pub async fn exchange_raw(&mut self, request: Frame) -> Result<Frame> {
        if self.poisoned {
            bail!("session state is uncertain after a transport failure. Disconnect and reconnect");
        }
        if request.direction != Direction::Request {
            bail!("only request frames may be sent");
        }
        let command = request.command;
        let selector = if command == Command::INFO || command == Command::DEVICE_INFO {
            request.payload.first().copied()
        } else {
            None
        };
        let bytes = request.encode()?;
        self.log("tx", &bytes)?;
        let deadline = Instant::now() + self.timeout;
        let result = async {
            timeout_at(deadline, self.transport.send(&bytes))
                .await
                .context("send timed out")??;
            loop {
                while let Some(frame) = self.inbox.pop_front() {
                    if frame.direction != Direction::Response {
                        bail!("received request on client connection");
                    }
                    if frame.command != command {
                        continue;
                    }
                    let result = *frame.payload.first().context("missing device result")?;
                    if let Some(selector) = selector.filter(|_| result == 0) {
                        if frame.payload.get(1) != Some(&selector) {
                            continue;
                        }
                    }
                    let result = wire::DeviceResult(result);
                    log::debug!(
                        "{command}: sent {} bytes, reply {result}, {} payload bytes",
                        bytes.len(),
                        frame.payload.len()
                    );
                    if matches!(result.0, 0x90 | 0x91) {
                        // Recovered for the STM32 firmware images only. Unknown for
                        // SQUARE/WIDE 2 (Sunplus) and the Renesas builds.
                        let stm32 = self.cpu_type != Some(1)
                            && matches!(
                                self.identified,
                                Some((LinkModel::Mini | LinkModel::Mini2 | LinkModel::Mini3 | LinkModel::MiniPlus | LinkModel::Wide, _))
                            );
                        let effect = if stm32 { "its firmware stops answering" } else { "the printer may stop answering" };
                        log::warn!("{command}: printer replied {result}; {effect} after this until it is switched off and on (charge or reseat the battery)");
                    }
                    return Ok(frame);
                }
                let bytes = timeout_at(deadline, self.transport.receive())
                    .await
                    .context("response timed out")??;
                if bytes.is_empty() {
                    bail!("empty transport read");
                }
                self.log("rx", &bytes)?;
                self.inbox.extend(self.decoder.push(&bytes)?);
            }
        }
        .await;
        if let Err(e) = &result {
            log::error!(
                "{command} failed after sending {} bytes: {e:#}; session must reconnect",
                bytes.len()
            );
            self.poisoned = true;
            let _ = self.note("error", &format!("{command}: {e:#}"));
        }
        result
    }
    pub async fn probe(&mut self) -> Result<PrinterInfo> {
        let image = ImageSupport::parse(&self.exchange(wire::query(Info::Image)).await?)?;
        let battery = Battery::parse(&self.exchange(wire::query(Info::Battery)).await?)?;
        let status = PrinterStatus::parse(&self.exchange(wire::query(Info::Printer)).await?)?;
        log::info!(
            "printer: {}x{} up to {} bytes, battery {}%{}, film {}, cover open {}, error {}",
            image.width,
            image.height,
            image.max_bytes,
            battery.percent,
            if status.charging { " charging" } else { "" },
            status.film_remaining,
            status.cover_open,
            status.error
        );
        Ok(PrinterInfo {
            image,
            battery,
            status,
        })
    }
    pub async fn device_info(&mut self, selector: u8) -> Result<String> {
        let frame = self
            .exchange(Frame::request(Command::DEVICE_INFO, vec![selector]))
            .await?;
        let p = frame.success_body()?;
        if p.len() < 2 || p[0] != selector || p.len() < 2 + p[1] as usize {
            bail!("malformed device information");
        }
        Ok(String::from_utf8_lossy(&p[2..2 + p[1] as usize])
            .trim_end_matches('\0')
            .to_owned())
    }
    pub async fn upload(
        &mut self,
        jpeg: &[u8],
        info: &PrinterInfo,
        commit: bool,
        options: [u8; 4],
    ) -> Result<TransferOutcome> {
        let height = info.image.height as u32 * if options[2] & 1 != 0 { 2 } else { 1 };
        crate::imaging::validate_jpeg(
            jpeg,
            info.image.width as u32,
            height,
            info.image.max_bytes as usize,
        )?;
        if info.image.picture_types & 2 == 0 {
            bail!("printer does not advertise JPEG support");
        }
        if options[1] & 8 != 0 && info.image.options & 8 == 0 {
            bail!("printer does not advertise 3D LUT support");
        }
        if options[2] & 1 != 0 && info.image.options2.unwrap_or(0) & 1 == 0 {
            bail!("printer does not advertise double-density/Design Print support");
        }
        // Both the STM32 and the Renesas Link 3 reject any nonzero option2 (0x81),
        // whatever the capability says.
        if options[2] != 0 && matches!(self.identified, Some((LinkModel::Mini3, _))) {
            bail!("mini Link 3 does not support Design Print");
        }
        // Original mini Link firmware 0104 rejects other JPEG layouts.
        if matches!(self.identified, Some((LinkModel::Mini, _))) {
            outstax_protocol::jpeg::check_mini1(jpeg)?;
        }
        // SendPrintImageTask/T1.x send this first, awaiting the reply, so the link
        // parameters settle during the status queries rather than during DATA.
        // The SQUARE app never sends 0103 and its firmware's handling is unknown,
        // so don't risk aborting the upload on it.
        let square = matches!(self.identified, Some((LinkModel::Square, _)));
        if square && self.transport.is_ble() {
            log::info!("skipping 0103 connection parameters on SQUARE, as its app does");
        }
        if self.transport.is_ble() && !square {
            self.exchange(Frame::request(
                Command::BLE_CONNECT,
                vec![0, 12, 0, 12, 0, 0, 1, 144],
            ))
            .await?;
        }
        // Refresh immediately before transfer: the UI's earlier status may be stale.
        let status = PrinterStatus::parse(&self.exchange(wire::query(Info::Printer)).await?)?;
        let ready = status.readiness(!commit);
        if ready.0 != 0 {
            bail!("printer is not ready: {ready}");
        }
        // Mini T1.x: Classic (not BLE), SP-4 and firmware digits >= 103 only.
        if let Some((LinkModel::Mini, digits)) =
            self.identified.filter(|_| !self.transport.is_ble())
        {
            // The app's Integer.parseInt throws here, so it never reaches START either.
            let digits = digits.context("SP-4 firmware revision has no digits")?;
            if digits >= 103 {
                self.calibrate_head().await?;
            }
        }
        log::info!(
            "upload start: {} JPEG bytes, options {options:02x?}, print {commit}",
            jpeg.len()
        );
        let started = Instant::now();
        let start = self
            .exchange(wire::download_start(jpeg.len(), options)?)
            .await?;
        let size = wire::transfer_chunk_size(&start)?;
        log::info!("upload chunk size {size} bytes");
        // Original mini Link 0104: only the START reply loads the transfer timer (4925
        // ticks, ~49 s at 10 ms), so DATA+END must finish within that budget or the
        // printer discards the upload.
        // ponytail: the shorter of the two recovered budgets (49.25/59.10 s); channel selection unknown.
        let budget = matches!(self.identified, Some((LinkModel::Mini, _)))
            .then(|| (Instant::now(), Duration::from_millis(49_250)));
        let chunks = wire::download_chunks(jpeg, size)?;
        for (index, packet) in chunks.into_iter().enumerate() {
            self.check_upload_budget(budget).await?;
            let ack = self.exchange(packet).await?;
            let p = ack.success_body()?;
            if p.len() < 4 || u32::from_be_bytes(p[..4].try_into().unwrap()) != index as u32 {
                self.poisoned = true;
                bail!("wrong DATA acknowledgement for chunk {index}");
            }
        }
        self.check_upload_budget(budget).await?;
        // Upload-only: cancel while the transfer is still open. The firmware accepts
        // CANCEL only after DATA and before END (mini Link 0104 `upload_cancel`
        // requires state 2; END clears it, so CANCEL after END is 0x82), and the apps
        // cancel only between START and END. Nothing stays staged on the printer.
        let last = if commit {
            Command::END
        } else {
            Command::CANCEL
        };
        self.exchange(Frame::request(last, vec![])).await?;
        let seconds = started.elapsed().as_secs_f64();
        log::info!(
            "upload {}: {} bytes in {seconds:.1} s ({:.1} KB/s)",
            if commit {
                "end"
            } else {
                "accepted, then cancelled before END (test only)"
            },
            jpeg.len(),
            jpeg.len() as f64 / 1000.0 / seconds.max(0.001)
        );
        if !commit {
            return Ok(TransferOutcome::Uploaded);
        }
        let status = PrinterStatus::parse(&self.exchange(wire::query(Info::Printer)).await?)?;
        let ready = status.readiness(false);
        if ready.0 != 0 {
            bail!("printer failed post-upload preflight: {ready}");
        }
        // An error after this point means the physical result is unknown. Never replay.
        let reply = self
            .exchange(Frame::request(Command::PRINT, vec![]))
            .await
            .context("PRINT result is uncertain. Check the printer before starting another job")?;
        let p = reply.success_body()?;
        let estimated_seconds = *p.first().ok_or_else(|| {
            anyhow::anyhow!(
                "PRINT was accepted but the completion estimate is missing. Check the printer"
            )
        })?;
        log::info!("print started, printer estimates {estimated_seconds} s");
        Ok(TransferOutcome::PrintAccepted { estimated_seconds })
    }
    /// Stops an upload whose printer-side transfer window has already expired:
    /// cancel the staged job (best effort) instead of sending blocks it will reject.
    async fn check_upload_budget(&mut self, budget: Option<(Instant, Duration)>) -> Result<()> {
        let Some((since, limit)) = budget else {
            return Ok(());
        };
        if since.elapsed() <= limit {
            return Ok(());
        }
        log::error!(
            "upload exceeded the printer's {:.1} s transfer window after {:.1} s",
            limit.as_secs_f64(),
            since.elapsed().as_secs_f64()
        );
        let _ = self.cancel_upload().await;
        bail!(
            "upload took longer than the printer's ~{} s transfer window, so the printer discarded it. Nothing was printed. Move closer to the printer and try again.",
            limit.as_secs()
        )
    }
    /// `3080`, then `3081` only for a FUTABA head with judge 1. Any non-OK aborts.
    async fn calibrate_head(&mut self) -> Result<()> {
        let reply = self
            .exchange(Frame::request(Command(0x30, 0x80), vec![]))
            .await?;
        let calibration = settings::HeadCalibration::parse(&reply)?;
        let (year, month) = utc_year_month(SystemTime::now().duration_since(UNIX_EPOCH)?);
        if let Some(rgb) = calibration.correction(year, month)? {
            self.exchange(settings::set_head_correction(rgb)?).await?;
        }
        Ok(())
    }
    /// Follow the official PrintTask: one-second status polls, tolerating C2
    /// responses without trying to parse their absent success body. This never
    /// reissues PRINT, even after an uncertain transport outcome.
    pub async fn wait_for_print(&mut self, maximum: Duration) -> Result<PrinterStatus> {
        let started = Instant::now();
        let deadline = started + maximum;
        loop {
            if Instant::now() >= deadline {
                bail!("print completion deadline reached. Do not replay PRINT automatically");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            let frame = self.exchange_raw(wire::query(Info::Printer)).await?;
            if frame.payload.first() == Some(&0xc2) {
                continue;
            }
            let status = PrinterStatus::parse(&frame)?;
            let result = status.readiness(true);
            if result.0 != 0 && result.0 != 1 {
                bail!("print completion failed: {result}");
            }
            if !status.busy {
                log::info!(
                    "print completed after {:.0} s, film left {}",
                    started.elapsed().as_secs_f64(),
                    status.film_remaining
                );
                return Ok(status);
            }
        }
    }
    /// Initial app handshake. Numeric selectors 9/10 and BT program info are
    /// queried only for mini Link 3, matching the Mini application's state flow.
    pub async fn identity(&mut self) -> Result<PrinterIdentity> {
        let versions = wire::FunctionVersions::parse(
            &self
                .exchange(Frame::request(Command::VERSION, vec![]))
                .await?,
        )?;
        versions.validate_print()?;
        let mut fields = Vec::new();
        for selector in 0..=5 {
            fields.push(self.device_info(selector).await?);
        }
        if !fields[0].eq_ignore_ascii_case("FUJIFILM") {
            bail!("unexpected manufacturer: {}", fields[0]);
        }
        let model = wire::LinkModel::from_model_number(&fields[1])
            .context("unrecognized standalone Link model")?;
        let mut bluetooth_versions = Vec::new();
        if model == wire::LinkModel::Mini3 {
            for selector in [9, 10] {
                bluetooth_versions.push(self.device_info(selector).await?);
            }
        }
        let image = ImageSupport::parse(&self.exchange(wire::query(Info::Image)).await?)?;
        let program = self
            .exchange(Frame::request(Command(0x20, 0x10), vec![]))
            .await?;
        let p = program.success_body()?;
        if p.len() < 2 {
            bail!("missing firmware program number");
        }
        let program = u16::from_be_bytes([p[0], p[1]]);
        let bt_program = if model == wire::LinkModel::Mini3 {
            let frame = self
                .exchange(Frame::request(Command(0x20, 0xa8), vec![]))
                .await?;
            let p = frame.success_body()?;
            if p.len() < 2 {
                bail!("missing Bluetooth firmware program number");
            }
            Some(u16::from_be_bytes([p[0], p[1]]))
        } else {
            None
        };
        // Mini T1.n asks SP-4 for its light-pen string; a non-OK reply fails identity.
        let head = if model == LinkModel::Mini {
            let (selector, text) = wire::parse_identify(&self.exchange(wire::identify(1)).await?)?;
            if selector != 1 {
                bail!("unexpected identification selector {selector}");
            }
            Some(wire::HeadType::from_light_pen(&text))
        } else {
            None
        };
        self.identified = Some((model, wire::firmware_digits(&fields[4])));
        log::info!(
            "identified {model:?} ({}), firmware {}",
            fields[1],
            fields[4]
        );
        Ok(PrinterIdentity {
            versions,
            fields,
            model,
            image,
            program,
            bluetooth_versions,
            bt_program,
            head,
        })
    }
    /// One pass of the app's post-identity connect steps, without its polling
    /// loop. Unlike the app, a failed step is returned rather than skipped.
    pub async fn initialize(&mut self, identity: &PrinterIdentity) -> Result<Initialization> {
        let result = self.initialize_steps(identity).await;
        match &result {
            Ok(init) => {
                self.cpu_type = init.cpu_type();
                if let Some(cpu) = self.cpu_type {
                    log::info!("printer CPU: {}", cpu_name(cpu));
                }
            }
            Err(e) => log::warn!("connect-time setup step failed (callers may continue): {e:#}"),
        }
        result
    }
    async fn initialize_steps(&mut self, identity: &PrinterIdentity) -> Result<Initialization> {
        use LinkModel::*;
        let model = identity.model;
        let mini = matches!(model, Mini | Mini2 | Mini3 | MiniPlus);
        let mut init = Initialization::default();
        if mini {
            // X3.G GET_PRINTER_INFO1: 0002/2, 0002/3, 0002/1.
            let status = self.exchange(wire::query(Info::Printer)).await?;
            init.status = Some(PrinterStatus::parse(&status)?);
            let history = self.exchange(wire::query(Info::PrintHistory)).await?;
            init.history = Some(PrintHistory::parse(&history)?);
            let battery = self.exchange(wire::query(Info::Battery)).await?;
            init.battery = Some(Battery::parse(&battery)?);
            if model == Mini3 {
                let light = settings::ShootingLight::ThreeWhiteOn;
                self.exchange(settings::shooting_light(model, light)?)
                    .await?;
            }
        }
        if model == Square {
            let set = settings::function_button_set(settings::ButtonAction::Reprint);
            init.function_button =
                Some(settings::parse_function_button(&self.exchange(set).await?)?);
        }
        if model != Mini {
            for selector in [0, 1] {
                let reply = self
                    .exchange(Frame::request(Command::ADDITIONAL_INFO, vec![selector]))
                    .await?;
                init.additional.push(AdditionalInfo::parse(&reply)?);
            }
        }
        if mini {
            // The app preference "accelerationLinkage" defaults to true (X3.b0).
            self.exchange(settings::axis_actions(true, true)).await?;
        } else {
            let read = settings::sleep_settings(settings::SleepMode::Current, [0; 4]);
            init.sleep_timers = Some(settings::parse_sleep(&self.exchange(read).await?)?);
        }
        Ok(init)
    }
    /// App "auto-sleep on": read the default timers (mode 2), write them back with mode 5.
    pub async fn sleep_enable(&mut self) -> Result<[u16; 4]> {
        let reply = self.exchange(settings::sleep_enable_read()).await?;
        let timers = settings::parse_sleep(&reply)?;
        self.exchange(settings::sleep_enable(timers)).await?;
        Ok(timers)
    }
    pub async fn cancel_upload(&mut self) -> Result<()> {
        self.exchange(Frame::request(Command::CANCEL, vec![]))
            .await?;
        Ok(())
    }
    pub async fn disconnect(&mut self) -> Result<()> {
        self.transport.disconnect().await
    }
}

/// ponytail: UTC calendar month; the app uses the phone's local date, which can
/// differ for a few hours at a month boundary.
fn utc_year_month(since_epoch: Duration) -> (u16, u8) {
    let (year, month, _) = crate::logging::civil_from_days((since_epoch.as_secs() / 86_400) as i64);
    (year as u16, month)
}
#[cfg(test)]
#[test]
fn civil_month() {
    assert_eq!(utc_year_month(Duration::ZERO), (1970, 1));
    // 2026-11-12T00:00:00Z
    assert_eq!(
        utc_year_month(Duration::from_secs(1_794_441_600)),
        (2026, 11)
    );
    // 2024-02-29T23:59:59Z
    assert_eq!(
        utc_year_month(Duration::from_secs(1_709_251_199)),
        (2024, 2)
    );
    // 2024-03-01T00:00:00Z
    assert_eq!(
        utc_year_month(Duration::from_secs(1_709_251_200)),
        (2024, 3)
    );
}

/// `3010/1` CPU byte as a name (Mini `W1.u`: 0 ST_MICRO, 1 RENESAS).
pub fn cpu_name(cpu: u8) -> &'static str {
    match cpu {
        0 => "STMicro",
        1 => "Renesas",
        _ => "unknown",
    }
}
