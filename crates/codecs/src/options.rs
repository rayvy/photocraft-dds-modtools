//! Decode limits and encode options.

use crate::error::CodecError;
use crate::image::{ChannelLayout, SampleType};

/// Decompression-bomb guards. Dimension checks precede pixel allocation;
/// metadata decoding checks its budget before growing decoded output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
    /// Maximum `width * height`.
    pub max_pixels: u64,
    /// Maximum bytes for the decoded pixel buffer (and passed on as the
    /// allocation budget to underlying decoders).
    /// PNG also applies this to the aggregate decoded text/XMP UTF-8 bytes,
    /// including keywords, separately from pixels. Animated WebP charges
    /// every retained frame's full pixel copy plus its own copy of the
    /// file's ICC profile against this budget. This is not a total RSS cap.
    pub max_alloc: u64,
}

impl Default for Limits {
    fn default() -> Self {
        // 2^30 pixels (e.g. 32768²) and, on 64-bit targets, 8 GiB: enough for the large
        // documents Photoshop users open (a 20000² RGBA 16-bit image is 3.2 GB). On 32-bit
        // targets (wasm32 has a 4 GiB address space shared with the document, history and
        // the decoder's own buffers) 1 GiB is the most one buffer can safely ask for.
        let max_alloc = if cfg!(target_pointer_width = "64") { 8 << 30 } else { 1 << 30 };
        Limits { max_width: 1 << 18, max_height: 1 << 18, max_pixels: 1 << 30, max_alloc }
    }
}

impl Limits {
    /// Effectively unlimited (still bounded by `usize` overflow checks).
    pub fn none() -> Self {
        Limits { max_width: u32::MAX, max_height: u32::MAX, max_pixels: u64::MAX, max_alloc: u64::MAX }
    }

    /// Validate declared dimensions for an output buffer of the given layout
    /// and sample type.
    pub fn check(&self, width: u32, height: u32, layout: ChannelLayout, sample: SampleType) -> Result<(), CodecError> {
        self.check_bytes(width, height, (layout.channels() * sample.bytes()) as u64)
    }

    pub(crate) fn check_bytes(&self, width: u32, height: u32, bytes_per_pixel: u64) -> Result<(), CodecError> {
        if width == 0 || height == 0 {
            return Err(CodecError::InvalidImage(format!("zero-sized image {width}x{height}")));
        }
        if width > self.max_width || height > self.max_height {
            return Err(CodecError::LimitExceeded(format!("dimensions {width}x{height} exceed max {}x{}", self.max_width, self.max_height)));
        }
        let pixels = width as u64 * height as u64;
        if pixels > self.max_pixels {
            return Err(CodecError::LimitExceeded(format!("{pixels} pixels exceed max {}", self.max_pixels)));
        }
        let bytes = pixels.saturating_mul(bytes_per_pixel);
        if bytes > self.max_alloc {
            return Err(CodecError::LimitExceeded(format!("{bytes} bytes exceed max_alloc {}", self.max_alloc)));
        }
        if usize::try_from(bytes).is_err() {
            return Err(CodecError::LimitExceeded(format!("{bytes} bytes do not fit in memory")));
        }
        Ok(())
    }

    /// Decompression-bomb guard for formats that compress: refuse a file too small to hold
    /// `declared_bytes` of (stored, pre-expansion) image data, given that the compression
    /// never beats `max_ratio`:1. Called before the output buffer is allocated, so a
    /// header-only hostile file cannot make the decoder reserve gigabytes. A fixed 1 MiB of
    /// slack keeps tiny images and headers with large metadata unaffected.
    pub(crate) fn check_plausible(declared_bytes: u64, file_len: usize, max_ratio: u64) -> Result<(), CodecError> {
        let capacity = (file_len as u64).saturating_mul(max_ratio).saturating_add(1 << 20);
        if declared_bytes > capacity {
            return Err(CodecError::LimitExceeded(format!("a {file_len} byte file cannot hold {declared_bytes} bytes of image data")));
        }
        Ok(())
    }

    pub(crate) fn alloc_usize(&self) -> usize {
        usize::try_from(self.max_alloc).unwrap_or(usize::MAX)
    }
}

/// Options for [`crate::decode_with`].
#[derive(Debug, Clone, Default)]
pub struct DecodeOptions {
    pub limits: Limits,
    /// Keep the pixels as stored instead of applying the EXIF / TIFF
    /// orientation (by default they are turned upright and the metadata's
    /// orientation is rewritten to 1).
    pub keep_orientation: bool,
}

/// PNG zlib effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PngCompression {
    None,
    Fast,
    #[default]
    Default,
    Best,
}

