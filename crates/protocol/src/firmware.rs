//! Recovered firmware formats and packets. This module does not initiate updates.
use crate::{err, Command, Frame, LinkModel, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub address: u32,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareImage {
    pub segments: Vec<Segment>,
    /// Intel HEX execution entry point is metadata, never a write address.
    pub entry_point: Option<u32>,
    /// DAT has no HEX marker; bundled Renesas HEX files omit the EOF record.
    pub eof_record: Option<bool>,
}
impl FirmwareImage {
    pub fn dat(bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > u32::MAX as usize {
            return Err(err("invalid DAT length"));
        }
        Ok(Self {
            segments: vec![Segment { address: 0, bytes }],
            entry_point: None,
            eof_record: None,
        })
    }
    /// Intel HEX 00..05 with checksum and address validation. Unlike the app,
    /// unknown records are rejected. Segments retain the file's record order.
    pub fn intel_hex(text: &str) -> Result<Self> {
        let mut base = 0u32;
        let mut entry_point = None;
        let mut segments = Vec::<Segment>::new();
        let mut eof = false;
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if eof {
                return Err(err("HEX record after EOF"));
            }
            let bytes = crate::unhex(
                line.strip_prefix(':')
                    .ok_or_else(|| err(format!("HEX line {} lacks colon", index + 1)))?,
            )?;
            if bytes.len() < 5
                || bytes.len() != bytes[0] as usize + 5
                || bytes.iter().fold(0u8, |s, b| s.wrapping_add(*b)) != 0
            {
                return Err(err(format!("invalid HEX record {}", index + 1)));
            }
            let offset = u16::from_be_bytes([bytes[1], bytes[2]]) as u32;
            let data = &bytes[4..bytes.len() - 1];
            match bytes[3] {
                0 => {
                    let address = base
                        .checked_add(offset)
                        .ok_or_else(|| err("HEX address overflow"))?;
                    address
                        .checked_add(data.len() as u32)
                        .ok_or_else(|| err("HEX span overflow"))?;
                    if !data.is_empty() {
                        segments.push(Segment {
                            address,
                            bytes: data.to_vec(),
                        });
                    }
                }
                1 if data.is_empty() && offset == 0 => eof = true,
                2 | 4 if data.len() == 2 && offset == 0 => {
                    base = (u16::from_be_bytes(data.try_into().unwrap()) as u32)
                        << if bytes[3] == 2 { 4 } else { 16 };
                }
                3 | 5 if data.len() == 4 && offset == 0 => {
                    let point = if bytes[3] == 5 {
                        u32::from_be_bytes(data.try_into().unwrap())
                    } else {
                        (u32::from(u16::from_be_bytes([data[0], data[1]])) << 4)
                            + u32::from(u16::from_be_bytes([data[2], data[3]]))
                    };
                    if entry_point.is_some_and(|old| old != point) {
                        return Err(err("conflicting HEX entry points"));
                    }
                    entry_point = Some(point);
                }
                _ => {
                    return Err(err(format!(
                        "unsupported or malformed HEX type {:02x}",
                        bytes[3]
                    )))
                }
            }
        }
        if segments.is_empty() {
            return Err(err("HEX lacks data"));
        }
        Ok(Self {
            segments,
            entry_point,
            eof_record: Some(eof),
        })
    }
    /// The vendor concatenates records without gap fill and uses the minimum
    /// address. Reproduce those facts for inspection, never silently for flashing.
    pub fn app_layout(&self) -> (u32, Vec<u8>) {
        let address = self.segments.iter().map(|s| s.address).min().unwrap_or(0);
        (
            address,
            self.segments
                .iter()
                .flat_map(|s| s.bytes.iter().copied())
                .collect(),
        )
    }
    pub fn contiguous_layout(&self) -> Result<(u32, Vec<u8>)> {
        if self.segments.is_empty() {
            return Err(err("empty firmware image"));
        }
        let (start, bytes) = self.app_layout();
        let mut next = start;
        for segment in &self.segments {
            if segment.address != next {
                return Err(err("HEX is sparse, overlapping or out of order, so concatenating it the way the app does would not give a contiguous image"));
            }
            next = next
                .checked_add(segment.bytes.len() as u32)
                .ok_or_else(|| err("firmware address overflow"))?;
        }
        Ok((start, bytes))
    }
}
/// `0100 [00]` abort. Mini `T1.i` and Wide `FirmwareUpdateTask` never send it from
/// the main-firmware path; Square's merged task `W1.c.h()` sends it on cancel or error.
pub fn main_abort(model: LinkModel) -> Option<Frame> {
    (model == LinkModel::Square).then(abort)
}
/// Link 3 BT updater (`U1.a` BT_ABORT_FW_UPGRADE): send on any error except a
/// lost connection.
pub fn bt_abort() -> Frame {
    abort()
}
fn abort() -> Frame {
    Frame::request(Command(0x01, 0x00), vec![0])
}
/// Main-controller START: length followed by start address.
pub fn main_start(length: u32, address: u32) -> Result<Frame> {
    if length == 0 {
        return Err(err("empty firmware"));
    }
    let mut p = length.to_be_bytes().to_vec();
    p.extend_from_slice(&address.to_be_bytes());
    Ok(Frame::request(Command(0x20, 0), p))
}
pub fn data(command: Command, sequence: u32, bytes: &[u8], block_size: usize) -> Result<Frame> {
    if !matches!(command, Command(0x20, 1) | Command(0x20, 0xa1))
        || block_size == 0
        || block_size > 65524
        || bytes.is_empty()
        || bytes.len() > block_size
    {
        return Err(err("invalid firmware DATA"));
    }
    let mut p = vec![0; 4 + block_size];
    p[..4].copy_from_slice(&sequence.to_be_bytes());
    p[4..4 + bytes.len()].copy_from_slice(bytes);
    Ok(Frame::request(command, p))
}
pub const BT_COMPONENT_IDS: [u16; 10] = [
    0x0100, 0x0200, 0x0300, 0x0400, 0x0410, 0x0500, 0x0602, 0x0700, 0x0800, 0x0a01,
];
pub fn bt_checksum(bytes: &[u8]) -> u32 {
    !bytes
        .iter()
        .fold(0u32, |sum, b| sum.wrapping_add(u32::from(*b)))
}
/// BT START: component id, length, complement of unsigned byte sum, reserved u16.
pub fn bt_start(id: u16, bytes: &[u8]) -> Result<Frame> {
    if !BT_COMPONENT_IDS.contains(&id) || bytes.is_empty() || bytes.len() > u32::MAX as usize {
        return Err(err("invalid Bluetooth firmware component"));
    }
    let mut p = id.to_be_bytes().to_vec();
    p.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    p.extend_from_slice(&bt_checksum(bytes).to_be_bytes());
    p.extend_from_slice(&[0, 0]);
    Ok(Frame::request(Command(0x20, 0xa0), p))
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAcknowledgement {
    pub program: u16,
    pub wait_seconds: u8,
}
impl UpdateAcknowledgement {
    pub fn parse(frame: &Frame) -> Result<Self> {
        if !matches!(frame.command, Command(0x20, 0x81) | Command(0x20, 0xb1)) {
            return Err(err("expected firmware update acknowledgement"));
        }
        let p = frame.success_body()?;
        if p.len() < 3 {
            return Err(err("truncated firmware update acknowledgement"));
        }
        Ok(Self {
            program: u16::from_be_bytes([p[0], p[1]]),
            wait_seconds: p[2],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abort_rules_and_frame() {
        assert_eq!(
            crate::hex(&bt_abort().encode().unwrap()),
            "4162000801000053"
        );
        assert_eq!(main_abort(LinkModel::Square), Some(bt_abort()));
        for model in [
            LinkModel::Mini,
            LinkModel::Mini3,
            LinkModel::Wide,
            LinkModel::Wide2,
        ] {
            assert_eq!(main_abort(model), None);
        }
    }
}
