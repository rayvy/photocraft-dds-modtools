//! Checked 2D BC4–BC7 blocks. BC5 is RG data, never luminance+alpha.
use crate::options::alloc_zeroed;
use crate::{ChannelLayout, CodecError, DdsCompression as C, Format, Image};

pub(super) fn decode(c: C, data: &[u8], width: u32, height: u32, row_pitch: usize) -> Result<Image, CodecError> {
    let err = || CodecError::malformed(Format::Dds, "invalid advanced DDS payload");
    let (w, h) = (width as usize, height as usize);
    let channels = match c {
        C::Bc4 | C::Bc4Snorm => 1,
        C::Bc5 | C::Bc5Snorm | C::Bc6h | C::Bc6hSigned => 3,
        _ => 4,
    };
    let layout = match channels {
        1 => ChannelLayout::Gray,
        3 => ChannelLayout::Rgb,
        _ => ChannelLayout::Rgba,
    };
    let count = w.checked_mul(h).and_then(|n| n.checked_mul(channels)).ok_or_else(err)?;
    let float = c.is_float();
    let mut pixels = if float { alloc_zeroed::<f32>(count)? } else { Vec::new() };
    let mut integer = if float { Vec::new() } else { alloc_zeroed::<u8>(count)? };
    if matches!(c, C::RgbaFloat | C::RgbaLinear | C::RgbaSrgb) {
        let unit = if float { 16 } else { 4 };
        for (y, row) in data.chunks_exact(row_pitch).take(h).enumerate() {
            for (x, p) in row.chunks_exact(unit).take(w).enumerate() {
                for k in 0..4 {
                    if float {
                        let bytes: [u8; 4] = p.get(k * 4..k * 4 + 4).and_then(|b| b.try_into().ok()).ok_or_else(err)?;
                        pixels[(y * w + x) * 4 + k] = f32::from_le_bytes(bytes);
                    } else {
                        integer[(y * w + x) * 4 + k] = p[k];
                    }
                }
            }
        }
    } else {
        let unit = if matches!(c, C::Bc4 | C::Bc4Snorm | C::Bc1Srgb | C::Bc1Linear) { 8 } else { 16 };
        let blocks_x = w.div_ceil(4);
        if blocks_x == 0 {
            return Err(err());
        }
        for (i, block) in data.chunks_exact(unit).enumerate() {
            let mut values = [0.0f32; 64];
            let mut bytes = [0u8; 64];
            match c {
                C::Bc4 | C::Bc4Snorm => bcdec_rs::bc4_float(block, &mut values, 4, c == C::Bc4Snorm),
                C::Bc5 | C::Bc5Snorm => {
                    let mut rg = [0.0f32; 32];
                    bcdec_rs::bc5_float(block, &mut rg, 8, c == C::Bc5Snorm);
                    for p in 0..16 {
                        values[p * 3..p * 3 + 2].copy_from_slice(&rg[p * 2..p * 2 + 2]);
                    }
                }
                C::Bc6h | C::Bc6hSigned => bcdec_rs::bc6h_float(block, &mut values, 12, c == C::Bc6hSigned),
                C::Bc1Srgb | C::Bc1Linear => bcdec_rs::bc1(block, &mut bytes, 16),
                C::Bc2Srgb | C::Bc2Linear => bcdec_rs::bc2(block, &mut bytes, 16),
                C::Bc3Srgb | C::Bc3Linear => bcdec_rs::bc3(block, &mut bytes, 16),
                C::Bc7 | C::Bc7Srgb => bcdec_rs::bc7(block, &mut bytes, 16),
                _ => return Err(err()),
            }
            if matches!(c, C::Bc1Srgb | C::Bc2Srgb | C::Bc3Srgb | C::Bc7 | C::Bc7Srgb | C::Bc1Linear | C::Bc2Linear | C::Bc3Linear) {
                for (v, b) in values.iter_mut().zip(bytes) {
                    *v = f32::from(b) / 255.0;
                }
            }
            let (bx, by) = (i % blocks_x * 4, i / blocks_x * 4);
            for p in 0..16 {
                let (x, y) = (bx + p % 4, by + p / 4);
                if x < w && y < h {
                    let at = (y * w + x) * channels;
                    if float {
                        pixels[at..at + channels].copy_from_slice(&values[p * channels..(p + 1) * channels]);
                    } else {
                        for k in 0..channels {
                            integer[at + k] = (values[p * channels + k] * 255.0).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
    }
    if float { Image::from_f32(width, height, layout, &pixels) } else { Image::from_u8(width, height, layout, integer) }
}