/// How a PNG's `sTER` chunk pairs its two views (PNG Extensions 1.4.0, §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngStereo {
    /// Side by side with the right-eye view first (cross-eyed viewing, mode 0).
    CrossFuse,
    /// Side by side with the left-eye view first (parallel viewing, mode 1).
    DivergingFuse,
}

impl PngStereo {
    /// The view for an `sTER` mode byte; reserved and unknown values are `None`.
    pub fn from_mode(mode: u8) -> Option<Self> {
        match mode {
            0 => Some(Self::CrossFuse),
            1 => Some(Self::DivergingFuse),
            _ => None,
        }
    }

    /// The `sTER` mode byte.
    pub fn mode(self) -> u8 {
        match self {
            Self::CrossFuse => 0,
            Self::DivergingFuse => 1,
        }
    }
}

/// TIFF strip compression. The default is none, as in Photoshop: uncompressed strips are the
/// only kind every TIFF reader opens (Deflate and the predictor are not baseline TIFF, #4484).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TiffCompression {
    #[default]
    None,
    Lzw,
    Deflate,
    PackBits,
}

/// JPEG scan layout: Photoshop's JPEG Options › Format Options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JpegFormat {
    /// Baseline ("Standard"): one sequential scan with the standard Huffman tables.
    #[default]
    Baseline,
    /// Baseline Optimized: one sequential scan with Huffman tables fitted to the image. Only at
    /// 4:4:4 for now: a 4:2:0 (subsampled RGB) baseline scan keeps the standard tables, because
    /// fitted tables make jpeg-encoder write one scan per component, which zune-jpeg 0.5.15 (our
    /// decoder) reads back wrongly when chroma is subsampled (see `web::encode_jpeg_rgb8`).
    Optimized,
    /// Progressive: the image refines over several scans as it loads.
    Progressive,
}

impl JpegFormat {
    /// `"baseline"`, `"optimized"` or `"progressive"`, as parameters and scripts spell it.
    pub fn name(self) -> &'static str {
        match self {
            JpegFormat::Baseline => "baseline",
            JpegFormat::Optimized => "optimized",
            JpegFormat::Progressive => "progressive",
        }
    }

    /// The inverse of [`Self::name`].
    pub fn parse(s: &str) -> Option<Self> {
        [JpegFormat::Baseline, JpegFormat::Optimized, JpegFormat::Progressive].into_iter().find(|f| f.name() == s)
    }
}

/// OpenEXR compression (lossless methods only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExrCompression {
    None,
    Rle,
    Zip1,
    #[default]
    Zip16,
    Piz,
}

/// Bits per sample of a DPX file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpxBits {
    Bits8,
    Bits10,
    Bits12,
    Bits16,
}

impl DpxBits {
    /// `8`, `10`, `12` or `16` as [`DpxBits`]; any other number is `None`.
    pub fn from_bits(bits: u64) -> Option<Self> {
        match bits {
            8 => Some(DpxBits::Bits8),
            10 => Some(DpxBits::Bits10),
            12 => Some(DpxBits::Bits12),
            16 => Some(DpxBits::Bits16),
            _ => None,
        }
    }

    /// The bits per sample.
    pub fn bits(self) -> u8 {
        match self {
            DpxBits::Bits8 => 8,
            DpxBits::Bits10 => 10,
            DpxBits::Bits12 => 12,
            DpxBits::Bits16 => 16,
        }
    }
}

/// DDS storage. Signed BC4/5 retain numeric data; BC6H retains HDR floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DdsCompression {
    #[default]
    None,
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc4Snorm,
    Bc5,
    Bc5Snorm,
    Bc6h,
    Bc6hSigned,
    Bc7,
    Bc1Srgb,
    Bc2Srgb,
    Bc3Srgb,
    Bc7Srgb,
    RgbaLinear,
    RgbaSrgb,
    RgbaFloat,
    Bc1Linear,
    Bc2Linear,
    Bc3Linear,
}

