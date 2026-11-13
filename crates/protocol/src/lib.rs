//! Transport-independent INSTAX Link protocol. All integer wire fields are big endian.
//! Protocol facts initially cross-checked against Jasper van Loenen's MIT InstaxBLE.
//! See docs/protocol/PROTOCOL.md for evidence and remaining uncertainties.
use std::{error::Error, fmt};

pub mod device;
pub mod firmware;
pub mod jpeg;
pub mod readback;
pub mod settings;
pub use device::*;

pub const SERVICE_UUID: &str = "70954782-2d83-473d-9e5f-81e1d02d5273";
pub const WRITE_UUID: &str = "70954783-2d83-473d-9e5f-81e1d02d5273";
pub const NOTIFY_UUID: &str = "70954784-2d83-473d-9e5f-81e1d02d5273";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError(pub String);
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ProtocolError {}
pub type Result<T> = std::result::Result<T, ProtocolError>;
fn err(message: impl Into<String>) -> ProtocolError {
    ProtocolError(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Request,
    Response,
}
impl Direction {
    fn magic(self) -> [u8; 2] {
        match self {
            Self::Request => *b"Ab",
            Self::Response => *b"aB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command(pub u8, pub u8);
impl Command {
    pub const VERSION: Self = Self(0x00, 0x00);
    pub const DEVICE_INFO: Self = Self(0x00, 0x01);
    pub const INFO: Self = Self(0x00, 0x02);
    pub const IDENTIFY: Self = Self(0x00, 0x10);
    pub const SLEEP: Self = Self(0x01, 0x02);
    pub const BLE_CONNECT: Self = Self(0x01, 0x03);
    pub const START: Self = Self(0x10, 0x00);
    pub const DATA: Self = Self(0x10, 0x01);
    pub const END: Self = Self(0x10, 0x02);
    pub const CANCEL: Self = Self(0x10, 0x03);
    /// `1010`-`1012` stored-image readback; see [`readback`].
    pub const READBACK_INFO: Self = Self(0x10, 0x10);
    pub const READBACK_DATA: Self = Self(0x10, 0x11);
    pub const READBACK_END: Self = Self(0x10, 0x12);
    pub const PRINT: Self = Self(0x10, 0x80);
    pub const AXES: Self = Self(0x30, 0x00);
    pub const LED: Self = Self(0x30, 0x01);
    pub const ADDITIONAL_INFO: Self = Self(0x30, 0x10);
    pub fn is_firmware(self) -> bool {
        self.0 == 0x20
    }
}
impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}{:02x}", self.0, self.1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub direction: Direction,
    pub command: Command,
    pub payload: Vec<u8>,
}
impl Frame {
    pub fn request(command: Command, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            direction: Direction::Request,
            command,
            payload: payload.into(),
        }
    }
    pub fn response(command: Command, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            direction: Direction::Response,
            command,
            payload: payload.into(),
        }
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        let length = self
            .payload
            .len()
            .checked_add(7)
            .filter(|x| *x <= u16::MAX as usize)
            .ok_or_else(|| err("frame exceeds 65535 bytes"))?;
        let mut data = Vec::with_capacity(length);
        data.extend_from_slice(&self.direction.magic());
        data.extend_from_slice(&(length as u16).to_be_bytes());
        data.extend_from_slice(&[self.command.0, self.command.1]);
        data.extend_from_slice(&self.payload);
        data.push(!data.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)));
        Ok(data)
    }
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 7 {
            return Err(err("frame shorter than 7 bytes"));
        }
        let direction = match &data[..2] {
            b"Ab" => Direction::Request,
            b"aB" => Direction::Response,
            _ => return Err(err("invalid frame magic")),
        };
        let declared = u16::from_be_bytes([data[2], data[3]]) as usize;
        if declared != data.len() {
            return Err(err(format!(
                "declared length {declared}, actual {}",
                data.len()
            )));
        }
        if data.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) != 255 {
            return Err(err("invalid checksum"));
        }
        Ok(Self {
            direction,
            command: Command(data[4], data[5]),
            payload: data[6..data.len() - 1].to_vec(),
        })
    }
    pub fn success_body(&self) -> Result<&[u8]> {
        if self.direction != Direction::Response {
            return Err(err("expected printer response"));
        }
        let (&status, body) = self
            .payload
            .split_first()
            .ok_or_else(|| err("missing response result"))?;
        if status != 0 {
            return Err(err(format!(
                "printer rejected {}: {}",
                self.command,
                DeviceResult(status)
            )));
        }
        Ok(body)
    }
}

