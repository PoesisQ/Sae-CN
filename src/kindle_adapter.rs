//! On-demand KF8/MOBI reconstruction. C only parses the binary container; Rust
//! owns bounds, packaging and lifetime. No child process or retained parse tree.
use crate::{security, text_adapter::Converted};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::HashSet,
    ffi::{CStr, c_char, c_int, c_void},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use zip::{ZipWriter, write::SimpleFileOptions};

#[cfg(windows)]
type PathChar = u16;
#[cfg(not(windows))]
type PathChar = c_char;
unsafe extern "C" {
    fn sae_kindle_convert(
        path: *const PathChar,
        emit: unsafe extern "C" fn(*mut c_void, *const c_char, *const u8, usize) -> c_int,
        user: *mut c_void,
    ) -> c_int;
}

/// Validate the record table and both halves of hybrid files before native
/// allocations. Encryption is rejected; the native decryption code is unbuilt.
fn preflight(path: &Path) -> Result<()> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    ensure!(
        length <= 96 * 1024 * 1024,
        "Kindle 文件超过当前 96 MiB 解析上限"
    );
    let mut header = [0u8; 78];
    file.read_exact(&mut header)
        .context("Kindle 文件头不完整")?;
    ensure!(
        &header[60..68] == b"BOOKMOBI",
        "不是有效的 MOBI / AZW3 文件"
    );
    let count = u16::from_be_bytes([header[76], header[77]]) as usize;
    ensure!(
        (2..=20_000).contains(&count),
        "Kindle 记录数量无效或超过上限"
    );
    let mut table = vec![0u8; count * 8];
    file.read_exact(&mut table).context("Kindle 记录表不完整")?;
    let mut offsets: Vec<u64> = table
        .chunks_exact(8)
        .map(|s| u32::from_be_bytes(s[..4].try_into().unwrap()) as u64)
        .collect();
    ensure!(
        offsets[0] >= 78 + table.len() as u64,
        "Kindle 记录覆盖文件头"
    );
    offsets.push(length);
    ensure!(
        offsets.windows(2).all(|s| s[0] < s[1]),
        "Kindle 记录越界或顺序错误"
    );
    let mut next_is_header = true;
    for (index, pair) in offsets.windows(2).enumerate() {
        let mut bytes = [0u8; 20];
        let size = ((pair[1] - pair[0]) as usize).min(bytes.len());
        file.seek(SeekFrom::Start(pair[0]))?;
        file.read_exact(&mut bytes[..size])?;
        let compression = u16::from_be_bytes([bytes[0], bytes[1]]);
        // Some hybrid multimedia books contain a second BOUNDARY followed by
        // CONT, not another MOBI header. Check actual headers, not every marker.
        if index == 0
            || next_is_header
                && (size == 20 && &bytes[16..20] == b"MOBI" || matches!(compression, 1 | 2 | 17480))
        {
            ensure!(size >= 16, "Kindle 内容头损坏");
            ensure!(
                u16::from_be_bytes([bytes[12], bytes[13]]) == 0,
                "此 Kindle 电子书受 DRM 加密保护，无法读取；请选择无 DRM 的版本"
            );
            ensure!(size == 20 && &bytes[16..20] == b"MOBI", "Kindle 内容头损坏");
            ensure!(
                matches!(u16::from_be_bytes([bytes[0], bytes[1]]), 1 | 2 | 17480),
                "Kindle 文本压缩方式暂不支持"
            );
        }
        next_is_header = size >= 8 && &bytes[..8] == b"BOUNDARY";
    }
    Ok(())
}

struct Archive {
    writer: ZipWriter<File>,
    names: HashSet<String>,
    total: usize,
    error: Option<anyhow::Error>,
    multimedia: bool,
}
impl Archive {
    fn entry(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        ensure!(security::safe_archive_path(name), "Kindle 资源路径不安全");
        ensure!(
            bytes.len() <= 32 * 1024 * 1024,
            "Kindle 单项资源超过 32 MiB 上限"
        );
        ensure!(
            self.names.len() < 10_000 && self.names.insert(name.to_ascii_lowercase()),
            "Kindle 资源重复或数量过多"
        );
        self.total = self
            .total
            .checked_add(bytes.len())
            .context("Kindle 资源大小溢出")?;
        ensure!(
            self.total <= 128 * 1024 * 1024,
            "Kindle 输出超过 128 MiB 上限"
        );
        self.writer.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )?;
        self.writer.write_all(bytes)?;
        Ok(())
    }
}
unsafe extern "C" fn emit(
    user: *mut c_void,
    name: *const c_char,
    bytes: *const u8,
    size: usize,
) -> c_int {
    let archive = unsafe { &mut *user.cast::<Archive>() };
    // Never unwind into C, even if a future writer implementation panics.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        ensure!(
            !name.is_null() && !bytes.is_null() && size <= 32 * 1024 * 1024,
            "Kindle 解析资源无效"
        );
        let name = unsafe { CStr::from_ptr(name) }.to_str()?;
        let bytes = unsafe { std::slice::from_raw_parts(bytes, size) };
        archive.multimedia |= [".mp3", ".mp4", ".mpeg"].iter().any(|e| name.ends_with(e));
        archive.entry(name, bytes)
    }));
    match result {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            archive.error = Some(error);
            1
        }
        Err(_) => {
            archive.error = Some(anyhow::anyhow!("Kindle 转换意外中断"));
            1
        }
    }
}
pub fn convert(path: &Path) -> Result<Converted> {
    preflight(path)?;
    let mut output = Converted::temporary("kindle")?;
    let mut archive = Archive {
        writer: ZipWriter::new(File::create(&output.path)?),
        names: HashSet::new(),
        total: 0,
        error: None,
        multimedia: false,
    };
    archive.entry("mimetype", b"application/epub+zip")?;
    archive.entry("META-INF/container.xml", b"<?xml version=\"1.0\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>")?;
    #[cfg(windows)]
    let native_path: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    };
    #[cfg(not(windows))]
    let native_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    let status = unsafe {
        sae_kindle_convert(
            native_path.as_ptr(),
            emit,
            (&mut archive as *mut Archive).cast(),
        )
    };
    if let Some(error) = archive.error.take() {
        return Err(error);
    }
    match status {
        0 => {}
        -1 => bail!("无法打开 Kindle 电子书"),
        -2 | 5 | 13 => bail!("此 Kindle 电子书受 DRM 加密保护，无法读取；请选择无 DRM 的版本"),
        -3 | 7 => bail!("Kindle 解析达到 96 MiB 原生内存或资源上限"),
        code => bail!("Kindle 文件损坏或包含暂不支持的结构（解析码 {code}）"),
    }
    ensure!(
        archive.names.contains("content.opf"),
        "Kindle 缺少可读取的书籍结构"
    );
    if archive.multimedia {
        output.notice = "此书含音视频资源，当前仅支持静态阅读".into();
    }
    archive.writer.finish()?;
    Ok(output)
}