impl DdsCompression {
    pub const ALL: [Self; 21] = [
        Self::None,
        Self::Bc1,
        Self::Bc2,
        Self::Bc3,
        Self::Bc4,
        Self::Bc4Snorm,
        Self::Bc5,
        Self::Bc5Snorm,
        Self::Bc6h,
        Self::Bc6hSigned,
        Self::Bc7,
        Self::Bc1Srgb,
        Self::Bc2Srgb,
        Self::Bc3Srgb,
        Self::Bc7Srgb,
        Self::RgbaLinear,
        Self::RgbaSrgb,
        Self::RgbaFloat,
        Self::Bc1Linear,
        Self::Bc2Linear,
        Self::Bc3Linear,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bc1 => "bc1",
            Self::Bc2 => "bc2",
            Self::Bc3 => "bc3",
            Self::Bc4 => "bc4",
            Self::Bc4Snorm => "bc4-snorm",
            Self::Bc5 => "bc5",
            Self::Bc5Snorm => "bc5-snorm",
            Self::Bc6h => "bc6h",
            Self::Bc6hSigned => "bc6h-signed",
            Self::Bc7 => "bc7",
            Self::Bc1Srgb => "bc1-srgb",
            Self::Bc2Srgb => "bc2-srgb",
            Self::Bc3Srgb => "bc3-srgb",
            Self::Bc7Srgb => "bc7-srgb",
            Self::RgbaLinear => "rgba-linear",
            Self::RgbaSrgb => "rgba-srgb",
            Self::RgbaFloat => "rgba-float",
            Self::Bc1Linear => "bc1-linear",
            Self::Bc2Linear => "bc2-linear",
            Self::Bc3Linear => "bc3-linear",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.to_ascii_lowercase();
        match name.as_str() {
            "dxt1" => Some(Self::Bc1),
            "dxt3" => Some(Self::Bc2),
            "dxt5" => Some(Self::Bc3),
            _ => Self::ALL.into_iter().find(|c| c.name() == name),
        }
    }
    /// Equivalent DXGI format, or zero for legacy uncompressed storage.
    pub fn dxgi(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Bc1 => 71,
            Self::Bc2 => 74,
            Self::Bc3 => 77,
            Self::Bc4 => 80,
            Self::Bc4Snorm => 81,
            Self::Bc5 => 83,
            Self::Bc5Snorm => 84,
            Self::Bc6h => 95,
            Self::Bc6hSigned => 96,
            Self::Bc7 => 98,
            Self::Bc1Srgb => 72,
            Self::Bc2Srgb => 75,
            Self::Bc3Srgb => 78,
            Self::Bc7Srgb => 99,
            Self::RgbaLinear => 28,
            Self::RgbaSrgb => 29,
            Self::RgbaFloat => 2,
            Self::Bc1Linear => 71,
            Self::Bc2Linear => 74,
            Self::Bc3Linear => 77,
        }
    }
    pub fn from_dxgi(dxgi: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|c| !matches!(c, Self::Bc1 | Self::Bc2 | Self::Bc3) && c.dxgi() == dxgi)
    }
    /// Stable document identity: legacy FourCC choices remain distinct from explicit DXGI.
    pub fn storage_id(self) -> u32 {
        match self {
            Self::Bc1 => u32::from_le_bytes(*b"DXT1"),
            Self::Bc2 => u32::from_le_bytes(*b"DXT3"),
            Self::Bc3 => u32::from_le_bytes(*b"DXT5"),
            _ => self.dxgi(),
        }
    }
    pub fn from_storage_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.storage_id() == id)
    }
    pub fn is_block(self) -> bool {
        !matches!(self, Self::None | Self::RgbaLinear | Self::RgbaSrgb | Self::RgbaFloat)
    }
    pub fn is_data(self) -> bool {
        matches!(self, Self::Bc4 | Self::Bc4Snorm | Self::Bc5 | Self::Bc5Snorm)
    }
    pub fn is_float(self) -> bool {
        matches!(self, Self::Bc4Snorm | Self::Bc5Snorm | Self::Bc6h | Self::Bc6hSigned | Self::RgbaFloat)
    }
    pub fn is_srgb(self) -> bool {
        matches!(self, Self::None | Self::Bc1 | Self::Bc2 | Self::Bc3 | Self::Bc1Srgb | Self::Bc2Srgb | Self::Bc3Srgb | Self::Bc7Srgb | Self::RgbaSrgb)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Uncompressed (legacy)",
            Self::Bc1 => "BC1 / DXT1",
            Self::Bc2 => "BC2 / DXT3",
            Self::Bc3 => "BC3 / DXT5",
            Self::Bc4 => "BC4 — R data",
            Self::Bc4Snorm => "BC4 SNORM — signed R",
            Self::Bc5 => "BC5 — RG data",
            Self::Bc5Snorm => "BC5 SNORM — signed RG",
            Self::Bc6h => "BC6H — HDR RGB",
            Self::Bc6hSigned => "BC6H — signed HDR RGB",
            Self::Bc7 => "BC7 — RGBA linear",
            Self::Bc1Srgb => "BC1 — sRGB",
            Self::Bc2Srgb => "BC2 — sRGB",
            Self::Bc3Srgb => "BC3 — sRGB",
            Self::Bc7Srgb => "BC7 — RGBA sRGB",
            Self::RgbaLinear => "RGBA8 — linear",
            Self::RgbaSrgb => "RGBA8 — sRGB",
            Self::RgbaFloat => "RGBA32F — lossless HDR",
            Self::Bc1Linear => "BC1 — linear (DX10)",
            Self::Bc2Linear => "BC2 — linear (DX10)",
            Self::Bc3Linear => "BC3 — linear (DX10)",
        }
    }
}

