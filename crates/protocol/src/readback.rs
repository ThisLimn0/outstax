//! Stored-image readback `1010`-`1012`. Layouts come from the external analysis
//! of mini Link firmware 0104 (2026-11-12).
//! All five analysed STM32 tables contain the opcodes. No app sends them.
//! Builders and parsers only: nothing here writes to the printer.
use crate::{err, Command, Frame, Result};

/// The upload format byte `1000` stores (JPEG).
pub const FORMAT_JPEG: u8 = 2;
/// After decoding, mini Link replaces the stored JPEG with [`DECODED_RGB_LEN`]
/// bytes: 800 rows stored bottom-up, each row 600 R, then 600 G, then 600 B.
pub const FORMAT_DECODED_RGB: u8 = 0x10;
pub const DECODED_RGB_LEN: u32 = 1_440_000;

/// Fails `0xb3` when no image is stored; resets the first/last markers.
pub fn readback_info() -> Frame {
    Frame::request(Command::READBACK_INFO, vec![])
}
pub fn readback_block(sequence: u32) -> Frame {
    Frame::request(Command::READBACK_DATA, sequence.to_be_bytes().to_vec())
}
/// Requires both markers, set by `1011` in the same if/else as upload DATA:
/// a one-block image needs sequence 0 read twice (static, untested).
pub fn readback_end() -> Frame {
    Frame::request(Command::READBACK_END, vec![])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadbackInfo {
    /// Not necessarily JPEG: see [`FORMAT_DECODED_RGB`].
    pub format: u8,
    pub flags: u8,
    pub size: u32,
    pub block_size: u32,
}
impl ReadbackInfo {
    /// Body: `format:u8, flags:u8, reserved:u16 = 0, size:u32, block_size:u32`.
    pub fn parse(frame: &Frame) -> Result<Self> {
        if frame.command != Command::READBACK_INFO {
            return Err(err("expected readback information"));
        }
        let p = frame.success_body()?;
        if p.len() < 12 || p[2..4] != [0, 0] {
            return Err(err("readback information requires twelve bytes"));
        }
        let info = Self {
            format: p[0],
            flags: p[1],
            size: u32::from_be_bytes(p[4..8].try_into().unwrap()),
            block_size: u32::from_be_bytes(p[8..12].try_into().unwrap()),
        };
        if info.format == 0
            || info.size == 0
            || info.block_size == 0
            || info.block_size > u16::MAX as u32 - 12
        {
            return Err(err("invalid readback information"));
        }
        Ok(info)
    }
    pub fn blocks(&self) -> u32 {
        self.size.div_ceil(self.block_size)
    }
}

/// Collects `1011` replies with a caller-chosen allocation bound. Tracks every
/// block: the firmware's first/last markers do not prove the middle arrived.
pub struct Readback {
    pub info: ReadbackInfo,
    data: Vec<u8>,
    seen: Vec<bool>,
}
impl Readback {
    pub fn new(info: ReadbackInfo, max_bytes: usize) -> Result<Self> {
        let size = info.size as usize;
        if size > max_bytes {
            return Err(err(format!(
                "readback of {size} bytes exceeds the {max_bytes}-byte limit"
            )));
        }
        Ok(Self {
            data: vec![0; size],
            seen: vec![false; info.blocks() as usize],
            info,
        })
    }
    /// Reply body: `sequence:u32, data[block_size]`, final block zero-padded.
    /// A repeated block must match the earlier copy.
    pub fn ingest(&mut self, frame: &Frame, sequence: u32) -> Result<()> {
        if frame.command != Command::READBACK_DATA {
            return Err(err("expected readback data"));
        }
        let p = frame.success_body()?;
        let block = self.info.block_size as usize;
        if p.len() != 4 + block {
            return Err(err("readback block has the wrong length"));
        }
        if p[..4] != sequence.to_be_bytes() {
            return Err(err("wrong readback sequence"));
        }
        let index = sequence as usize;
        if index >= self.seen.len() {
            return Err(err("readback sequence beyond the final block"));
        }
        let start = index * block;
        let count = block.min(self.data.len() - start);
        let (bytes, padding) = p[4..].split_at(count);
        if padding.iter().any(|b| *b != 0) {
            return Err(err("nonzero readback padding"));
        }
        let target = &mut self.data[start..start + count];
        if self.seen[index] && target != bytes {
            return Err(err("repeated readback block differs"));
        }
        target.copy_from_slice(bytes);
        self.seen[index] = true;
        Ok(())
    }
    pub fn missing(&self) -> Option<u32> {
        self.seen.iter().position(|s| !s).map(|i| i as u32)
    }
    pub fn finish(self) -> Result<Vec<u8>> {
        match self.missing() {
            Some(i) => Err(err(format!("readback block {i} missing"))),
            None => Ok(self.data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;
    fn data(sequence: u32, bytes: &[u8]) -> Frame {
        let mut p = vec![0];
        p.extend_from_slice(&sequence.to_be_bytes());
        p.extend_from_slice(bytes);
        p.resize(5 + 900, 0);
        Frame::response(Command::READBACK_DATA, p)
    }
    #[test]
    fn request_frames() {
        assert_eq!(hex(&readback_info().encode().unwrap()), "41620007101035");
        assert_eq!(
            hex(&readback_block(0x12345678).encode().unwrap()),
            "4162000b1011123456781c"
        );
        assert_eq!(readback_end().encode().unwrap().len(), 7);
    }
    #[test]
    fn info_layout_and_rejections() {
        let reply = |p: Vec<u8>| Frame::response(Command::READBACK_INFO, p);
        let info =
            ReadbackInfo::parse(&reply(vec![0, 2, 0, 0, 0, 0, 0, 3, 0x85, 0, 0, 3, 0x84])).unwrap();
        assert_eq!(
            info,
            ReadbackInfo {
                format: FORMAT_JPEG,
                flags: 0,
                size: 901,
                block_size: 900
            }
        );
        assert_eq!(info.blocks(), 2);
        assert!(ReadbackInfo::parse(&reply(vec![0xb3])).is_err());
        assert!(
            ReadbackInfo::parse(&reply(vec![0, 2, 0, 0, 1, 0, 0, 3, 0x85, 0, 0, 3, 0x84])).is_err()
        );
        assert!(
            ReadbackInfo::parse(&reply(vec![0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0x84])).is_err()
        );
    }
    #[test]
    fn collection_bounds_duplicates_and_padding() {
        let info = ReadbackInfo {
            format: FORMAT_JPEG,
            flags: 0,
            size: 901,
            block_size: 900,
        };
        assert!(Readback::new(info.clone(), 900).is_err());
        let mut r = Readback::new(info.clone(), 153_600).unwrap();
        let first = data(0, &[0x35; 900]);
        let last = data(1, &[0x77]);
        assert!(r.ingest(&last, 0).is_err());
        assert!(r.ingest(&data(2, &[]), 2).is_err());
        r.ingest(&last, 1).unwrap();
        assert_eq!(r.missing(), Some(0));
        r.ingest(&first, 0).unwrap();
        // The one-block marker workaround re-reads a block; identical repeats pass.
        r.ingest(&first, 0).unwrap();
        let mut conflict = first.clone();
        conflict.payload[5] = 0x36;
        assert!(r.ingest(&conflict, 0).is_err());
        let mut tail = last.clone();
        tail.payload[904] = 1;
        assert!(r.ingest(&tail, 1).is_err());
        assert!(r
            .ingest(&Frame::response(Command::READBACK_DATA, vec![0x82]), 1)
            .is_err());
        let image = r.finish().unwrap();
        assert_eq!(image.len(), 901);
        assert!(image[..900].iter().all(|b| *b == 0x35));
        assert_eq!(image[900], 0x77);
        assert!(Readback::new(info, 153_600).unwrap().finish().is_err());
    }
}