/// Reassembles split notifications and multiple coalesced frames. Fails closed on corruption.
#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
}
impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Frame>> {
        let mut frames = Vec::new();
        for byte in bytes {
            self.pending.push(*byte);
            if self.pending.len() >= 2 && !matches!(&self.pending[..2], b"Ab" | b"aB") {
                self.pending.clear();
                return Err(err("invalid stream magic"));
            }
            if self.pending.len() >= 4 {
                let length = u16::from_be_bytes([self.pending[2], self.pending[3]]) as usize;
                if length < 7 {
                    self.pending.clear();
                    return Err(err("invalid stream frame length"));
                }
                if self.pending.len() == length {
                    let data = std::mem::take(&mut self.pending);
                    frames.push(Frame::decode(&data)?);
                }
            }
        }
        Ok(frames)
    }
    pub fn finish(&self) -> Result<()> {
        if self.pending.is_empty() {
            Ok(())
        } else {
            Err(err("truncated frame at end of stream"))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Info {
    Image = 0,
    Battery = 1,
    Printer = 2,
    PrintHistory = 3,
    Camera = 4,
    CameraHistory = 5,
}
pub fn query(info: Info) -> Frame {
    Frame::request(Command::INFO, vec![info as u8])
}
pub fn info_body(frame: &Frame, selector: Info) -> Result<&[u8]> {
    if frame.command != Command::INFO {
        return Err(err("unexpected information command"));
    }
    let body = frame.success_body()?;
    if body.first() != Some(&(selector as u8)) {
        return Err(err("unexpected information selector"));
    }
    Ok(&body[1..])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSupport {
    pub width: u16,
    pub height: u16,
    pub picture_types: u8,
    pub options: u8,
    pub max_bytes: u32,
    pub options2: Option<u8>,
}
impl ImageSupport {
    pub fn parse(frame: &Frame) -> Result<Self> {
        let p = info_body(frame, Info::Image)?;
        if p.len() < 10 {
            return Err(err(
                "image capabilities require dimensions, format flags, and maximum byte count",
            ));
        }
        let value = Self {
            width: u16::from_be_bytes([p[0], p[1]]),
            height: u16::from_be_bytes([p[2], p[3]]),
            picture_types: p[4],
            options: p[5],
            max_bytes: u32::from_be_bytes(p[6..10].try_into().unwrap()),
            // Mini Link 7.0.6 reads this extension only in 23-byte responses.
            options2: if frame.payload.len() + 7 == 23 {
                p.get(10).copied()
            } else {
                None
            },
        };
        if value.width == 0
            || value.height == 0
            || value.max_bytes == 0
            || value.max_bytes > 64 * 1024 * 1024
        {
            return Err(err("invalid image capabilities"));
        }
        Ok(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battery {
    pub state: u8,
    pub percent: u8,
    pub charger_type: u8,
    pub charger_state: u8,
}
impl Battery {
    pub fn parse(frame: &Frame) -> Result<Self> {
        let p = info_body(frame, Info::Battery)?;
        if p.len() < 4 || p[1] > 100 {
            return Err(err("invalid battery response"));
        }
        Ok(Self {
            state: p[0],
            percent: p[1],
            charger_type: p[2],
            charger_state: p[3],
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterStatus {
    pub film_remaining: u8,
    pub charging: bool,
    pub battery_level: u8,
    pub cover_open: bool,
    pub busy: bool,
    pub error: bool,
    pub operation: u8,
    pub print_result: u8,
    pub wait_seconds: u8,
    pub error_code: u32,
    pub raw: Vec<u8>,
}
impl PrinterStatus {
    pub fn parse(frame: &Frame) -> Result<Self> {
        let p = info_body(frame, Info::Printer)?;
        if p.len() < 8 {
            return Err(err("printer status requires eight bytes"));
        }
        let flags = p[0];
        Ok(Self {
            film_remaining: flags & 15,
            charging: flags & 128 != 0,
            battery_level: (flags >> 4) & 7,
            cover_open: p[1] & 1 != 0,
            busy: p[1] & 2 != 0,
            error: p[1] & 4 != 0,
            operation: p[1] >> 4,
            print_result: p[2],
            wait_seconds: p[3],
            error_code: u32::from_be_bytes(p[4..8].try_into().unwrap()),
            raw: p.to_vec(),
        })
    }
}

impl PrinterStatus {
    /// Matches the app's isPrinterGood gate. During completion, zero remaining
    /// film is permitted because the job may have consumed the last exposure.
    pub fn readiness(&self, printing: bool) -> DeviceResult {
        if !matches!(self.battery_level, 1..=3) {
            return DeviceResult(0x90);
        }
        if self.error {
            return DeviceResult(match self.print_result {
                1 => 0xb0,
                2 => 0xb2,
                127 => 0x7f,
                _ => {
                    let priority = [
                        (0, 0x90),
                        (1, 0x91),
                        (2, 0x92),
                        (3, 0x93),
                        (4, 0xb4),
                        (5, 0xb5),
                        (6, 0xb0),
                        (29, 0xf0),
                        (30, 0xf1),
                        (31, 0xf2),
                    ];
                    priority
                        .into_iter()
                        .find(|(bit, _)| self.error_code & (1u32 << bit) != 0)
                        .map_or(0xff, |(_, result)| result)
                }
            });
        }
        if self.busy {
            return DeviceResult(1);
        }
        if self.cover_open {
            return DeviceResult(0xb0);
        }
        if self.film_remaining == 0 && !printing {
            return DeviceResult(0xb2);
        }
        DeviceResult(0)
    }
    pub fn firmware_battery_ready(&self) -> bool {
        matches!(self.battery_level, 2 | 3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Mini,
    Square,
    Wide,
}
impl Format {
    pub fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Mini => (600, 800),
            Self::Square => (800, 800),
            Self::Wide => (1260, 840),
        }
    }
    pub fn from_dimensions(w: u16, h: u16) -> Option<Self> {
        match (w, h) {
            (600, 800) => Some(Self::Mini),
            (800, 800) => Some(Self::Square),
            (1260, 840) => Some(Self::Wide),
            _ => None,
        }
    }
}

/// Every analysed STM32 image (mini Link, Link 2, WIDE, Link 3, Link+) and the
/// Renesas RX builds of Link 2/3 fix print DATA at a 911-byte frame: sequence plus
/// exactly this many bytes, final block zero-padded. SQUARE and WIDE 2 (Sunplus)
/// take the segment size from the START reply and have no fixed frame, so the
/// session always uses the START reply. docs/re/firmware/{firmware,mini-link-0104,
/// renesas-mini2-mini3,square,wide2}.md.
pub const IMAGE_BLOCK_LEN: usize = 900;

/// Print START reply advertises the DATA block size; do not infer model from it.
pub fn transfer_chunk_size(frame: &Frame) -> Result<usize> {
    if frame.command != Command::START {
        return Err(err("expected START acknowledgement"));
    }
    let p = frame.success_body()?;
    if p.len() < 4 {
        return Err(err("START reply omits chunk size"));
    }
    let size = u32::from_be_bytes(p[..4].try_into().unwrap()) as usize;
    if size == 0 || size > u16::MAX as usize - 11 {
        return Err(err("invalid transfer chunk size"));
    }
    Ok(size)
}

pub fn download_start(image_len: usize, options: [u8; 4]) -> Result<Frame> {
    if image_len == 0 || image_len > u32::MAX as usize {
        return Err(err("invalid image length"));
    }
    let mut payload = options.to_vec();
    payload.extend_from_slice(&(image_len as u32).to_be_bytes());
    Ok(Frame::request(Command::START, payload))
}
pub fn download_chunks(image: &[u8], chunk_size: usize) -> Result<Vec<Frame>> {
    if image.is_empty() || chunk_size == 0 || chunk_size > u16::MAX as usize - 11 {
        return Err(err("invalid image or chunk size"));
    }
    image
        .chunks(chunk_size)
        .enumerate()
        .map(|(index, chunk)| {
            let index = u32::try_from(index).map_err(|_| err("too many chunks"))?;
            let mut payload = vec![0; 4 + chunk_size];
            payload[..4].copy_from_slice(&index.to_be_bytes());
            payload[4..4 + chunk.len()].copy_from_slice(chunk);
            Ok(Frame::request(Command::DATA, payload))
        })
        .collect()
}
/// Opt-in and untested on hardware. mini Link 0104 sets its first-block and
/// last-block markers in an if/else, so END after a one-block upload returns
/// 0x82 unless sequence 0 is sent twice (static analysis). DATA has no
/// next-sequence check and overwrites `seq * 900`, so the repeat is idempotent.
/// Other models are unknown; the session does not apply this. Each DATA ACK
/// echoes the sequence in the frame, not the frame's position in this list.
pub fn repeat_single_block(mut chunks: Vec<Frame>) -> Vec<Frame> {
    if chunks.len() == 1 {
        chunks.push(chunks[0].clone());
    }
    chunks
}

pub fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(text: &str) -> Result<Vec<u8>> {
    let s: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .collect();
    if !s.is_ascii() || !s.len().is_multiple_of(2) {
        return Err(err("hex input must contain complete ASCII bytes"));
    }
    s.as_bytes()
        .chunks(2)
        .map(|p| {
            u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16)
                .map_err(|_| err("invalid hex digit"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_query_bytes() {
        assert_eq!(
            hex(&query(Info::Image).encode().unwrap()),
            "4162000800020052"
        );
        assert_eq!(
            hex(&Frame::request(Command::PRINT, vec![]).encode().unwrap()),
            "416200071080c5"
        );
    }
    #[test]
    fn real_wide_start_ack() {
        let frame = Frame::decode(&unhex("61 42 00 0c 10 00 00 00 00 03 84 b9").unwrap()).unwrap();
        assert_eq!(transfer_chunk_size(&frame).unwrap(), 900);
    }
    #[test]
    fn all_fragment_boundaries_and_coalescing() {
        let frame = query(Info::Image);
        let wire = frame.encode().unwrap();
        for split in 0..=wire.len() {
            let mut decoder = Decoder::default();
            let mut frames = decoder.push(&wire[..split]).unwrap();
            frames.extend(decoder.push(&wire[split..]).unwrap());
            assert_eq!(frames, vec![frame.clone()]);
            decoder.finish().unwrap();
        }
        let mut decoder = Decoder::default();
        assert_eq!(
            decoder.push(&[wire.clone(), wire].concat()).unwrap().len(),
            2
        );
    }
    #[test]
    fn corruption_and_truncation() {
        let mut wire = query(Info::Image).encode().unwrap();
        wire[6] ^= 1;
        assert!(Frame::decode(&wire).is_err());
        let mut d = Decoder::default();
        d.push(b"Ab\x00\x08").unwrap();
        assert!(d.finish().is_err());
        assert!(Decoder::default().push(b"Ab\x00\x00").is_err());
        assert!(Decoder::default().push(b"xx").is_err());
    }
    #[test]
    fn chunk_padding_and_index() {
        let chunks = download_chunks(&[1, 2, 3, 4, 5], 3).unwrap();
        assert_eq!(chunks[0].payload, vec![0, 0, 0, 0, 1, 2, 3]);
        assert_eq!(chunks[1].payload, vec![0, 0, 0, 1, 4, 5, 0]);
        assert!(download_chunks(&[], 900).is_err());
        assert!(download_chunks(&[1], 0).is_err());
    }
    #[test]
    fn capabilities_keep_advertised_limits() {
        let f = Frame::response(Command::INFO, vec![0, 0, 4, 236, 3, 72, 2, 8, 0, 5, 40, 0]);
        assert_eq!(
            ImageSupport::parse(&f).unwrap(),
            ImageSupport {
                width: 1260,
                height: 840,
                picture_types: 2,
                options: 8,
                max_bytes: 337920,
                options2: None
            }
        );
    }
    #[test]
    fn mini_link_plus_extension_is_length_gated() {
        let mut payload = vec![0, 0, 2, 88, 3, 32, 2, 8, 0, 1, 224, 0, 1, 0, 0, 0];
        let extended = Frame::response(Command::INFO, payload.clone());
        assert_eq!(extended.encode().unwrap().len(), 23);
        assert_eq!(ImageSupport::parse(&extended).unwrap().options2, Some(1));
        payload.truncate(12);
        assert_eq!(
            ImageSupport::parse(&Frame::response(Command::INFO, payload))
                .unwrap()
                .options2,
            None
        );
    }
    #[test]
    fn unknown_opcodes_round_trip() {
        let f = Frame::response(Command(0xff, 0xff), vec![2, 3, 4]);
        assert_eq!(Frame::decode(&f.encode().unwrap()).unwrap(), f);
    }
    #[test]
    fn oversize_rejected_without_wrapping() {
        assert!(Frame::request(Command::DATA, vec![0; 65529])
            .encode()
            .is_err());
        assert!(Frame::request(Command::DATA, vec![0; 65528])
            .encode()
            .is_ok());
    }
    #[test]
    fn info_selector_and_error_are_checked() {
        assert!(Battery::parse(&Frame::response(Command::INFO, vec![0, 0, 2, 70])).is_err());
        assert!(Battery::parse(&Frame::response(Command::INFO, vec![1, 1, 2, 70])).is_err());
        assert!(Battery::parse(&Frame::response(Command::INFO, vec![0, 1, 2, 101])).is_err());
    }

    // Golden frames from the external mini Link 0104 upload plan
    // (examples/mini1-upload-plan): an 18 784-byte synthetic JPEG, 21 blocks.
    const MINI1_LAST_BLOCK_DATA: [&str; 20] = [
        "0299aa62014e02940a70141ac58805380a00a7014cd530029c052814a053354c00a502940a70141a",
        "a62014e02940a70141ac5880514f028a66a99f3a814e02940a502bcc3f90130029c050053c0a0d62",
        "c4029c05005380a66a98014e028029c053358b0029c05005380a0d53100a7014a05380a0d62c4029",
        "c05005380a66c98014e028029c053348b0029c05005380a0da2c4029c052814e0299aa62014e0280",
        "29c0506b16005380a502940a66b16005380a00a70141ac58805380a5029c053354c4029c052814e0",
        "28358b100a7014a05380a0d53100a7014014e0299ac58014e028029c0506a98014e028029c053358",
        "b100a7014a05380a66a98805380a00a70141b458014e028029c053354c00a7014014e0299ac58805",
        "380a5029c0506a98805380a00a78141ac58805380a502940a66a98014e028029c0506a98014e0280",
        "29c0506b160052814a05380a66b162014e02940a50299aa6005380a00a70141aa6005380a00a7014",
        "cd530029c05005380a0d62c4029c052814e028368b100a7014a05380a66898805380a502940a66c9",
        "8014e028029c050689880514f028a0d533e75029c052814e02bcd3f9022c4029c052814e028354c4",
        "029c052814e028358b100a7014a052814cd530029c05005380a66c98014e028029c0506a98805380",
        "a5029c053358b100a7014a05380a0d53100a7014a05380a66b162014e02940a5028354c4029c0528",
        "14e0299aa62014e02940a70141ac58805380a5029c053354c4029c052814a0506b162014e02940a7",
        "014cd53100a7014a05380a0d93100a7014a05380a669162014e02940a70141b458805380a00a7014",
        "cd13002940a5029c0506d162014e02940a7014cd53100a7014a05380a0d53100a7014014f0299aa6",
        "2014e02940a5028358b100a7014a05380a66a98805380a5029c0506b162014e02940a7014cd53100",
        "a7014a0528141ac58014e028029e0506a988052814a05380a66b162014e02940a7014cd53100a701",
        "4a05380a0d62c4029c052814e0299b262014e028029c05069162014e02940a70141b262014e02940",
        "a7014cd53100a7014a05380a66b16201453c0a28344cffd9",
    ];
    #[test]
    fn mini1_upload_plan_golden_frames() {
        assert_eq!(
            hex(&download_start(18_784, [2, 0, 0, 0])
                .unwrap()
                .encode()
                .unwrap()),
            "4162000f1000020000000000496092"
        );
        assert_eq!(
            hex(&Frame::request(Command::END, vec![]).encode().unwrap()),
            "41620007100243"
        );
        let tail = unhex(&MINI1_LAST_BLOCK_DATA.concat()).unwrap();
        assert_eq!(tail.len(), 784);
        let image = [vec![0xaa; 20 * IMAGE_BLOCK_LEN], tail.clone()].concat();
        let chunks = download_chunks(&image, IMAGE_BLOCK_LEN).unwrap();
        assert_eq!(chunks.len(), 21);
        assert!(chunks.iter().all(|c| c.encode().unwrap().len() == 911));
        let golden = [
            unhex("4162038f100100000014").unwrap(),
            tail,
            vec![0; IMAGE_BLOCK_LEN - 784],
            vec![0x62],
        ]
        .concat();
        assert_eq!(chunks[20].encode().unwrap(), golden);
    }
    #[test]
    fn single_block_repeat_is_opt_in() {
        let one = download_chunks(&[7; 900], IMAGE_BLOCK_LEN).unwrap();
        assert_eq!(one.len(), 1);
        let repeated = repeat_single_block(one.clone());
        assert_eq!(repeated, vec![one[0].clone(), one[0].clone()]);
        let two = download_chunks(&[7; 901], IMAGE_BLOCK_LEN).unwrap();
        assert_eq!(repeat_single_block(two.clone()), two);
    }
    #[test]
    fn mini1_identity_and_status_vectors() {
        assert_eq!(
            hex(&Frame::request(Command::VERSION, vec![]).encode().unwrap()),
            "41620007000055"
        );
        // 0000 returns the fixed body 01 00 00 00 00 00 00 00.
        let version = Frame::decode(&unhex("6142001000000001000000000000004b").unwrap()).unwrap();
        FunctionVersions::parse(&version)
            .unwrap()
            .validate_print()
            .unwrap();
        let image = Frame::response(Command::INFO, vec![0, 0, 2, 88, 3, 32, 2, 0, 0, 2, 88, 0]);
        assert_eq!(image.encode().unwrap().len(), 19);
        let caps = ImageSupport::parse(&image).unwrap();
        assert_eq!(
            (caps.width, caps.height, caps.options, caps.max_bytes),
            (600, 800, 0, 153_600)
        );
        let status = Frame::response(
            Command::INFO,
            vec![0, 2, 0x12, 0x34, 0x7f, 45, 0x89, 0xab, 0xcd, 0xef],
        );
        let s = PrinterStatus::parse(&status).unwrap();
        assert_eq!(
            (s.film_remaining, s.battery_level, s.charging),
            (2, 1, false)
        );
        assert!(s.error && !s.busy && !s.cover_open);
        assert_eq!((s.operation, s.print_result, s.wait_seconds), (3, 0x7f, 45));
        assert_eq!(s.error_code, 0x89ab_cdef);
        // The session's identity reads stay inside the mini Link handler ranges.
        assert!((0..=5).all(|s| MINI1_DEVICE_INFO_SELECTORS.contains(&s)));
        assert!((0..=3).all(|s| MINI1_INFO_SELECTORS.contains(&s)));
        assert!(MINI1_IDENTIFY_SELECTORS.contains(&1));
    }
    #[test]
    fn decoder_two_frames_at_every_split_and_bytewise() {
        let frames = [
            Frame::response(Command::INFO, vec![0, 2, 1, 2, 0x7f, 30, 0, 0, 1, 0]),
            Frame::response(Command::DATA, vec![0, 0, 0, 0, 7]),
        ];
        let wire: Vec<u8> = frames.iter().flat_map(|f| f.encode().unwrap()).collect();
        for split in 0..=wire.len() {
            let mut d = Decoder::default();
            let mut got = d.push(&wire[..split]).unwrap();
            got.extend(d.push(&wire[split..]).unwrap());
            assert_eq!(got, frames);
            d.finish().unwrap();
        }
        let mut d = Decoder::default();
        let mut got = vec![];
        for byte in &wire {
            got.extend(d.push(&[*byte]).unwrap());
        }
        assert_eq!(got, frames);
    }
    #[test]
    fn decoder_nested_magic_and_fail_closed_corruption() {
        // A magic inside a payload must not end the enclosing frame early.
        let nested = Frame::response(Command(0x12, 0x34), b"\0aB\x00\x08\x01\x02garbage".to_vec());
        assert_eq!(
            Decoder::default().push(&nested.encode().unwrap()).unwrap(),
            vec![nested]
        );
        // Unlike the external decoder we do not resynchronise: the session must
        // treat any corruption as a lost stream. Leading garbage, a bad
        // checksum and an impossible length all fail; the buffer is then empty.
        let good = Frame::response(Command::DATA, vec![0, 0, 0, 0, 3])
            .encode()
            .unwrap();
        let mut corrupt = good.clone();
        *corrupt.last_mut().unwrap() ^= 0x80;
        for input in [
            [&[0, 1][..], &good].concat(),
            corrupt,
            b"aB\x00\x01".to_vec(),
        ] {
            let mut d = Decoder::default();
            assert!(d.push(&input).is_err());
            d.finish().unwrap();
            assert_eq!(d.push(&good).unwrap().len(), 1);
        }
    }
}
