//! Structural JPEG checks matching the mini Link 0104 decoder wrapper (external
//! analysis, 2026-11-12, docs/re/firmware/mini-link-0104.md). Headers only: no
//! entropy decoding. Other models' decoders are unanalysed.
use crate::{err, Result};

/// The wrapper's size table: 608x800 is centre-cropped to 600 columns, 480x640
/// is scaled 5/4. Anything else is rejected.
pub const MINI1_DIMENSIONS: [(u16, u16); 3] = [(600, 800), (608, 800), (480, 640)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JpegHeader {
    pub width: u16,
    pub height: u16,
    pub precision: u8,
    /// Start-of-frame marker: 0xc0 baseline, 0xc2 progressive, and so on.
    pub sof: u8,
    pub component_ids: Vec<u8>,
    pub jfif: bool,
    pub adobe_transform: Option<u8>,
}
impl JpegHeader {
    /// Walks the segments up to the first scan. Like the firmware, requires SOI
    /// at the start and EOI as exactly the last two bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 4 || !data.starts_with(&[0xff, 0xd8]) || !data.ends_with(&[0xff, 0xd9]) {
            return Err(err("JPEG must start with SOI and end with EOI"));
        }
        let (mut at, mut frame, mut jfif, mut adobe) = (2, None::<Self>, false, None);
        loop {
            if data.get(at) != Some(&0xff) {
                return Err(err("expected a JPEG marker"));
            }
            while data.get(at) == Some(&0xff) {
                at += 1;
            }
            let marker = *data.get(at).ok_or_else(|| err("truncated JPEG marker"))?;
            at += 1;
            match marker {
                0x01 => continue,
                0x00 | 0xd0..=0xd9 => return Err(err("unexpected JPEG marker before scan")),
                _ => {}
            }
            let len = data
                .get(at..at + 2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
                .filter(|n| *n >= 2)
                .ok_or_else(|| err("invalid JPEG segment length"))?;
            let seg = data
                .get(at + 2..at + len)
                .ok_or_else(|| err("truncated JPEG segment"))?;
            match marker {
                0xe0 if seg.starts_with(b"JFIF\0") => jfif = true,
                0xee if seg.starts_with(b"Adobe") => {
                    adobe = Some(*seg.get(11).ok_or_else(|| err("truncated Adobe segment"))?)
                }
                0xc4 | 0xc8 | 0xcc => {}
                0xc0..=0xcf => {
                    let n = *seg.get(5).unwrap_or(&0) as usize;
                    if frame.is_some() || n == 0 || seg.len() != 6 + 3 * n {
                        return Err(err("invalid or repeated JPEG frame header"));
                    }
                    let height = u16::from_be_bytes([seg[1], seg[2]]);
                    let width = u16::from_be_bytes([seg[3], seg[4]]);
                    if width == 0 || height == 0 {
                        return Err(err("JPEG frame header has a zero dimension"));
                    }
                    frame = Some(Self {
                        width,
                        height,
                        precision: seg[0],
                        sof: marker,
                        component_ids: (0..n).map(|i| seg[6 + 3 * i]).collect(),
                        jfif: false,
                        adobe_transform: None,
                    });
                }
                0xda => {
                    let mut header = frame.ok_or_else(|| err("JPEG scan before frame header"))?;
                    let n = *seg.first().unwrap_or(&0) as usize;
                    if n == 0 || seg.len() != 4 + 2 * n {
                        return Err(err("invalid JPEG scan header"));
                    }
                    let ids: Vec<u8> = (0..n).map(|i| seg[1 + 2 * i]).collect();
                    if ids
                        .iter()
                        .enumerate()
                        .any(|(i, id)| !header.component_ids.contains(id) || ids[..i].contains(id))
                    {
                        return Err(err("JPEG scan names an unknown or repeated component"));
                    }
                    header.jfif = jfif;
                    header.adobe_transform = adobe;
                    return Ok(header);
                }
                _ => {}
            }
            at += len;
        }
    }
    /// The firmware requires libjpeg colour space 3 (YCbCr). IDs 1,2,3 without
    /// an Adobe "RGB" transform infer YCbCr under both libjpeg orderings
    /// (JFIF/Adobe first in 6b, component IDs first in 9).
    pub fn is_ycbcr(&self) -> bool {
        self.component_ids == [1, 2, 3] && self.adobe_transform != Some(0)
    }
}

