//! Optional Windows DDS encoder. No shell, shared temporary paths or destination writes.
use crate::{CodecError, EncodeOptions, Format, Image};

fn err(e: impl std::fmt::Display) -> CodecError {
    CodecError::encode(Format::Dds, e)
}

pub(super) fn encode(image: &Image, opts: &EncodeOptions) -> Result<Vec<u8>, CodecError> {
    #[cfg(target_os = "windows")]
    {
        windows::encode(image, opts)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (image, opts);
        Err(err("the advanced DDS encoding backend is available only on Windows"))
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{CodecError, EncodeOptions, Image, err};
    use crate::{ChannelLayout, DdsCompression as C, SampleType};
    use std::os::windows::process::CommandExt;
    use std::{
        fs::{self, File},
        io::Read,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    const MAX_LOG: u64 = 1 << 20;

    fn backend() -> Result<PathBuf, CodecError> {
        let path = if let Some(p) = std::env::var_os("PHOTOCRAFT_DDS_BACKEND") {
            PathBuf::from(p)
        } else {
            std::env::current_exe().map_err(err)?.parent().ok_or_else(|| err("cannot find DDS backend directory"))?.join("codecs/dds/texconv.exe")
        };
        if !path.is_absolute() || !path.is_file() {
            return Err(err("DDS encoding backend missing: install codecs/dds/texconv.exe beside PhotoCraft"));
        }
        Ok(path)
    }

    fn diagnostic(path: &Path) -> String {
        let mut b = Vec::new();
        if let Ok(f) = File::open(path) {
            let _ = f.take(8192).read_to_end(&mut b);
        }
        String::from_utf8_lossy(&b).into_owned()
    }

    // Files avoid pipe deadlocks. Both size and elapsed time are checked while the child runs.
    fn run(mut command: Command, log: &Path, timeout: Duration) -> Result<(), CodecError> {
        let output = File::create(log).map_err(err)?;
        command.stdout(Stdio::from(output.try_clone().map_err(err)?)).stderr(Stdio::from(output)).stdin(Stdio::null()).creation_flags(0x0800_0000);
        let mut child = command.spawn().map_err(|e| err(format!("cannot start DDS backend: {e}")))?;
        let start = Instant::now();
        loop {
            let status = match child.try_wait() {
                Ok(status) => status,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(err(format!("DDS backend wait failed: {e}")));
                }
            };
            if let Some(status) = status {
                if !status.success() {
                    return Err(err(format!("DDS backend failed ({status}): {}", diagnostic(log))));
                }
                if fs::metadata(log).map_err(err)?.len() > MAX_LOG {
                    return Err(err("DDS backend diagnostic limit exceeded"));
                }
                return Ok(());
            }
            let too_much = fs::metadata(log).map(|m| m.len() > MAX_LOG).unwrap_or(false);
            if start.elapsed() > timeout || too_much {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err(if too_much { "DDS backend diagnostic limit exceeded" } else { "DDS backend timed out; no destination was written" }));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn dxgi_name(c: C) -> Result<&'static str, CodecError> {
        Ok(match c {
            C::Bc4 => "BC4_UNORM",
            C::Bc4Snorm => "BC4_SNORM",
            C::Bc5 => "BC5_UNORM",
            C::Bc5Snorm => "BC5_SNORM",
            C::Bc6h => "BC6H_UF16",
            C::Bc6hSigned => "BC6H_SF16",
            C::Bc7 => "BC7_UNORM",
            C::Bc7Srgb => "BC7_UNORM_SRGB",
            C::Bc1Srgb => "BC1_UNORM_SRGB",
            C::Bc1Linear => "BC1_UNORM",
            C::Bc2Linear => "BC2_UNORM",
            C::Bc3Linear => "BC3_UNORM",
            C::Bc2Srgb => "BC2_UNORM_SRGB",
            C::Bc3Srgb => "BC3_UNORM_SRGB",
            C::RgbaLinear => "R8G8B8A8_UNORM",
            C::RgbaSrgb => "R8G8B8A8_UNORM_SRGB",
            C::RgbaFloat => "R32G32B32A32_FLOAT",
            _ => return Err(err("unexpected DDS backend format")),
        })
    }

    /// DDS RGBA32F, or RGBA8 tagged with the same transfer as the requested LDR target.
    fn interchange(image: &Image, c: C) -> Result<Vec<u8>, CodecError> {
        let sample = if c.is_float() { SampleType::F32 } else { SampleType::U8 };
        let img = image.converted(ChannelLayout::Rgba, sample);
        let (w, h) = img.dimensions();
        let mut b = crate::options::alloc_zeroed::<u8>(148usize.checked_add(img.data().len()).ok_or_else(|| err("DDS interchange size overflow"))?)?;
        b[..4].copy_from_slice(b"DDS ");
        let pitch = w.checked_mul(if sample == SampleType::F32 { 16 } else { 4 }).ok_or_else(|| err("DDS pitch overflow"))?;
        let dxgi = if sample == SampleType::F32 {
            2
        } else if c.is_srgb() {
            29
        } else {
            28
        };
        for (at, v) in [
            (4, 124),
            (8, 0x100f),
            (12, h),
            (16, w),
            (20, pitch),
            (76, 32),
            (80, 4),
            (84, u32::from_le_bytes(*b"DX10")),
            (108, 0x1000),
            (128, dxgi),
            (132, 3),
            (140, 1),
        ] {
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        // Windows is little endian; Image's float storage is native endian.
        b[148..].copy_from_slice(img.data());
        Ok(b)
    }

    fn expected_size(w: u32, h: u32, c: C, levels: u32) -> Result<usize, CodecError> {
        let mut total = 148usize;
        let (mut w, mut h) = (w as usize, h as usize);
        for _ in 0..levels {
            let size = if c.is_block() {
                w.div_ceil(4)
                    .checked_mul(h.div_ceil(4))
                    .and_then(|n| n.checked_mul(if matches!(c, C::Bc4 | C::Bc4Snorm | C::Bc1Srgb | C::Bc1Linear) { 8 } else { 16 }))
            } else {
                w.checked_mul(h).and_then(|n| n.checked_mul(if c == C::RgbaFloat { 16 } else { 4 }))
            }
            .ok_or_else(|| err("DDS output size overflow"))?;
            total = total.checked_add(size).ok_or_else(|| err("DDS output size overflow"))?;
            (w, h) = ((w / 2).max(1), (h / 2).max(1));
        }
        Ok(total)
    }

    fn validate(b: &[u8], image: &Image, c: C, levels: u32) -> Result<(), CodecError> {
        let info = super::super::dds::info(b)?;
        let read = |at| -> Option<u32> { Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?)) };
        if (info.width, info.height) != image.dimensions()
            || info.levels != levels
            || read(128) != Some(c.dxgi())
            || b.get(84..88) != Some(b"DX10")
            || b.len() != expected_size(info.width, info.height, c, levels)?
        {
            return Err(err("DDS backend returned mismatched or truncated output"));
        }
        Ok(())
    }

    pub(super) fn encode(image: &Image, opts: &EncodeOptions) -> Result<Vec<u8>, CodecError> {
        let path = backend()?;
        let c = opts.dds.compression;
        let levels = if opts.dds.mipmaps { image.width().max(image.height()).max(1).ilog2() + 1 } else { 1 };
        let size = expected_size(image.width(), image.height(), c, levels)?;
        if size as u64 > crate::Limits::default().max_alloc {
            return Err(err("DDS output exceeds allocation limit"));
        }
        let temp = tempfile::Builder::new().prefix("photocraft-dds-").tempdir().map_err(err)?;
        let input = temp.path().join("input.dds");
        let out = temp.path().join("out");
        fs::create_dir(&out).map_err(err)?;
        fs::write(&input, interchange(image, c)?).map_err(err)?;
        let mut command = Command::new(path);
        // DirectXTex's GPU BC6H path clamps negative source values even for SF16.
        // The CPU SF16 codec retains them (covered by signed HDR round-trip tests).
        // The GPU BC7 encoder also loses alpha on partial edge blocks (9x6 regression).
        // Keep the GPU for block-aligned LDR textures; use CPU to reduce edge alpha error.
        let partial_bc7 = matches!(c, C::Bc7 | C::Bc7Srgb) && (!image.width().is_multiple_of(4) || !image.height().is_multiple_of(4));
        if c == C::Bc6hSigned || partial_bc7 {
            command.arg("-nogpu");
        }
        // DirectXTex's native BOX path rejects non-power-of-two float images.
        let filter = if image.width().is_power_of_two() && image.height().is_power_of_two() { "BOX" } else { "LINEAR" };
        command
            .args(["-nologo", "-dx10", "-sepalpha", "-if", filter, "-f", dxgi_name(c)?, "-m", if opts.dds.mipmaps { "0" } else { "1" }, "-ft", "dds", "-o"])
            .arg(&out)
            .arg(&input);
        run(command, &temp.path().join("backend.log"), Duration::from_secs(300))?;
        let output = out.join("input.dds");
        if fs::metadata(&output).map_err(|e| err(format!("DDS backend produced no file: {e}")))?.len() != size as u64 {
            return Err(err("DDS backend output size mismatch"));
        }
        let bytes = fs::read(output).map_err(err)?;
        validate(&bytes, image, c, levels)?;
        Ok(bytes)
    }

    #[cfg(test)]
    mod tests {
        use super::{interchange, run, validate};
        use crate::{ChannelLayout, DdsCompression as C, Image};
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        #[test]
        #[ignore]
        fn child_fixture() {
            match std::env::var("PHOTOCRAFT_TEST_DDS_CHILD").as_deref() {
                Ok("hang") => std::thread::sleep(Duration::from_secs(5)),
                Ok("flood") => {
                    use std::io::Write;
                    std::io::stdout().write_all(&vec![b'x'; 2 << 20]).unwrap();
                }
                Ok("fail") => panic!("synthetic backend failure"),
                _ => {}
            }
        }
        #[test]
        fn backend_failure_timeout_and_log_limit_are_bounded() {
            let temp = tempfile::tempdir().unwrap();
            for mode in ["hang", "flood", "fail"] {
                let mut cmd = Command::new(std::env::current_exe().unwrap());
                cmd.args(["--ignored", "--exact", "codecs::dds_native::windows::tests::child_fixture", "--nocapture"]).env("PHOTOCRAFT_TEST_DDS_CHILD", mode);
                let start = Instant::now();
                let error = run(cmd, &temp.path().join(mode), Duration::from_millis(150)).unwrap_err().to_string();
                assert!(start.elapsed() < Duration::from_secs(3), "{mode}: {error}");
                assert!(
                    error.contains(match mode {
                        "hang" => "timed out",
                        "flood" => "limit",
                        _ => "failed",
                    }),
                    "{mode}: {error}"
                );
            }
        }
        #[test]
        fn rejects_malformed_success_output() {
            let image = Image::from_u8(4, 4, ChannelLayout::Rgba, vec![255; 64]).unwrap();
            assert!(validate(b"not a DDS", &image, C::Bc7, 1).is_err());
            let b = interchange(&image, C::Bc7).unwrap();
            assert!(validate(&b, &image, C::Bc7, 1).is_err());
        }
    }
}