/// What a DDS file is written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DdsOptions {
    pub compression: DdsCompression,
    /// Write the full mip chain down to 1×1, each level a box filter of the one above.
    pub mipmaps: bool,
}

/// How a DDS texture opens.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DdsImport {
    /// Where the texture's alpha goes: the layer's transparency (the default), or an "Alpha 1"
    /// document channel preserving it as editable data — game textures often carry data such
    /// as specular maps in alpha rather than transparency (the NVIDIA/Intel plug-ins' "load
    /// transparency as alpha channel").
    pub alpha_as_channel: bool,
    /// A layer per mip level (the Intel Texture Works plugin's choice), not the top level only.
    pub mips_as_layers: bool,
    /// Flip rows vertically on open (the GIMP plug-in's "load flipped" toggle for textures
    /// written for the opposite GL/D3D origin convention).
    pub flip: bool,
}

/// What [`crate::dds_info`] reports about a DDS texture, from its header alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DdsInfo {
    /// Explicit linear transfer in the DX10 header (legacy files remain untagged).
    pub linear: bool,
    /// The top mip level's size.
    pub width: u32,
    pub height: u32,
    /// The declared mip count (1 = the top level only).
    pub levels: u32,
    /// The encoding carries an alpha channel.
    pub alpha: bool,
    /// The compression a save could write back; `None` when the encoding has no writable
    /// form.
    pub compression: Option<DdsCompression>,
}

/// Options for [`crate::encode`].
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// JPEG quality 1..=100 (also AVIF quality when enabled).
    pub jpeg_quality: u8,
    /// JPEG 4:2:0 chroma subsampling (`false` = 4:4:4).
    pub jpeg_chroma_subsampling: bool,
    /// JPEG scan layout (baseline, baseline optimized or progressive).
    pub jpeg_format: JpegFormat,
    pub png_compression: PngCompression,
    /// Write Adam7-interlaced PNG.
    pub png_interlaced: bool,
    /// Write an `sTER` chunk marking the image as a stereo pair (PNG Extensions 1.4.0, §3.6),
    /// so a stereo PNG keeps its mark when saved again (#6063).
    pub png_stereo: Option<PngStereo>,
    /// WebP: lossless (VP8L, via `image-webp`) or lossy (our VP8 encoder, see
    /// `codecs::vp8`) with [`Self::webp_quality`].
    pub webp_lossless: bool,
    /// Lossy WebP quality 0..=100 (the source application's scale; 75–85 is typical for photos).
    pub webp_quality: u8,
    pub tiff_compression: TiffCompression,
    /// Always write TIFF as BigTIFF (8-byte offsets). Without it a TIFF is written as BigTIFF
    /// only when it could pass the 4 GiB a classic TIFF can address.
    pub tiff_bigtiff: bool,
    pub exr_compression: ExrCompression,
    /// DPX bits per sample. `None` follows the image: 8 for 8-bit, 16 otherwise.
    pub dpx_bits: Option<DpxBits>,
    /// DDS compression and mip chain.
    pub dds: DdsOptions,
    /// Embed the ICC profile when the format supports it.
    pub embed_icc: bool,
    /// Embed EXIF/XMP/DPI/text when the format supports it.
    pub embed_metadata: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        EncodeOptions {
            jpeg_quality: 90,
            jpeg_chroma_subsampling: true,
            jpeg_format: JpegFormat::Baseline,
            png_compression: PngCompression::Default,
            png_interlaced: false,
            png_stereo: None,
            webp_lossless: true,
            webp_quality: 80,
            tiff_compression: TiffCompression::None,
            tiff_bigtiff: false,
            exr_compression: ExrCompression::Zip16,
            dpx_bits: None,
            dds: DdsOptions::default(),
            embed_icc: true,
            embed_metadata: true,
        }
    }
}

/// A zero-filled buffer of `len` elements, allocated fallibly: an allocation failure becomes
/// [`CodecError::LimitExceeded`] instead of aborting the process (which `catch_unwind` cannot
/// stop).
pub(crate) fn alloc_zeroed<T: Copy + Default>(len: usize) -> Result<Vec<T>, CodecError> {
    let mut v = Vec::new();
    v.try_reserve_exact(len).map_err(|_| CodecError::LimitExceeded(format!("cannot allocate {len} elements for the image")))?;
    v.resize(len, T::default());
    Ok(v)
}
