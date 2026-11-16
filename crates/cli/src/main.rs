use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use outstax_client::{
    imaging::{self, Fit},
    session::Session,
    transport::{TcpTransport, Transport},
};
use outstax_protocol::{self as wire, Command, Decoder, Frame};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Parser)]
#[command(
    version,
    about = "OutStax: independent client and protocol analysis tools for INSTAX Link printers"
)]
struct Args {
    #[arg(
        long,
        global = true,
        help = "Exact BLE printer ID, address, or advertised name from scan"
    )]
    device: Option<String>,
    #[arg(
        long,
        global = true,
        conflicts_with = "device",
        help = "Raw Link protocol diagnostic bridge host:port"
    )]
    tcp: Option<String>,
    #[arg(long, global = true, default_value_t = 5)]
    scan_seconds: u64,
    #[arg(long, global = true, default_value_t = 15)]
    timeout_seconds: u64,
    #[arg(
        long,
        global = true,
        default_value_t = 0,
        help = "BLE write fragment size in bytes. 0 means negotiated ATT MTU - 3, as the apps do"
    )]
    write_size: usize,
    #[arg(
        long,
        global = true,
        default_value_t = 0,
        help = "Extra pause after each BLE write. The apps only wait for each write to complete"
    )]
    write_delay_ms: u64,
    #[arg(
        long,
        global = true,
        help = "Write a NEW JSONL packet trace (it contains the transferred image bytes)"
    )]
    trace: Option<PathBuf>,
    #[arg(
        short,
        long,
        global = true,
        action = clap::ArgAction::Count,
        help = "Log to stderr: -v info, -vv debug, -vvv trace (or set OUTSTAX_LOG)"
    )]
    verbose: u8,
    #[command(subcommand)]
    command: Action,
}
#[derive(Clone, Copy, ValueEnum)]
enum FitArg {
    Crop,
    Contain,
}
impl From<FitArg> for Fit {
    fn from(value: FitArg) -> Self {
        match value {
            FitArg::Crop => Fit::Crop,
            FitArg::Contain => Fit::Contain,
        }
    }
}
#[derive(Subcommand)]
enum Action {
    /// Discover printers without connecting or printing.
    Scan,
    /// Query image capabilities, battery, and film/status.
    Status,
    /// Query device identity information (selector 0..10; 9/10 are Link 3).
    Identity {
        #[arg(default_value_t = 1)]
        selector: u8,
    },
    /// Printer settings and telemetry from recovered app paths.
    Control {
        #[command(subcommand)]
        command: Control,
    },
    /// Read function versions and perform the full app identity handshake.
    Handshake,
    /// Inspect a DAT/HEX firmware image offline; sends no printer commands.
    FirmwareInspect { file: PathBuf },
    /// Prepare, transfer, and print. Without --commit, uploads only; consumes no film.
    Print {
        image: PathBuf,
        #[arg(long)]
        commit: bool,
        #[arg(long,value_enum,default_value_t=FitArg::Crop)]
        fit: FitArg,
        #[arg(
            long,
            help = "Send an already prepared JPEG without resizing/re-encoding"
        )]
        prepared: bool,
        #[arg(long, help = "Conservative cap, limited by printer-advertised maximum")]
        max_bytes: Option<usize>,
        #[arg(
            long,
            help = "Apply a local 64 MiB RGB lookup table and set the LUT-applied flag",
            conflicts_with = "prepared"
        )]
        lut: Option<PathBuf>,
        #[arg(
            long,
            help = "Set LUT-applied flag for a JPEG that already has Rich processing",
            requires = "prepared"
        )]
        lut_applied: bool,
        #[arg(
            long,
            help = "Use double-density mode only if advertised by the printer"
        )]
        design: bool,
    },
    /// Prepare a JPEG offline for a film format or explicit dimensions.
    Prepare {
        image: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "mini")]
        format: String,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, default_value_t = 107520)]
        max_bytes: usize,
        #[arg(long,value_enum,default_value_t=FitArg::Crop)]
        fit: FitArg,
        #[arg(long)]
        design: bool,
        /// Apply the recovered optional Link+ linear-light unsharp algorithm.
        #[arg(long)]
        unsharp: bool,
    },
    /// Decode a complete hex frame with checksum/length validation.
    Decode { hex: String },
    /// Decode JSONL tx/rx traces, reassembling fragments independently by direction.
    Trace { file: PathBuf },
    /// Export all protocol enums recovered from app analysis.
    Commands,
    /// Run a deterministic local printer simulator for end-to-end testing.
    Simulate {
        #[arg(long, default_value = "127.0.0.1:7878")]
        listen: String,
        #[arg(long, default_value = "mini")]
        format: String,
        #[arg(long)]
        save: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum Control {
    /// Read axes and switch/button state.
    Axes,
    /// Read lifetime print and cover-ejection counters.
    History,
    /// Read voltage/temperature (0), or print count/color/CPU (1).
    Additional {
        #[arg(value_parser=clap::value_parser!(u8).range(0..=1))]
        selector: u8,
    },
    /// Read the four current auto-sleep timers.
    Sleep,
    /// Read power-on/off LED pattern, or set it when --value is supplied.
    PowerLed {
        #[arg(long)]
        value: Option<u8>,
    },
    /// Set axis-triggered power-off/reprint permissions.
    AxisActions {
        #[arg(long)]
        power_off: bool,
        #[arg(long)]
        reprint: bool,
    },
    /// Set Link 2/3 AR vibration and read the returned switch state.
    Vibration {
        #[arg(long)]
        enabled: bool,
        #[arg(long, default_value_t = 0)]
        intensity: u8,
    },
    /// Write one mini Link 3 parameter (0 mode switch, 3 shooting light); 1, 2, 4 are never sent by the app.
    Parameter {
        #[arg(value_parser=clap::value_parser!(u8).range(0..=4))]
        parameter: u8,
        value: u8,
    },
}
async fn control(session: &mut Session<Box<dyn Transport>>, command: &Control) -> Result<()> {
    use wire::{settings, Info};
    let request = match command {
        Control::Axes => Frame::request(Command::AXES, vec![]),
        Control::History => wire::query(Info::PrintHistory),
        Control::Additional { selector } => {
            Frame::request(Command::ADDITIONAL_INFO, vec![*selector])
        }
        Control::Sleep => settings::sleep_settings(settings::SleepMode::Current, [0; 4]),
        Control::PowerLed { value } => {
            let identity = session.identity().await?;
            settings::power_led(identity.model, value.is_some(), value.unwrap_or(0))?
        }
        Control::AxisActions { power_off, reprint } => settings::axis_actions(*power_off, *reprint),
        Control::Vibration { enabled, intensity } => settings::ar_vibration(*enabled, *intensity),
        Control::Parameter { parameter, value } => {
            let kind = match parameter {
                0 => settings::Parameter::ModeSwitch,
                3 => settings::Parameter::ShootingLight,
                _ => bail!("the app never sends parameters 1, 2 or 4, so the printer's response is unknown"),
            };
            let identity = session.identity().await?;
            let mut data = [0; 8];
            data[0] = *value;
            settings::parameter(identity.model, kind, data)?
        }
    };
    let reply = session.exchange(request).await?;
    match command {
        Control::Axes => println!("{:?}", wire::Axes::parse(&reply)?),
        Control::History => println!("{:?}", wire::PrintHistory::parse(&reply)?),
        Control::Additional { .. } => println!("{:?}", wire::AdditionalInfo::parse(&reply)?),
        Control::Sleep => println!("{:?}", settings::parse_sleep(&reply)?),
        Control::PowerLed { value: None } => println!("{}", settings::parse_power_led(&reply)?),
        _ => print_frame(&reply),
    }
    Ok(())
}

fn dimensions(format: &str) -> Result<(u32, u32)> {
    match format {
        "mini" => Ok((600, 800)),
        "square" => Ok((800, 800)),
        "wide" => Ok((1260, 840)),
        _ => bail!("format must be mini, square, or wide"),
    }
}
async fn connect(args: &Args) -> Result<Session<Box<dyn Transport>>> {
    let transport: Box<dyn Transport> = if let Some(address) = &args.tcp {
        Box::new(
            tokio::time::timeout(
                Duration::from_secs(args.timeout_seconds),
                TcpTransport::connect(address),
            )
            .await??,
        )
    } else {
        let selector = args
            .device
            .as_deref()
            .context("choose --device from scan, or --tcp for a diagnostic bridge")?;
        #[cfg(feature = "ble")]
        {
            Box::new(
                tokio::time::timeout(
                    Duration::from_secs(args.scan_seconds + args.timeout_seconds),
                    outstax_client::ble::BleTransport::connect(
                        selector,
                        Duration::from_secs(args.scan_seconds),
                        args.write_size,
                        Duration::from_millis(args.write_delay_ms),
                    ),
                )
                .await??,
            )
        }
        #[cfg(not(feature = "ble"))]
        {
            let _ = selector;
            bail!("this build has no BLE support. Rebuild with the default features");
        }
    };
    let mut session = Session::new(transport);
    session.set_timeout(Duration::from_secs(args.timeout_seconds));
    if let Some(path) = &args.trace {
        session.trace_to(path)?;
    }
    Ok(session)
}
fn print_frame(frame: &Frame) {
    println!(
        "{}",
        serde_json::json!({"direction":format!("{:?}",frame.direction),"command":frame.command.to_string(),"payload_hex":wire::hex(&frame.payload)})
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    use log::LevelFilter::*;
    let level = [Warn, Info, Debug, Trace][usize::from(args.verbose.min(3))];
    outstax_client::logging::init(outstax_client::logging::level(level), None)?;
    match &args.command {
        Action::FirmwareInspect { file } => {
            let bytes = std::fs::read(file)?;
            let image = if file
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("hex"))
            {
                wire::firmware::FirmwareImage::intel_hex(std::str::from_utf8(&bytes)?)?
            } else {
                wire::firmware::FirmwareImage::dat(bytes)?
            };
            let (address, bytes) = image.app_layout();
            println!(
                "{} bytes, start 0x{address:08x}, {} records, contiguous: {}, HEX EOF: {:?}",
                bytes.len(),
                image.segments.len(),
                image.contiguous_layout().is_ok(),
                image.eof_record
            );
        }

        Action::Scan => {
            #[cfg(feature = "ble")]
            for device in
                outstax_client::ble::discover(Duration::from_secs(args.scan_seconds)).await?
            {
                let d = device.info;
                println!(
                    "{}",
                    serde_json::json!({"id":d.id,"name":d.name,"address":d.address,"rssi":d.rssi,"known_service":d.has_print_service})
                );
            }
            #[cfg(not(feature = "ble"))]
            bail!("this build has no BLE support");
        }
        Action::Decode { hex } => print_frame(&Frame::decode(&wire::unhex(hex)?)?),
        Action::Trace { file } => {
            let text = std::fs::read_to_string(file)?;
            let mut tx = Decoder::default();
            let mut rx = Decoder::default();
            for (line, row) in text.lines().enumerate() {
                let result = (|| -> Result<()> {
                    let v: serde_json::Value = serde_json::from_str(row)?;
                    let direction = v["direction"].as_str().context("missing direction")?;
                    let decoder = match direction {
                        "tx" => &mut tx,
                        "rx" => &mut rx,
                        "open" | "error" => {
                            eprintln!("{row}");
                            return Ok(());
                        }
                        _ => bail!("direction must be tx, rx, open or error"),
                    };
                    for frame in
                        decoder.push(&wire::unhex(v["hex"].as_str().context("missing hex")?)?)?
                    {
                        print_frame(&frame);
                    }
                    Ok(())
                })();
                result.with_context(|| format!("trace line {}", line + 1))?;
            }
            tx.finish()?;
            rx.finish()?;
        }
        Action::Prepare {
            image,
            output,
            format,
            width,
            height,
            max_bytes,
            fit,
            design,
            unsharp,
        } => {
            let (w, h) = match (width, height) {
                (None, None) => dimensions(format)?,
                (Some(w), Some(h)) => (*w, *h),
                _ => bail!("provide both --width and --height"),
            };
            let p = imaging::prepare_with_options(
                imaging::load(image)?,
                w,
                h,
                *max_bytes,
                (*fit).into(),
                imaging::Processing {
                    lut: None,
                    design: *design,
                    unsharp: unsharp.then_some(Default::default()),
                    ..Default::default()
                },
            )?;
            write_new(output, &p.jpeg)?;
            println!(
                "{}x{} JPEG, {} bytes, quality {} -> {}",
                p.width,
                p.height,
                p.jpeg.len(),
                p.quality,
                output.display()
            );
        }
        Action::Commands => println!("{}", include_str!("../../../data/protocol/commands.json")),
        Action::Simulate {
            listen,
            format,
            save,
        } => simulator(listen, dimensions(format)?, save.as_deref()).await?,
        Action::Status
        | Action::Handshake
        | Action::Control { .. }
        | Action::Identity { .. }
        | Action::Print { .. } => {
            let mut session = connect(&args).await?;
            let result = async {
                match &args.command {
                    Action::Control { command } => control(&mut session, command).await?,
                    Action::Status => {
                        let identity = session.identity().await?;
                        let init = session.initialize(&identity).await?;
                        println!("{init:#?}");
                        if let Some(cpu) = init.cpu_type() {
                            println!("CPU: {}", outstax_client::session::cpu_name(cpu));
                        }
                        println!("{:#?}", session.probe().await?);
                    }
                    Action::Handshake => println!("{:#?}", session.identity().await?),
                    Action::Identity { selector } => {
                        if *selector > 10 {
                            bail!("identity selector must be 0..10");
                        }
                        println!("{}", session.device_info(*selector).await?);
                    }
                    Action::Print {
                        image,
                        commit,
                        fit,
                        prepared,
                        max_bytes,
                        lut,
                        lut_applied,
                        design,
                    } => {
                        let identity = session.identity().await?;
                        println!(
                            "Printer {:?}, firmware {}",
                            identity.model, identity.fields[4]
                        );
                        // The app skips failed connect-time steps; Session logs a warning.
                        let _ = session.initialize(&identity).await;
                        let info = session.probe().await?;
                        let limit = (*max_bytes)
                            .unwrap_or(info.image.max_bytes as usize)
                            .min(info.image.max_bytes as usize);
                        let table = lut.as_ref().map(std::fs::read).transpose()?;
                        let jpeg = if *prepared {
                            let bytes = std::fs::read(image)?;
                            imaging::validate_jpeg(
                                &bytes,
                                info.image.width as u32,
                                info.image.height as u32 * if *design { 2 } else { 1 },
                                limit,
                            )?;
                            bytes
                        } else {
                            imaging::prepare_job(
                                imaging::load(image)?,
                                info.image.width as u32,
                                info.image.height as u32,
                                limit,
                                (*fit).into(),
                                table.as_deref(),
                                *design,
                            )?
                            .jpeg
                        };
                        println!(
                            "Transferring {} bytes, physical print {}",
                            jpeg.len(),
                            if *commit { "enabled" } else { "disabled" }
                        );
                        let outcome = session
                            .upload(
                                &jpeg,
                                &info,
                                *commit,
                                [
                                    2,
                                    if lut.is_some() || *lut_applied { 8 } else { 0 },
                                    u8::from(*design),
                                    0,
                                ],
                            )
                            .await?;
                        println!("{outcome:?}");
                        if *commit {
                            let status = session.wait_for_print(Duration::from_secs(180)).await?;
                            println!(
                                "Print completed, {} exposures remaining",
                                status.film_remaining
                            );
                        } else {
                            println!("Test upload accepted and cancelled before END. Nothing is staged and no film was used");
                        }
                    }
                    _ => unreachable!(),
                }
                Ok::<(), anyhow::Error>(())
            }
            .await;
            let disconnected = session.disconnect().await;
            result?;
            disconnected?;
        }
    }
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

async fn simulator(address: &str, dimensions: (u32, u32), save: Option<&Path>) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Simulator listening on {}", listener.local_addr()?);
    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut decoder = Decoder::default();
        let mut declared = None;
        let mut image = Vec::new();
        let mut next = 0u32;
        let mut complete = false;
        loop {
            let mut b = [0; 4096];
            let n = stream.read(&mut b).await?;
            if n == 0 {
                break;
            }
            for request in decoder.push(&b[..n])? {
                let mut payload = vec![0];
                match request.command {
                    Command::VERSION => payload.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0, 0]),
                    Command(0x20, 0x10) => payload.extend_from_slice(&[1, 0]),
                    Command::INFO => match request.payload.first() {
                        Some(0) => {
                            payload.push(0);
                            payload.extend_from_slice(&(dimensions.0 as u16).to_be_bytes());
                            payload.extend_from_slice(&(dimensions.1 as u16).to_be_bytes());
                            payload.extend_from_slice(&[2, 8]);
                            payload.extend_from_slice(&409600u32.to_be_bytes());
                        }
                        Some(1) => payload.extend_from_slice(&[1, 2, 80, 0, 0]),
                        Some(2) => payload.extend_from_slice(&[2, 0x3a, 0, 0, 0, 0, 0, 0, 0]),
                        Some(3) => payload.extend_from_slice(&[3, 0, 0, 0, 1, 0, 0, 0, 0]),
                        _ => payload = vec![0x81],
                    },
                    Command::DEVICE_INFO => {
                        let selector = request.payload.first().copied().unwrap_or(1);
                        let s: &[u8] = match selector {
                            0 => b"FUJIFILM",
                            1 => match dimensions {
                                (600, 800) => b"SP-4",
                                (800, 800) => b"FI017",
                                _ => b"BO-22",
                            },
                            // Digits >= 103 so the SP-4 head-calibration gate runs over TCP.
                            4 => b"0103",
                            _ => b"simulator",
                        };
                        payload.extend_from_slice(&[
                            request.payload.first().copied().unwrap_or(1),
                            s.len() as u8,
                        ]);
                        payload.extend_from_slice(s);
                    }
                    // SP-4 light pen: 14 bytes without "AD" = TOHOKU.
                    Command::IDENTIFY => payload.extend_from_slice(b"\x01\x0eSIM01234567890"),
                    Command(0x30, 2) => {}
                    Command(0x30, 8) => payload.push(1),
                    Command::ADDITIONAL_INFO => match request.payload.first() {
                        Some(0) => payload.extend_from_slice(&[0, 0x0f, 0xa0, 0, 25]),
                        Some(1) => payload.extend_from_slice(&[1, 0, 0, 0, 5, 1, 0, 0, 1]),
                        _ => payload = vec![0x81],
                    },
                    Command::SLEEP => payload.extend_from_slice(&[0, 180, 0, 0, 0, 0, 0, 0]),
                    // TOHOKU head, judge 0: the app skips 3081 and goes to START.
                    Command(0x30, 0x80) => payload
                        .extend_from_slice(&[1, 0, 0, 0x07, 0xea, 11, 12, 0, 32, 0, 32, 0, 32]),
                    Command::START => {
                        if request.payload.len() != 8 {
                            payload = vec![0x81];
                        } else {
                            let size = u32::from_be_bytes(request.payload[4..8].try_into().unwrap())
                                as usize;
                            if size == 0 || size > 409600 {
                                payload = vec![0x81];
                            } else {
                                declared = Some(size);
                                image.clear();
                                next = 0;
                                complete = false;
                                payload.extend_from_slice(&900u32.to_be_bytes());
                            }
                        }
                    }
                    Command::DATA => {
                        if declared.is_none()
                            || complete
                            || request.payload.len() != 904
                            || request.payload[..4] != next.to_be_bytes()
                        {
                            payload = vec![0x81];
                        } else {
                            image.extend_from_slice(&request.payload[4..]);
                            payload.extend_from_slice(&next.to_be_bytes());
                            next += 1;
                        }
                    }
                    Command::END => {
                        if declared.is_some_and(|n| image.len() >= n && image.len() < n + 900) {
                            image.truncate(declared.unwrap());
                            complete = true;
                        } else {
                            payload = vec![0x81];
                        }
                    }
                    // mini Link 0104 `upload_cancel` requires an open transfer (state 2:
                    // after DATA, before END); otherwise SEQUENCE_ERROR.
                    Command::CANCEL if declared.is_none() || complete || next == 0 => {
                        payload = vec![0x82];
                    }
                    Command::CANCEL => {
                        declared = None;
                        image.clear();
                        complete = false;
                    }
                    Command::PRINT => {
                        if !complete {
                            payload = vec![0x81];
                        } else {
                            imaging::validate_jpeg(&image, dimensions.0, dimensions.1, 409600)?;
                            if let Some(path) = save {
                                write_new(path, &image)?;
                            }
                            println!("Simulator accepted {} JPEG bytes", image.len());
                            payload.push(1);
                            complete = false;
                        }
                    }
                    _ => payload = vec![0x81],
                }
                let reply = Frame::response(request.command, payload).encode()?;
                // Intentionally fragment replies to exercise stream reassembly.
                for piece in reply.chunks(3) {
                    stream.write_all(piece).await?;
                }
            }
        }
    }
}
