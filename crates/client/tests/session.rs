use anyhow::Result;
use async_trait::async_trait;
use outstax_client::{
    imaging::{self, Fit},
    session::{PrinterInfo, Session, TransferOutcome},
    Transport,
};
use outstax_protocol::{Battery, Command, Frame, ImageSupport, PrinterStatus};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

struct Mock {
    sent: Arc<Mutex<Vec<Frame>>>,
    replies: VecDeque<Vec<u8>>,
    ble: bool,
    /// Simulated slow link: virtual time spent sending each DATA block.
    data_delay: Duration,
}
#[async_trait]
impl Transport for Mock {
    fn is_ble(&self) -> bool {
        self.ble
    }
    async fn send(&mut self, b: &[u8]) -> Result<()> {
        let frame = Frame::decode(b)?;
        if frame.command == Command::DATA && !self.data_delay.is_zero() {
            tokio::time::sleep(self.data_delay).await;
        }
        self.sent.lock().unwrap().push(frame);
        Ok(())
    }
    async fn receive(&mut self) -> Result<Vec<u8>> {
        match self.replies.pop_front() {
            Some(b) => Ok(b),
            None => {
                std::future::pending::<()>().await;
                unreachable!()
            }
        }
    }
    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }
}
fn info() -> PrinterInfo {
    PrinterInfo {
        image: ImageSupport {
            width: 60,
            height: 80,
            picture_types: 2,
            options: 8,
            max_bytes: 50000,
            options2: Some(1),
        },
        battery: Battery {
            state: 2,
            percent: 80,
            charger_type: 0,
            charger_state: 0,
        },
        status: PrinterStatus {
            film_remaining: 10,
            charging: false,
            battery_level: 3,
            cover_open: false,
            busy: false,
            error: false,
            operation: 0,
            print_result: 0,
            wait_seconds: 0,
            error_code: 0,
            raw: vec![],
        },
    }
}
fn jpeg() -> Vec<u8> {
    imaging::prepare(
        image::DynamicImage::new_rgb8(30, 40),
        60,
        80,
        50000,
        Fit::Crop,
    )
    .unwrap()
    .jpeg
}
fn response(cmd: Command, p: Vec<u8>) -> Vec<u8> {
    Frame::response(cmd, p).encode().unwrap()
}
fn mock(replies: Vec<Vec<u8>>, ble: bool) -> (Session<Mock>, Arc<Mutex<Vec<Frame>>>) {
    mock_delayed(replies, ble, Duration::ZERO)
}
fn mock_delayed(
    mut replies: Vec<Vec<u8>>,
    ble: bool,
    data_delay: Duration,
) -> (Session<Mock>, Arc<Mutex<Vec<Frame>>>) {
    // The pre-transfer status reply follows BLE_CONNECT, as in SendPrintImageTask.
    let first = replies.first().map(|r| Frame::decode(r).unwrap().command);
    let at = match first {
        Some(Command::BLE_CONNECT) => Some(1),
        Some(Command::START) => Some(0),
        _ => None,
    };
    if let Some(at) = at {
        replies.insert(
            at,
            response(Command::INFO, vec![0, 2, 0x3a, 0, 0, 0, 0, 0, 0, 0]),
        );
    }
    let sent = Arc::new(Mutex::new(vec![]));
    let session = Session::new(Mock {
        sent: sent.clone(),
        replies: replies.into(),
        ble,
        data_delay,
    });
    (session, sent)
}
#[tokio::test]
async fn upload_only_never_sends_print_and_negotiates_block_size() {
    let image = jpeg();
    let size = 181;
    let count = image.len().div_ceil(size);
    let mut replies = vec![response(
        Command::START,
        [vec![0], (size as u32).to_be_bytes().to_vec()].concat(),
    )];
    for i in 0..count {
        replies.push(response(
            Command::DATA,
            [vec![0], (i as u32).to_be_bytes().to_vec()].concat(),
        ));
    }
    replies.push(response(Command::CANCEL, vec![0]));
    let (mut session, sent) = mock(replies, false);
    assert_eq!(
        session
            .upload(&image, &info(), false, [2, 0, 0, 0])
            .await
            .unwrap(),
        TransferOutcome::Uploaded
    );
    let sent = sent.lock().unwrap();
    assert!(!sent.iter().any(|f| f.command == Command::PRINT));
    // Firmware accepts CANCEL only before END (mini Link 0104: CANCEL after END is 0x82).
    assert!(!sent.iter().any(|f| f.command == Command::END));
    assert_eq!(sent.last().unwrap().command, Command::CANCEL);
    let blocks: Vec<_> = sent.iter().filter(|f| f.command == Command::DATA).collect();
    assert_eq!(blocks.len(), count);
    assert!(blocks.iter().all(|f| f.payload.len() == size + 4));
    let reconstructed: Vec<u8> = blocks
        .iter()
        .flat_map(|f| f.payload[4..].iter().copied())
        .take(image.len())
        .collect();
    assert_eq!(image, reconstructed);
}
#[tokio::test]
async fn commit_once_and_ble_setup_precedes_transfer() {
    let image = imaging::prepare_job(
        image::DynamicImage::new_rgb8(30, 40),
        60,
        80,
        50000,
        Fit::Crop,
        None,
        true,
    )
    .unwrap()
    .jpeg;
    let replies = vec![
        response(Command::BLE_CONNECT, vec![0]),
        response(Command::START, vec![0, 0, 0, 7, 16]),
        response(Command::DATA, vec![0, 0, 0, 0, 0]),
        response(Command::END, vec![0]),
        response(Command::INFO, vec![0, 2, 0x3a, 0, 0, 0, 0, 0, 0, 0]),
        response(Command::PRINT, vec![0, 16]),
    ];
    let (mut session, sent) = mock(replies, true);
    assert_eq!(
        session
            .upload(&image, &info(), true, [2, 0, 1, 0])
            .await
            .unwrap(),
        TransferOutcome::PrintAccepted {
            estimated_seconds: 16
        }
    );
    let sent = sent.lock().unwrap();
    assert_eq!(sent[0].command, Command::BLE_CONNECT);
    assert_eq!(sent[0].payload, [0, 12, 0, 12, 0, 0, 1, 144]);
    assert_eq!(sent[1].command, Command::INFO);
    assert_eq!(
        sent.iter().filter(|f| f.command == Command::PRINT).count(),
        1
    );
    assert_eq!(&sent[2].payload[..4], &[2, 0, 1, 0]);
}
#[cfg(feature = "ble")]
#[test]
fn auto_fragment_follows_negotiated_mtu() {
    use outstax_client::ble::fragment_size;
    assert_eq!(fragment_size(23), 20);
    assert_eq!(fragment_size(0), 20);
    assert_eq!(fragment_size(247), 244);
    assert_eq!(fragment_size(517), 512);
}
#[tokio::test]
async fn wrong_sequence_aborts_before_end_or_print() {
    let (mut session, sent) = mock(
        vec![
            response(Command::START, vec![0, 0, 0, 7, 16]),
            response(Command::DATA, vec![0, 0, 0, 0, 9]),
        ],
        false,
    );
    assert!(session
        .upload(&jpeg(), &info(), true, [2, 0, 0, 0])
        .await
        .is_err());
    assert!(sent
        .lock()
        .unwrap()
        .iter()
        .all(|f| f.command != Command::PRINT && f.command != Command::END));
}
#[tokio::test]
async fn timeout_poisoning_prevents_retry_of_print() {
    let (mut session, sent) = mock(vec![], false);
    session.set_timeout(Duration::from_millis(5));
    assert!(session
        .exchange(Frame::request(Command::PRINT, vec![]))
        .await
        .is_err());
    assert!(session
        .exchange(Frame::request(Command::PRINT, vec![]))
        .await
        .is_err());
    assert_eq!(sent.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn fragmented_reply_and_unrelated_event() {
    let ack = response(Command::INFO, vec![0, 1, 2, 80, 0, 0]);
    let mut replies = vec![response(Command::AXES, vec![0, 1, 2, 3])];
    replies.extend(ack.chunks(2).map(|s| s.to_vec()));
    let (mut session, _) = mock(replies, false);
    let frame = session
        .exchange(outstax_protocol::query(outstax_protocol::Info::Battery))
        .await
        .unwrap();
    assert_eq!(Battery::parse(&frame).unwrap().percent, 80);
}
#[tokio::test]
async fn unavailable_design_and_empty_film_stop_before_upload() {
    let (mut session, sent) = mock(vec![], false);
    let mut caps = info();
    caps.image.options2 = None;
    assert!(session
        .upload(&jpeg(), &caps, true, [2, 0, 1, 0])
        .await
        .is_err());
    assert!(sent.lock().unwrap().is_empty());
    let (mut session, sent) = mock(
        vec![response(
            Command::INFO,
            vec![0, 2, 0x30, 0, 0, 0, 0, 0, 0, 0],
        )],
        false,
    );
    caps.status.film_remaining = 0;
    assert!(session
        .upload(&jpeg(), &caps, true, [2, 0, 0, 0])
        .await
        .is_err());
    assert_eq!(sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn negative_device_result_does_not_desynchronize_session() {
    let (mut session, sent) = mock(
        vec![
            response(Command::AXES, vec![0x80]),
            response(Command::INFO, vec![0, 1, 2, 80, 0, 0]),
        ],
        false,
    );
    assert!(session
        .exchange(Frame::request(Command::AXES, vec![]))
        .await
        .is_err());
    let frame = session
        .exchange(outstax_protocol::query(outstax_protocol::Info::Battery))
        .await
        .unwrap();
    assert_eq!(Battery::parse(&frame).unwrap().percent, 80);
    assert_eq!(sent.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn completion_handles_c2_busy_and_last_exposure_without_reprinting() {
    let (mut session, sent) = mock(
        vec![
            response(Command::INFO, vec![0xc2]),
            response(Command::INFO, vec![0, 2, 0x31, 2, 0, 0, 0, 0, 0, 0]),
            response(Command::INFO, vec![0, 2, 0x30, 0, 0, 0, 0, 0, 0, 0]),
        ],
        false,
    );
    let status = session
        .wait_for_print(Duration::from_secs(6))
        .await
        .unwrap();
    assert_eq!(status.film_remaining, 0);
    assert!(!status.busy);
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 3);
    assert!(sent
        .iter()
        .all(|f| f.command == Command::INFO && f.payload == [2]));
}

fn identity_replies(model: &str, firmware: &str) -> Vec<Vec<u8>> {
    let mut r = vec![response(Command::VERSION, vec![0, 1, 0, 0, 0, 0, 0, 0, 0])];
    let mut selectors: Vec<u8> = (0..=5).collect();
    if model == "FI033" {
        selectors.extend([9, 10]);
    }
    for selector in selectors {
        let text: &[u8] = match selector {
            0 => b"FUJIFILM",
            1 => model.as_bytes(),
            4 => firmware.as_bytes(),
            _ => b"x",
        };
        r.push(response(
            Command::DEVICE_INFO,
            [&[0, selector, text.len() as u8][..], text].concat(),
        ));
    }
    r.push(response(
        Command::INFO,
        vec![0, 0, 2, 88, 3, 32, 2, 8, 0, 1, 224, 0],
    ));
    r.push(response(Command(0x20, 0x10), vec![0, 1, 0]));
    if model == "FI033" {
        r.push(response(Command(0x20, 0xa8), vec![0, 1, 0]));
    }
    if model == "SP-4" {
        r.push(response(
            Command::IDENTIFY,
            [&[0, 1, 14][..], b"AD123456789012"].concat(),
        ));
    }
    r
}
/// Replies for an upload-only transfer that follows the identity replies.
fn upload_replies(ble: bool, calibration: Vec<Vec<u8>>, blocks: usize) -> Vec<Vec<u8>> {
    let mut r = vec![];
    if ble {
        r.push(response(Command::BLE_CONNECT, vec![0]));
    }
    r.push(response(
        Command::INFO,
        vec![0, 2, 0x3a, 0, 0, 0, 0, 0, 0, 0],
    ));
    r.extend(calibration);
    // mini Link 0104 uses fixed 900-byte blocks.
    r.push(response(Command::START, vec![0, 0, 0, 0x03, 0x84]));
    for seq in 0..blocks as u32 {
        let mut ack = vec![0];
        ack.extend(seq.to_be_bytes());
        r.push(response(Command::DATA, ack));
    }
    r.push(response(Command::CANCEL, vec![0]));
    r
}
const HEAD_INFO: Command = Command(0x30, 0x80);
const HEAD_SET: Command = Command(0x30, 0x81);
/// FUTABA (0), judge 1, made 2020-01, rgb 30/30/30: a correction is due.
fn futaba_judge1() -> Vec<u8> {
    response(
        HEAD_INFO,
        vec![0, 0, 1, 0, 0x07, 0xe4, 1, 1, 0, 30, 0, 30, 0, 30],
    )
}
async fn identify_and_upload(
    ble: bool,
    firmware: &str,
    calibration: Vec<Vec<u8>>,
) -> (Result<TransferOutcome>, Vec<Command>) {
    identify_and_upload_delayed(ble, firmware, calibration, Duration::ZERO).await
}
async fn identify_and_upload_delayed(
    ble: bool,
    firmware: &str,
    calibration: Vec<Vec<u8>>,
    data_delay: Duration,
) -> (Result<TransferOutcome>, Vec<Command>) {
    // SP-4 uploads must pass the mini Link 0104 JPEG checks (600x800 baseline YCbCr).
    let mut info = info();
    (info.image.width, info.image.height, info.image.max_bytes) = (600, 800, 153_600);
    // A detailed pattern so the upload spans many blocks.
    let pattern = image::RgbImage::from_fn(600, 800, |x, y| {
        image::Rgb([(x * 7) as u8, (y * 3) as u8, (x ^ y) as u8])
    });
    let jpeg = imaging::prepare(
        image::DynamicImage::ImageRgb8(pattern),
        600,
        800,
        153_600,
        Fit::Crop,
    )
    .unwrap()
    .jpeg;
    let mut replies = identity_replies("SP-4", firmware);
    replies.extend(upload_replies(ble, calibration, jpeg.len().div_ceil(900)));
    let (mut session, sent) = mock_delayed(replies, ble, data_delay);
    let identity = session.identity().await.unwrap();
    assert_eq!(identity.head, Some(outstax_protocol::HeadType::Futaba));
    let outcome = session.upload(&jpeg, &info, false, [2, 0, 0, 0]).await;
    let commands = sent.lock().unwrap().iter().map(|f| f.command).collect();
    (outcome, commands)
}
#[tokio::test]
async fn sp4_identity_ends_with_light_pen_query() {
    let (mut session, sent) = mock(identity_replies("SP-4", "0103"), false);
    session.identity().await.unwrap();
    let last = sent.lock().unwrap().last().cloned().unwrap();
    assert_eq!(last.encode().unwrap(), [0x41, 0x62, 0, 8, 0, 0x10, 1, 0x43]);
}
#[tokio::test]
async fn head_calibration_is_skipped_over_ble() {
    let (outcome, commands) = identify_and_upload(true, "0103", vec![]).await;
    assert_eq!(outcome.unwrap(), TransferOutcome::Uploaded);
    assert!(!commands.contains(&HEAD_INFO) && !commands.contains(&HEAD_SET));
}
#[tokio::test]
async fn head_calibration_is_skipped_below_firmware_103() {
    let (outcome, commands) = identify_and_upload(false, "01.02", vec![]).await;
    assert_eq!(outcome.unwrap(), TransferOutcome::Uploaded);
    assert!(!commands.contains(&HEAD_INFO));
}
#[tokio::test]
async fn head_calibration_precedes_start_on_classic_sp4() {
    let calibration = vec![futaba_judge1(), response(HEAD_SET, vec![0])];
    let (outcome, commands) = identify_and_upload(false, "01.03", calibration).await;
    assert_eq!(outcome.unwrap(), TransferOutcome::Uploaded);
    let at = |c| commands.iter().position(|x| *x == c).unwrap();
    assert!(at(Command::INFO) < at(HEAD_INFO));
    assert_eq!(at(HEAD_INFO) + 1, at(HEAD_SET));
    assert_eq!(at(HEAD_SET) + 1, at(Command::START));
}
#[tokio::test]
async fn head_calibration_failure_aborts_before_start() {
    for calibration in [
        vec![response(HEAD_INFO, vec![0xb3])],
        vec![futaba_judge1(), response(HEAD_SET, vec![0x81])],
    ] {
        let (outcome, commands) = identify_and_upload(false, "0103", calibration).await;
        assert!(outcome.is_err());
        assert!(!commands.contains(&Command::START));
    }
}
#[tokio::test]
async fn initialize_runs_per_model_steps_in_app_order() {
    let status = || response(Command::INFO, vec![0, 2, 0x3a, 0, 0, 0, 0, 0, 0, 0]);
    let history = || response(Command::INFO, vec![0, 3, 0, 0, 0, 1, 0, 0, 0, 0]);
    let battery = || response(Command::INFO, vec![0, 1, 2, 80, 0, 0]);
    let extra = || {
        vec![
            response(Command::ADDITIONAL_INFO, vec![0, 0, 0x0f, 0xa0, 0, 25]),
            response(Command::ADDITIONAL_INFO, vec![0, 1, 0, 0, 0, 5, 1, 0, 0, 1]),
        ]
    };
    let sleep = || response(Command::SLEEP, vec![0, 0, 180, 0, 0, 0, 0, 0, 0]);
    let ok = |c| response(c, vec![0]);
    let info_steps = [(Command::INFO, 2), (Command::INFO, 3), (Command::INFO, 1)];
    let extra_steps = [(Command::ADDITIONAL_INFO, 0), (Command::ADDITIONAL_INFO, 1)];
    let cases = [
        (
            "SP-4",
            vec![status(), history(), battery(), ok(Command(0x30, 2))],
            [&info_steps[..], &[(Command(0x30, 2), 2)]].concat(),
        ),
        (
            "FI033",
            [
                vec![status(), history(), battery(), ok(Command(0x30, 0x20))],
                extra(),
                vec![ok(Command(0x30, 2))],
            ]
            .concat(),
            [
                &info_steps[..],
                &[(Command(0x30, 0x20), 3)],
                &extra_steps,
                &[(Command(0x30, 2), 2)],
            ]
            .concat(),
        ),
        (
            "FI017",
            [
                vec![response(Command(0x30, 8), vec![0, 1])],
                extra(),
                vec![sleep()],
            ]
            .concat(),
            [
                &[(Command(0x30, 8), 2)][..],
                &extra_steps,
                &[(Command::SLEEP, 0)],
            ]
            .concat(),
        ),
        (
            "BO-22",
            [extra(), vec![sleep()]].concat(),
            [&extra_steps[..], &[(Command::SLEEP, 0)]].concat(),
        ),
    ];
    for (model, init_replies, expected) in cases {
        let mut replies = identity_replies(model, "0103");
        let skip = replies.len();
        replies.extend(init_replies);
        let (mut session, sent) = mock(replies, false);
        let identity = session.identity().await.unwrap();
        let init = session.initialize(&identity).await.unwrap();
        let sent: Vec<_> = sent.lock().unwrap()[skip..]
            .iter()
            .map(|f| (f.command, f.payload[0]))
            .collect();
        assert_eq!(sent, expected, "{model}");
        assert_eq!(init.cpu_type(), (model != "SP-4").then_some(1), "{model}");
        if matches!(model, "FI017" | "BO-22") {
            assert_eq!(init.sleep_timers, Some([180, 0, 0, 0]));
        }
    }
}

/// Our encoder's output satisfies the mini Link 0104 decoder-wrapper checks.
fn mini1_jpeg() -> Vec<u8> {
    imaging::prepare(
        image::DynamicImage::new_rgb8(300, 400),
        600,
        800,
        153_600,
        Fit::Crop,
    )
    .unwrap()
    .jpeg
}
#[test]
fn prepared_jpeg_passes_mini1_firmware_checks() {
    let header = outstax_protocol::jpeg::check_mini1(&mini1_jpeg()).unwrap();
    assert_eq!((header.width, header.height), (600, 800));
}
/// Session-level counterpart of the external upload plan: START `[2,0,0,0,len]`,
/// 911-byte DATA frames in sequence order, then empty END, and no PRINT.
#[tokio::test]
async fn mini1_upload_uses_fixed_911_byte_data_frames() {
    let image = mini1_jpeg();
    let count = image.len().div_ceil(outstax_protocol::IMAGE_BLOCK_LEN);
    assert!(count > 1);
    let mut replies = vec![response(Command::START, vec![0, 0, 0, 3, 0x84])];
    for i in 0..count {
        replies.push(response(
            Command::DATA,
            [vec![0], (i as u32).to_be_bytes().to_vec()].concat(),
        ));
    }
    replies.push(response(Command::CANCEL, vec![0]));
    let (mut session, sent) = mock(replies, false);
    let mut caps = info();
    caps.image = ImageSupport {
        width: 600,
        height: 800,
        picture_types: 2,
        options: 0,
        max_bytes: 153_600,
        options2: None,
    };
    assert_eq!(
        session
            .upload(&image, &caps, false, [2, 0, 0, 0])
            .await
            .unwrap(),
        TransferOutcome::Uploaded
    );
    let sent = sent.lock().unwrap();
    let start = sent
        .iter()
        .position(|f| f.command == Command::START)
        .unwrap();
    assert_eq!(
        sent[start].payload,
        [
            vec![2, 0, 0, 0],
            (image.len() as u32).to_be_bytes().to_vec()
        ]
        .concat()
    );
    let data = &sent[start + 1..start + 1 + count];
    for (i, frame) in data.iter().enumerate() {
        assert_eq!(frame.command, Command::DATA);
        assert_eq!(frame.encode().unwrap().len(), 911);
        assert_eq!(frame.payload[..4], (i as u32).to_be_bytes());
    }
    // Upload-only cancels while the transfer is open: CANCEL replaces END.
    assert_eq!(
        sent[start + 1 + count],
        Frame::request(Command::CANCEL, vec![])
    );
    assert_eq!(sent.len(), start + 2 + count);
}
#[tokio::test(start_paused = true)]
async fn mini1_upload_stops_at_the_transfer_window_and_cancels() {
    // 12 s per block (virtual time, under the 15 s per-exchange timeout): the ~49 s
    // window from the START reply runs out after five blocks, long before the JPEG ends.
    let (outcome, commands) =
        identify_and_upload_delayed(true, "0103", vec![], Duration::from_secs(12)).await;
    let error = format!("{:#}", outcome.unwrap_err());
    assert!(error.contains("transfer window"), "{error}");
    assert_eq!(commands.iter().filter(|c| **c == Command::DATA).count(), 5);
    assert!(commands.contains(&Command::CANCEL) && !commands.contains(&Command::END));
}
#[tokio::test]
async fn square_over_ble_skips_connection_parameters() {
    // The SQUARE app never sends 0103; the client follows it on BLE.
    let image = jpeg();
    let mut replies = identity_replies("FI017", "0101");
    replies.extend(upload_replies(false, vec![], image.len().div_ceil(900)));
    let (mut session, sent) = mock(replies, true);
    session.identity().await.unwrap();
    let outcome = session.upload(&image, &info(), false, [2, 0, 0, 0]).await;
    assert_eq!(outcome.unwrap(), TransferOutcome::Uploaded);
    let commands: Vec<Command> = sent.lock().unwrap().iter().map(|f| f.command).collect();
    assert!(!commands.contains(&Command::BLE_CONNECT));
    assert!(commands.contains(&Command::START) && commands.contains(&Command::CANCEL));
}
#[tokio::test]
async fn mini3_refuses_design_print_even_if_advertised() {
    // info() advertises options2 bit 0, but both Link 3 builds reject any option2.
    let (mut session, sent) = mock(identity_replies("FI033", "0102"), true);
    session.identity().await.unwrap();
    let before = sent.lock().unwrap().len();
    // Design Print doubles the height: 60x160 passes the dimension check first.
    let tall = imaging::prepare(
        image::DynamicImage::new_rgb8(60, 160),
        60,
        160,
        50000,
        Fit::Crop,
    )
    .unwrap()
    .jpeg;
    let error = session
        .upload(&tall, &info(), false, [2, 0, 1, 0])
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("Design Print"), "{error:#}");
    assert_eq!(
        sent.lock().unwrap().len(),
        before,
        "nothing sent after identity"
    );
}
