//! Typed fields recovered from the Android Link applications.
use crate::{err, info_body, Command, Frame, Info, Result};

/// Preserve every device result, including unknown future values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceResult(pub u8);
impl DeviceResult {
    pub fn name(self) -> &'static str {
        match self.0 {
            0 => "OK",
            1 => "PRINTER_BUSY",
            2 => "CAMERA_BUSY",
            0x7f => "PENDING_RESPONSE",
            0x80 => "SID_NOT_SUPPORTED",
            0x81 => "PARAMETER_ERROR",
            0x82 => "SEQUENCE_ERROR",
            0x83 => "OTHER_USED",
            0x84 => "SECURITY_CODE_ERROR",
            0x85 => "TIMEOUT",
            0x90 => "LOW_POWER",
            0x91 => "BATTERY_NOT_MOUNTED",
            0x92 => "BATTERY_OVERHEAT",
            0x93 => "BATTERY_CHARGE_FAULT",
            0xa0 => "FIRMWARE_NO_DATA",
            0xa1 => "FIRMWARE_DATA_ERROR",
            0xa2 => "FIRMWARE_UPDATE_FAILURE",
            0xa3 => "FIRMWARE_SDCARD_ERROR",
            0xb0 => "COVER_OPEN",
            0xb1 => "NO_FILM",
            0xb2 => "NO_PAPER",
            0xb3 => "IMAGE_ERROR",
            0xb4 => "JAMMED",
            0xb5 => "PRINT_FAULT",
            0xc0 => "MEMORY_FULL",
            0xc1 => "POST_VIEW_PRINT_ERROR",
            0xc2 => "NOW_PRINTING",
            0xf0 => "SOFTWARE_ABNORMALITY",
            0xf1 => "HARDWARE_ABNORMALITY",
            0xf2 => "MECHANICAL_ABNORMALITY",
            _ => "UNKNOWN",
        }
    }
}
impl std::fmt::Display for DeviceResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (0x{:02x})", self.name(), self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionVersions {
    pub functions: u8,
    pub device_info: u8,
    pub image: u8,
    pub battery: u8,
    pub printer: u8,
    pub print_history: u8,
    pub camera: u8,
    pub camera_history: u8,
}
impl FunctionVersions {
    /// Current app schemas use revision 0 for these structures and image <=2.
    pub fn validate_print(&self) -> Result<()> {
        if self.functions & 1 == 0 {
            return Err(err("device has no print function"));
        }
        if self.device_info > 0
            || self.image > 2
            || self.battery > 0
            || self.printer > 0
            || self.print_history > 0
        {
            return Err(err(
                "device reports a newer structure version than the recovered print schemas",
            ));
        }
        Ok(())
    }
    pub fn parse(frame: &Frame) -> Result<Self> {
        if frame.command != Command::VERSION {
            return Err(err("expected function versions"));
        }
        let p = frame.success_body()?;
        if p.len() < 8 {
            return Err(err("function versions require eight bytes"));
        }
        Ok(Self {
            functions: p[0],
            device_info: p[1],
            image: p[2],
            battery: p[3],
            printer: p[4],
            print_history: p[5],
            camera: p[6],
            camera_history: p[7],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkModel {
    Mini,
    Mini2,
    Mini3,
    MiniPlus,
    Square,
    Wide,
    Wide2,
}
impl LinkModel {
    pub fn from_model_number(model: &str) -> Option<Self> {
        match model.trim_end_matches('\0') {
            "SP-4" => Some(Self::Mini),
            "FI023" => Some(Self::Mini2),
            "FI033" => Some(Self::Mini3),
            "FI046" => Some(Self::MiniPlus),
            "FI017" => Some(Self::Square),
            "BO-22" => Some(Self::Wide),
            "FI049" => Some(Self::Wide2),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintHistory {
    pub prints: u32,
    pub cover_ejections: u32,
}
impl PrintHistory {
    pub fn parse(frame: &Frame) -> Result<Self> {
        let p = info_body(frame, Info::PrintHistory)?;
        if p.len() < 8 {
            return Err(err("history requires eight bytes"));
        }
        Ok(Self {
            prints: u32::from_be_bytes(p[..4].try_into().unwrap()),
            cover_ejections: u32::from_be_bytes(p[4..8].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Axes {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub switches: u8,
}
impl Axes {
    pub fn parse(frame: &Frame) -> Result<Self> {
        if frame.command != Command::AXES {
            return Err(err("expected axes"));
        }
        let p = frame.success_body()?;
        if p.len() < 7 {
            return Err(err("axes require seven bytes"));
        }
        Ok(Self {
            x: i16::from_be_bytes([p[0], p[1]]),
            y: i16::from_be_bytes([p[2], p[3]]),
            z: i16::from_be_bytes([p[4], p[5]]),
            switches: p[6],
        })
    }
    pub fn main_button(&self) -> bool {
        self.switches & 1 != 0
    }
    pub fn ar_pressure(&self) -> u8 {
        (self.switches >> 1) & 3
    }
    pub fn link3_switch(&self) -> bool {
        self.switches & 8 != 0
    }
}

/// 3010 selector 0: raw voltage/temperature units are not specified by the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdditionalInfo {
    Voltage {
        voltage: i16,
        temperature: i16,
    },
    Color {
        print_attempts: u32,
        battery_type: u8,
        color: u8,
        film_pi: u8,
        cpu_type: Option<u8>,
    },
    Unknown {
        selector: u8,
        data: Vec<u8>,
    },
}
impl AdditionalInfo {
    pub fn parse(frame: &Frame) -> Result<Self> {
        if frame.command != Command::ADDITIONAL_INFO {
            return Err(err("expected additional information"));
        }
        let p = frame.success_body()?;
        let (&selector, p) = p
            .split_first()
            .ok_or_else(|| err("missing additional info selector"))?;
        Ok(match selector {
            0 if p.len() >= 4 => Self::Voltage {
                voltage: i16::from_be_bytes([p[0], p[1]]),
                temperature: i16::from_be_bytes([p[2], p[3]]),
            },
            1 if p.len() >= 7 => Self::Color {
                print_attempts: u32::from_be_bytes(p[..4].try_into().unwrap()),
                battery_type: p[4],
                color: p[5],
                film_pi: p[6],
                cpu_type: p.get(7).copied(),
            },
            0 | 1 => return Err(err("truncated additional information")),
            _ => Self::Unknown {
                selector,
                data: p.to_vec(),
            },
        })
    }
}

/// SP-4 print head maker, from the `0010` selector 1 (light-pen) string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadType {
    Futaba,
    Tohoku,
}
impl HeadType {
    /// Mini `T1.n`: raw, untrimmed text starting with `AD`, or not 14 bytes long, is FUTABA.
    pub fn from_light_pen(raw: &[u8]) -> Self {
        if raw.starts_with(b"AD") || raw.len() != 14 {
            Self::Futaba
        } else {
            Self::Tohoku
        }
    }
}
/// Selectors the mini Link 0104 handlers accept; others fail `0x81` (external
/// analysis). Other models are not bounded
/// here: the Mini app reads `0001` selectors 9 and 10 from Link 3.
pub const MINI1_DEVICE_INFO_SELECTORS: std::ops::RangeInclusive<u8> = 0..=8;
/// `0002`: image, battery, printer status, history; no camera selectors.
pub const MINI1_INFO_SELECTORS: std::ops::RangeInclusive<u8> = 0..=3;
/// `0010`: 1 is the 10- or 14-byte light-pen record [`HeadType`] reads.
pub const MINI1_IDENTIFY_SELECTORS: std::ops::RangeInclusive<u8> = 0..=3;
pub fn identify(selector: u8) -> Frame {
    Frame::request(Command::IDENTIFY, vec![selector])
}
/// Reply body: `selector:u8, len:u8, text[len]`.
pub fn parse_identify(frame: &Frame) -> Result<(u8, Vec<u8>)> {
    if frame.command != Command::IDENTIFY {
        return Err(err("expected identification"));
    }
    let p = frame.success_body()?;
    if p.len() < 2 || p.len() < 2 + p[1] as usize {
        return Err(err("malformed identification"));
    }
    Ok((p[0], p[2..2 + p[1] as usize].to_vec()))
}
/// The app strips `[^0-9]` from the firmware revision and calls `Integer.parseInt`.
pub fn firmware_digits(revision: &str) -> Option<u32> {
    revision
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hex, unhex};
    #[test]
    fn head_identification_vectors() {
        assert_eq!(hex(&identify(1).encode().unwrap()), "4162000800100143");
        let reply = Frame::decode(
            &unhex("61 42 00 18 00 10 00 01 0e 41 44 31 32 33 34 35 36 37 38 39 30 31 32 30")
                .unwrap(),
        )
        .unwrap();
        let (selector, text) = parse_identify(&reply).unwrap();
        assert_eq!((selector, text.as_slice()), (1, &b"AD123456789012"[..]));
        assert_eq!(HeadType::from_light_pen(&text), HeadType::Futaba);
        assert_eq!(
            HeadType::from_light_pen(b"XY123456789012"),
            HeadType::Tohoku
        );
        assert_eq!(HeadType::from_light_pen(b"XY12345678901"), HeadType::Futaba);
        let short = Frame::response(Command::IDENTIFY, vec![0, 1, 5, b'A']);
        assert!(parse_identify(&short).is_err());
    }
    #[test]
    fn firmware_digits_match_parse_int() {
        assert_eq!(firmware_digits("01.03"), Some(103));
        assert_eq!(firmware_digits("V0100"), Some(100));
        assert_eq!(firmware_digits("simulator"), None);
    }
}