/// Firmware checks (boundary markers, three-component YCbCr, size table) plus
/// one client policy: baseline 8-bit, since progressive acceptance is unknown.
/// The 153 600-byte limit comes from `0002/0` ([`crate::ImageSupport`]).
pub fn check_mini1(data: &[u8]) -> Result<JpegHeader> {
    let header = JpegHeader::parse(data)?;
    if !MINI1_DIMENSIONS.contains(&(header.width, header.height)) {
        return Err(err(format!(
            "mini Link 0104 does not accept {}x{} JPEG",
            header.width, header.height
        )));
    }
    if !header.is_ycbcr() {
        return Err(err("mini Link 0104 requires three-component YCbCr JPEG"));
    }
    if header.sof != 0xc0 || header.precision != 8 {
        return Err(err("only baseline 8-bit JPEG is known to decode"));
    }
    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Header-only fixture: JFIF APP0, SOF, SOS, then EOI with no scan data.
    fn header(sof: u8, w: u16, h: u16, ids: &[u8]) -> Vec<u8> {
        let mut j = vec![0xff, 0xd8, 0xff, 0xe0, 0, 16];
        j.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        let n = ids.len() as u16;
        j.extend_from_slice(&[0xff, sof]);
        j.extend_from_slice(&(8 + 3 * n).to_be_bytes());
        j.push(8);
        j.extend_from_slice(&h.to_be_bytes());
        j.extend_from_slice(&w.to_be_bytes());
        j.push(ids.len() as u8);
        for id in ids {
            j.extend_from_slice(&[*id, 0x11, 0]);
        }
        j.extend_from_slice(&[0xff, 0xda]);
        j.extend_from_slice(&(6 + 2 * n).to_be_bytes());
        j.push(ids.len() as u8);
        for id in ids {
            j.extend_from_slice(&[*id, 0]);
        }
        j.extend_from_slice(&[0, 63, 0, 0xff, 0xd9]);
        j
    }
    #[test]
    fn firmware_dimension_table() {
        for (w, h) in MINI1_DIMENSIONS {
            let parsed = check_mini1(&header(0xc0, w, h, &[1, 2, 3])).unwrap();
            assert_eq!((parsed.width, parsed.height, parsed.jfif), (w, h, true));
        }
        assert!(check_mini1(&header(0xc0, 800, 600, &[1, 2, 3])).is_err());
        assert!(check_mini1(&header(0xc0, 800, 800, &[1, 2, 3])).is_err());
    }
    #[test]
    fn colour_space_and_coding_policy() {
        assert!(check_mini1(&header(0xc0, 600, 800, &[1])).is_err());
        assert!(check_mini1(&header(0xc0, 600, 800, b"RGB")).is_err());
        assert!(check_mini1(&header(0xc2, 600, 800, &[1, 2, 3])).is_err());
        // APP14 transform 0 (RGB) after the frame header still counts.
        let plain = header(0xc0, 600, 800, &[1, 2, 3]);
        let sos = plain.len() - 16;
        assert_eq!(plain[sos..sos + 2], [0xff, 0xda]);
        let mut adobe = plain[..sos].to_vec();
        adobe.extend_from_slice(b"\xff\xee\x00\x0eAdobe\x00\x64\x00\x00\x00\x00\x00");
        adobe.extend_from_slice(&plain[sos..]);
        assert_eq!(JpegHeader::parse(&adobe).unwrap().adobe_transform, Some(0));
        assert!(check_mini1(&adobe).is_err());
    }
    #[test]
    fn boundary_markers_and_structure() {
        let good = header(0xc0, 600, 800, &[1, 2, 3]);
        let mut trailing = good.clone();
        trailing.push(0);
        assert!(JpegHeader::parse(&trailing).is_err());
        let mut no_frame = good.clone();
        no_frame[21] = 0xc4; // the SOF becomes a DHT, so the scan comes first
        assert!(JpegHeader::parse(&no_frame).is_err());
        let mut bad_length = good.clone();
        bad_length[4..6].copy_from_slice(&[0xff, 0xff]);
        assert!(JpegHeader::parse(&bad_length).is_err());
        let mut scan_id = good;
        let at = scan_id.len() - 11;
        scan_id[at] = 9;
        assert!(JpegHeader::parse(&scan_id).is_err());
        assert!(JpegHeader::parse(&[0xff, 0xd8, 0xff, 0xd9]).is_err());
    }
}
