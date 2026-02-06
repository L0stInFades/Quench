#![allow(dead_code)]

use std::{io::Read, path::PathBuf, pin::Pin, sync::Arc, time::Duration};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use thiserror::Error;
use std::future::Future;

pub mod telemetry {
    use super::*;
    use std::time::Instant;

    #[derive(Debug, Clone, Default)]
    pub struct Throughput {
        pub bytes_total: u64,
        pub elapsed: Duration,
    }

    impl Throughput {
        pub fn mb_per_sec(&self) -> f64 {
            if self.elapsed.as_secs_f64() == 0.0 {
                0.0
            } else {
                (self.bytes_total as f64 / 1_000_000.0) / self.elapsed.as_secs_f64()
            }
        }

        pub fn record(&mut self, bytes: u64, start: Instant) {
            self.bytes_total += bytes;
            self.elapsed = start.elapsed();
        }
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Progress {
        pub files: u64,
        pub bytes: u64,
    }
}

pub mod errors {
    use super::*;

    #[derive(Error, Debug)]
    pub enum ExtractError {
        #[error("io error: {0}")]
        Io(#[from] std::io::Error),
        #[error("serde error: {0}")]
        Serde(#[from] serde_json::Error),
        #[error("task join error: {0}")]
        Join(#[from] tokio::task::JoinError),
        #[error("integrity failure: {details}")]
        IntegrityFailure { details: String },
        #[error("unsupported format: {0}")]
        Unsupported(String),
        #[error("password required or incorrect")]
        Password,
        #[error("corrupt block at offset {offset}")]
        CorruptBlock { offset: u64 },
        #[error("unimplemented: {0}")]
        Unimplemented(String),
    }

    pub type Result<T> = std::result::Result<T, ExtractError>;
}

pub mod codecs {
    use super::*;

    use crate::errors::{ExtractError, Result};
    use crate::resilience::{guard, IntegrityPolicy};

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CodecResult {
        pub bytes_in: u64,
        pub bytes_out: u64,
        pub verified: bool,
    }

    pub trait Codec: Send + Sync {
        fn name(&self) -> &'static str;

        fn decompress(&self, payload: &[u8], integrity: &IntegrityPolicy) -> Result<Bytes>;
    }

    #[derive(Clone)]
    pub struct ZstdCodec;

    #[derive(Clone)]
    pub struct Lz4Codec;

    #[derive(Clone)]
    pub struct BrotliCodec;

    impl Codec for ZstdCodec {
        fn name(&self) -> &'static str { "zstd" }

        fn decompress(&self, payload: &[u8], integrity: &IntegrityPolicy) -> Result<Bytes> {
            let mut cursor = std::io::Cursor::new(payload);
            let mut decoder = zstd::stream::read::Decoder::new(&mut cursor)
                .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
            let mut out = Vec::new();
            std::io::copy(&mut decoder, &mut out)?;
            guard(&out, integrity)?;
            Ok(Bytes::from(out))
        }
    }

    impl Codec for Lz4Codec {
        fn name(&self) -> &'static str { "lz4" }

        fn decompress(&self, payload: &[u8], integrity: &IntegrityPolicy) -> Result<Bytes> {
            let out = lz4_flex::block::decompress(payload, 0)
                .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
            guard(&out, integrity)?;
            Ok(Bytes::from(out))
        }
    }

    impl Codec for BrotliCodec {
        fn name(&self) -> &'static str { "brotli" }

        fn decompress(&self, payload: &[u8], integrity: &IntegrityPolicy) -> Result<Bytes> {
            let mut reader = brotli::Decompressor::new(payload, 4096);
            let mut out = Vec::new();
            std::io::copy(&mut reader, &mut out)?;
            guard(&out, integrity)?;
            Ok(Bytes::from(out))
        }
    }

    pub enum CodecKind {
        Zstd,
        Lz4,
        Brotli,
    }

    pub fn codec_from_name(name: &str) -> Option<Arc<dyn Codec>> {
        match name {
            "zstd" | "zst" => Some(Arc::new(ZstdCodec)),
            "lz4" | "lz4hc" => Some(Arc::new(Lz4Codec)),
            "brotli" | "br" => Some(Arc::new(BrotliCodec)),
            _ => None,
        }
    }

    pub trait Compressor: Send + Sync {
        fn name(&self) -> &'static str;
        fn compress(&self, data: &[u8], level: Option<u32>) -> Result<Vec<u8>>;
    }

    #[derive(Clone)]
    pub struct ZstdCompressor;

    #[derive(Clone)]
    pub struct Lz4Compressor;

    #[derive(Clone)]
    pub struct BrotliCompressor;

    impl Compressor for ZstdCompressor {
        fn name(&self) -> &'static str { "zstd" }

        fn compress(&self, data: &[u8], level: Option<u32>) -> Result<Vec<u8>> {
            let level = level.unwrap_or(3) as i32;
            let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), level)
                .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
            std::io::copy(&mut &*data, &mut encoder)?;
            let compressed = encoder.finish()
                .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
            Ok(compressed)
        }
    }

    impl Compressor for Lz4Compressor {
        fn name(&self) -> &'static str { "lz4" }

        fn compress(&self, data: &[u8], _level: Option<u32>) -> Result<Vec<u8>> {
            Ok(lz4_flex::block::compress(data))
        }
    }

    impl Compressor for BrotliCompressor {
        fn name(&self) -> &'static str { "brotli" }

        fn compress(&self, data: &[u8], level: Option<u32>) -> Result<Vec<u8>> {
            let level = level.unwrap_or(3) as u32;
            let mut compressor = brotli::CompressorReader::new(data, 4096, level, 22);
            let mut compressed = Vec::new();
            std::io::copy(&mut compressor, &mut compressed)?;
            Ok(compressed)
        }
    }

    pub fn compressor_from_name(name: &str) -> Option<Arc<dyn Compressor>> {
        match name {
            "zstd" | "zst" => Some(Arc::new(ZstdCompressor)),
            "lz4" | "lz4hc" => Some(Arc::new(Lz4Compressor)),
            "brotli" | "br" => Some(Arc::new(BrotliCompressor)),
            _ => None,
        }
    }
}

pub mod format_detection {
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    use crate::errors::{ExtractError, Result};

    #[derive(Debug, Clone, PartialEq)]
    pub enum DetectedFormat {
        TarZstd,
        TarLz4,
        TarBrotli,
        TarGzip,
        TarPlain,
        Zip,
        SevenZip,
        Rar,
        Unknown,
    }

    impl DetectedFormat {
        pub fn as_str(&self) -> &'static str {
            match self {
                DetectedFormat::TarZstd => "tar.zst",
                DetectedFormat::TarLz4 => "tar.lz4",
                DetectedFormat::TarBrotli => "tar.br",
                DetectedFormat::TarGzip => "tar.gz",
                DetectedFormat::TarPlain => "tar",
                DetectedFormat::Zip => "zip",
                DetectedFormat::SevenZip => "7z",
                DetectedFormat::Rar => "rar",
                DetectedFormat::Unknown => "unknown",
            }
        }

        pub fn extension(&self) -> &'static str {
            match self {
                DetectedFormat::TarZstd => ".tar.zst",
                DetectedFormat::TarLz4 => ".tar.lz4",
                DetectedFormat::TarBrotli => ".tar.br",
                DetectedFormat::TarGzip => ".tar.gz",
                DetectedFormat::TarPlain => ".tar",
                DetectedFormat::Zip => ".zip",
                DetectedFormat::SevenZip => ".7z",
                DetectedFormat::Rar => ".rar",
                DetectedFormat::Unknown => "",
            }
        }
    }

    /// Detect file format from magic bytes (first few bytes of file)
    pub fn detect_from_magic_bytes(path: &Path) -> Result<DetectedFormat> {
        let mut file = File::open(path)
            .map_err(|e| ExtractError::Io(e))?;

        let mut buffer = [0u8; 262]; // Enough for all known magic bytes
        let n = file.read(&mut buffer)
            .map_err(|e| ExtractError::Io(e))?;

        if n == 0 {
            return Ok(DetectedFormat::Unknown);
        }

        // ZIP magic: PK\x03\x04 or PK\x05\x06 (empty archive)
        if buffer.starts_with(b"PK\x03\x04") || buffer.starts_with(b"PK\x05\x06") {
            return Ok(DetectedFormat::Zip);
        }

        // 7-Zip magic: 7z\xBC\xAF\x27\x1C
        if buffer.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
            return Ok(DetectedFormat::SevenZip);
        }

        // RAR magic: Rar!\x1A\x07\x00 or Rar!\x1A\x07\x01\x00
        if buffer.starts_with(b"Rar!\x1A\x07") {
            return Ok(DetectedFormat::Rar);
        }

        // Zstandard magic: 0xFD2FB528 (little endian)
        if buffer.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
            return Ok(DetectedFormat::TarZstd);
        }

        // LZ4 magic: 0x04224D18 (little endian)
        if buffer.starts_with(&[0x18, 0x4D, 0x22, 0x04]) {
            return Ok(DetectedFormat::TarLz4);
        }

        // Brotli magic (no fixed magic, but typical files start with specific patterns)
        // Check for valid Brotli header bits
        if n >= 2 && (buffer[0] & 0xE0) == 0 && (buffer[1] & 0x03) != 0x03 {
            // Likely Brotli - use heuristics
            // Brotli doesn't have a fixed magic, so we check the header structure
            return Ok(DetectedFormat::TarBrotli);
        }

        // Gzip magic: \x1F\x8B
        if buffer.starts_with(&[0x1F, 0x8B]) {
            return Ok(DetectedFormat::TarGzip);
        }

        // TAR magic: No fixed magic, but check for tar header patterns
        // TAR files start with a 512-byte header
        if n >= 512 {
            // Check for valid tar header: first 100 bytes are filename (null terminated)
            // and fields at specific positions should be valid octal numbers
            let has_valid_tar_header = validate_tar_header(&buffer);
            if has_valid_tar_header {
                return Ok(DetectedFormat::TarPlain);
            }
        }

        Ok(DetectedFormat::Unknown)
    }

    /// Detect format from file extension as fallback
    pub fn detect_from_extension(path: &Path) -> DetectedFormat {
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        // Handle compound extensions like .tar.gz
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if file_name.ends_with(".tar.zst") || file_name.ends_with(".tar.zst") {
            return DetectedFormat::TarZstd;
        }
        if file_name.ends_with(".tar.lz4") || file_name.ends_with(".tar.lz4") {
            return DetectedFormat::TarLz4;
        }
        if file_name.ends_with(".tar.br") || file_name.ends_with(".tar.br") {
            return DetectedFormat::TarBrotli;
        }
        if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
            return DetectedFormat::TarGzip;
        }
        if file_name.ends_with(".tar") {
            return DetectedFormat::TarPlain;
        }

        match extension.to_lowercase().as_str() {
            "zip" => DetectedFormat::Zip,
            "7z" => DetectedFormat::SevenZip,
            "rar" => DetectedFormat::Rar,
            "zst" => DetectedFormat::TarZstd,
            "lz4" => DetectedFormat::TarLz4,
            "br" => DetectedFormat::TarBrotli,
            "gz" => DetectedFormat::TarGzip,
            _ => DetectedFormat::Unknown,
        }
    }

    /// Auto-detect format using both magic bytes and extension
    pub fn detect_format(path: &Path) -> Result<DetectedFormat> {
        // Try magic bytes first (more reliable)
        match detect_from_magic_bytes(path) {
            Ok(DetectedFormat::Unknown) => {
                // Fall back to extension detection
                Ok(detect_from_extension(path))
            }
            result => result,
        }
    }

    fn validate_tar_header(buffer: &[u8]) -> bool {
        if buffer.len() < 512 {
            return false;
        }

        // Check for null-terminated filename in first 100 bytes
        let filename_area = &buffer[0..100];
        let has_null = filename_area.iter().any(|&b| b == 0);

        // Check checksum field at position 148 (8 bytes, should be octal)
        let checksum_area = &buffer[148..155];
        let checksum_valid = checksum_area.iter().all(|&b| b == 0 || b.is_ascii_digit() || b == b' ');

        // Check magic field at position 257 (should be "ustar" or null)
        let magic_area = &buffer[257..263];
        let magic_valid = magic_area == b"ustar\0" || magic_area.iter().all(|&b| b == 0);

        has_null && (checksum_valid || magic_valid)
    }
}

pub mod resilience {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::io::Read;
    use serde::{Deserialize, Serialize};

    use crate::errors::{ExtractError, Result};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct IntegrityPolicy {
        pub crc32: Option<u32>,
        pub hmac_key: Option<Vec<u8>>,
        pub hmac_tag: Option<Vec<u8>>,
        pub retry_attempts: u8,
        pub skip_bad_blocks: bool,
        pub block_size: usize,
    }

    impl Default for IntegrityPolicy {
        fn default() -> Self {
            Self {
                crc32: None,
                hmac_key: None,
                hmac_tag: None,
                retry_attempts: 1,
                skip_bad_blocks: true,
                block_size: 1 << 20, // 1 MiB chunks for integrity rolling
            }
        }
    }

    impl IntegrityPolicy {
        pub fn strict() -> Self {
            Self {
                retry_attempts: 3,
                skip_bad_blocks: false,
                ..Default::default()
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum IntegrityVerdict {
        Clean,
        Corrupt { reason: String },
    }

    pub fn verify_crc32(bytes: &[u8], expected: u32) -> IntegrityVerdict {
        let calc = crc32fast::hash(bytes);
        if calc == expected {
            IntegrityVerdict::Clean
        } else {
            IntegrityVerdict::Corrupt {
                reason: format!("crc mismatch expected {expected} got {calc}"),
            }
        }
    }

    pub fn verify_hmac(bytes: &[u8], key: &[u8], expected: &[u8]) -> IntegrityVerdict {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(key)
            .unwrap_or_else(|_| HmacSha256::new_from_slice(&[0u8; 32]).unwrap());
        mac.update(bytes);
        match mac.verify_slice(expected) {
            Ok(_) => IntegrityVerdict::Clean,
            Err(_) => IntegrityVerdict::Corrupt {
                reason: "hmac mismatch".to_string(),
            },
        }
    }

    pub fn guard(bytes: &[u8], policy: &IntegrityPolicy) -> Result<()> {
        if let Some(expected) = policy.crc32 {
            if let IntegrityVerdict::Corrupt { reason } = verify_crc32(bytes, expected) {
                return Err(ExtractError::IntegrityFailure { details: reason });
            }
        }
        if let (Some(key), Some(tag)) = (policy.hmac_key.as_ref(), policy.hmac_tag.as_ref()) {
            if let IntegrityVerdict::Corrupt { reason } = verify_hmac(bytes, key, tag) {
                return Err(ExtractError::IntegrityFailure { details: reason });
            }
        }
        Ok(())
    }

    pub struct IntegrityGuardReader<R: Read> {
        inner: R,
        policy: IntegrityPolicy,
        crc: Option<crc32fast::Hasher>,
        hmac: Option<Hmac<Sha256>>,
        bytes: u64,
    }

    impl<R: Read> IntegrityGuardReader<R> {
        pub fn new(inner: R, policy: IntegrityPolicy) -> Self {
            let crc = policy.crc32.map(|_| crc32fast::Hasher::new());
            let hmac = policy
                .hmac_key
                .as_ref()
                .map(|key| Hmac::<Sha256>::new_from_slice(key).unwrap());
            Self {
                inner,
                policy,
                crc,
                hmac,
                bytes: 0,
            }
        }

        pub fn finalize(mut self) -> Result<()> {
            if let Some(expected) = self.policy.crc32 {
                if let Some(hasher) = self.crc.take() {
                    let calc = hasher.finalize();
                    if calc != expected {
                        return Err(ExtractError::IntegrityFailure {
                            details: format!("crc mismatch expected {expected} got {calc}"),
                        });
                    }
                }
            }
            if let (Some(mac), Some(tag)) = (self.hmac.take(), self.policy.hmac_tag.as_ref()) {
                mac.verify_slice(tag)
                    .map_err(|_| ExtractError::IntegrityFailure { details: "hmac mismatch".into() })?;
            }
            Ok(())
        }
    }

    impl<R: Read> Read for IntegrityGuardReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(buf)?;
            if n == 0 {
                return Ok(0);
            }
            if let Some(hasher) = self.crc.as_mut() {
                hasher.update(&buf[..n]);
            }
            if let Some(mac) = self.hmac.as_mut() {
                mac.update(&buf[..n]);
            }
            self.bytes += n as u64;
            Ok(n)
        }
    }
}

pub mod scheduler {
    use rayon::prelude::*;

    pub struct ChunkScheduler {
        pool: rayon::ThreadPool,
    }

    impl ChunkScheduler {
        pub fn new(workers: usize) -> Self {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers.max(1))
                .build()
                .expect("failed to build rayon pool");
            Self { pool }
        }

        pub fn map<I, F, R>(&self, input: I, f: F) -> Vec<R>
        where
            I: IntoIterator + Send,
            I::Item: Send,
            F: Fn(I::Item) -> R + Send + Sync,
            R: Send,
        {
            let input_vec: Vec<_> = input.into_iter().collect();
            let f_ref = &f;
            self.pool.install(|| input_vec.into_par_iter().map(f_ref).collect())
        }
    }
}

pub mod containers {
    use super::*;

    use crate::codecs::Codec;
    use crate::errors::{ExtractError, Result};
    use crate::resilience::{IntegrityGuardReader, IntegrityPolicy};

    #[derive(Debug, Clone)]
    pub struct ExtractOptions {
        pub destination: PathBuf,
        pub integrity: IntegrityPolicy,
        pub concurrency: usize,
    }

    impl Default for ExtractOptions {
        fn default() -> Self {
            Self {
                destination: PathBuf::from("./output"),
                integrity: IntegrityPolicy::default(),
                concurrency: num_cpus::get().max(1),
            }
        }
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ExtractReport {
        pub entries: u64,
        pub bytes_written: u64,
        pub warnings: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProgressInfo {
        pub current_file: String,
        pub current_file_bytes: u64,
        pub total_bytes: u64,
        pub files_processed: u64,
        pub total_files: u64,
    }

    pub trait Container: Send + Sync {
        fn name(&self) -> &'static str;

        fn extract_boxed(
            &self,
            reader: Box<dyn AsyncRead + Unpin + Send>,
            options: ExtractOptions,
        ) -> Pin<Box<dyn Future<Output = Result<ExtractReport>> + Send + '_>>;
    }

    #[derive(Clone)]
    pub struct TarContainer {
        codec: Arc<dyn Codec>,
    }

    impl TarContainer {
        pub fn new(codec: Arc<dyn Codec>) -> Self { Self { codec } }
    }

    #[derive(Clone)]
    pub struct ZipContainer;

    impl Container for TarContainer {
        fn name(&self) -> &'static str {
            match self.codec.name() {
                "zstd" => "tar.zst",
                "lz4" => "tar.lz4",
                "brotli" => "tar.br",
                _ => "tar",
            }
        }

        fn extract_boxed(
            &self,
            reader: Box<dyn AsyncRead + Unpin + Send>,
            options: ExtractOptions,
        ) -> Pin<Box<dyn Future<Output = Result<ExtractReport>> + Send + '_>> {
            Box::pin(async move {
                let dest = options.destination.clone();
                let policy = options.integrity.clone();
                let codec = self.codec.clone();

                // Read all data into memory first
                let mut data = Vec::new();
                {
                    let mut reader = reader;
                    reader.read_to_end(&mut data).await
                        .map_err(|e| ExtractError::IntegrityFailure { details: format!("{}", e) })?;
                }

                let report = tokio::task::spawn_blocking(move || -> Result<ExtractReport> {
                    let decoder: Box<dyn Read> = match codec.name() {
                        "zstd" => Box::new(
                            zstd::stream::read::Decoder::new(&data[..])
                                .map_err(|e| ExtractError::IntegrityFailure { details: format!("{}", e) })?,
                        ),
                        "lz4" | "lz4hc" => Box::new(lz4_flex::frame::FrameDecoder::new(&data[..])),
                        "brotli" | "br" => Box::new(brotli::Decompressor::new(&data[..], 32 * 1024)),
                        _ => Box::new(&data[..]),
                    };

                    let mut guarded = IntegrityGuardReader::new(decoder, policy.clone());
                    let mut archive = tar::Archive::new(&mut guarded);
                    let mut entries = 0u64;
                    let mut bytes_written = 0u64;
                    let mut warnings = Vec::new();

                    let entries_iter = archive.entries()?;
                    for entry_res in entries_iter {
                        let mut file: tar::Entry<_> = match entry_res {
                            Ok(f) => f,
                            Err(e) => {
                                warnings.push(format!("entry read failure: {}", e));
                                if !policy.skip_bad_blocks {
                                    return Err(ExtractError::IntegrityFailure { details: format!("{}", e) });
                                }
                                continue;
                            }
                        };

                        let path = match file.path() {
                            Ok(p) => p.into_owned(),
                            Err(e) => {
                                warnings.push(format!("path error: {}", e));
                                if !policy.skip_bad_blocks {
                                    return Err(ExtractError::IntegrityFailure { details: format!("{}", e) });
                                }
                                continue;
                            }
                        };

                        let out_path = dest.join(path);
                        if let Some(parent) = out_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        match file.unpack(&out_path) {
                            Ok(_) => {
                                bytes_written += file.size();
                                entries += 1;
                            }
                            Err(e) => {
                                warnings.push(format!("failed unpack {}: {}", out_path.display(), e));
                                if !policy.skip_bad_blocks {
                                    return Err(ExtractError::IntegrityFailure { details: format!("{}", e) });
                                }
                            }
                        }
                    }

                    guarded.finalize()?;
                    Ok(ExtractReport {
                        entries,
                        bytes_written,
                        warnings,
                    })
                })
                .await??;

                Ok(report)
            })
        }
    }

    impl Container for ZipContainer {
        fn name(&self) -> &'static str {
            "zip"
        }

        fn extract_boxed(
            &self,
            mut reader: Box<dyn AsyncRead + Unpin + Send>,
            options: ExtractOptions,
        ) -> Pin<Box<dyn Future<Output = Result<ExtractReport>> + Send + '_>> {
            Box::pin(async move {
            let dest = options.destination.clone();
            let policy = options.integrity.clone();

            let temp = tokio::task::spawn_blocking(|| tempfile::NamedTempFile::new())
                .await
                .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })??;

            let mut writer = tokio::fs::File::from_std(temp.reopen()?);
            tokio::io::copy(&mut reader, &mut writer).await?;
            writer.flush().await?;

            let temp_path = temp.into_temp_path();

            let report = tokio::task::spawn_blocking(move || -> Result<ExtractReport> {
                let file = std::fs::File::open(&temp_path)?;
                let mut archive = zip::ZipArchive::new(file)
                    .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
                let mut entries = 0u64;
                let mut bytes_written = 0u64;
                let mut warnings = Vec::new();

                for i in 0..archive.len() {
                    match archive.by_index(i) {
                        Ok(mut file) => {
                            let out_path = dest.join(file.mangled_name());
                            if file.name().ends_with('/') {
                                std::fs::create_dir_all(&out_path)?;
                                continue;
                            }
                            if let Some(parent) = out_path.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            match std::fs::File::create(&out_path) {
                                Ok(mut outfile) => match std::io::copy(&mut file, &mut outfile) {
                                    Ok(written) => {
                                        bytes_written += written as u64;
                                        entries += 1;
                                    }
                                    Err(e) => {
                                        warnings.push(format!("copy failed {}: {e}", out_path.display()));
                                        if !policy.skip_bad_blocks {
                                            return Err(ExtractError::IntegrityFailure { details: e.to_string() });
                                        }
                                    }
                                },
                                Err(e) => {
                                    warnings.push(format!("create failed {}: {e}", out_path.display()));
                                    if !policy.skip_bad_blocks {
                                        return Err(e.into());
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warnings.push(format!("entry {i} read failed: {e}"));
                            if !policy.skip_bad_blocks {
                                return Err(ExtractError::IntegrityFailure { details: e.to_string() });
                            }
                        }
                    }
                }

                let _ = temp_path.close();

                Ok(ExtractReport {
                    entries,
                    bytes_written,
                    warnings,
                })
            })
            .await??;

            Ok(report)
            })
        }
    }
}

pub mod pipeline {
    use super::*;
    use crate::codecs::{BrotliCodec, Codec, Lz4Codec, ZstdCodec};
    use crate::containers::{Container, ExtractOptions, ExtractReport, TarContainer, ZipContainer};
    use crate::errors::{ExtractError, Result};

    #[derive(Debug, Clone)]
    pub struct CompressOptions {
        pub source: PathBuf,
        pub destination: PathBuf,
        pub format: String,
        pub compression_level: Option<u32>,
        pub include: Option<Vec<String>>,
        pub exclude: Option<Vec<String>>,
    }

    impl Default for CompressOptions {
        fn default() -> Self {
            Self {
                source: PathBuf::from("./input"),
                destination: PathBuf::from("./output.tar.zst"),
                format: "tar.zst".to_string(),
                compression_level: None,
                include: None,
                exclude: None,
            }
        }
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CompressReport {
        pub files: u64,
        pub bytes_read: u64,
        pub bytes_written: u64,
        pub compression_ratio: f64,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct BatchExtractReport {
        pub total_archives: u64,
        pub successful: u64,
        pub failed: u64,
        pub total_files: u64,
        pub total_bytes: u64,
        pub errors: Vec<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct BatchCompressReport {
        pub total_sources: u64,
        pub successful: u64,
        pub failed: u64,
        pub total_files: u64,
        pub total_bytes_read: u64,
        pub total_bytes_written: u64,
        pub errors: Vec<String>,
    }

    #[derive(Debug, Clone)]
    pub struct BatchExtractOptions {
        pub archives: Vec<(PathBuf, PathBuf)>,
        pub extract_options: ExtractOptions,
    }

    #[derive(Debug, Clone)]
    pub struct BatchCompressOptions {
        pub sources: Vec<(PathBuf, PathBuf, String)>,
        pub compress_options: CompressOptions,
    }

    pub struct Extractor {
        containers: Vec<Arc<dyn Container>>,
    }

    impl Extractor {
        pub fn with_defaults() -> Self {
            let mut extractor = Self { containers: Vec::new() };
            extractor.register(Arc::new(TarContainer::new(Arc::new(ZstdCodec))));
            extractor.register(Arc::new(TarContainer::new(Arc::new(Lz4Codec))));
            extractor.register(Arc::new(TarContainer::new(Arc::new(BrotliCodec))));
            extractor.register(Arc::new(ZipContainer));
            extractor
        }

        pub fn register(&mut self, container: Arc<dyn Container>) {
            self.containers.push(container);
        }

        fn find(&self, name: &str) -> Option<Arc<dyn Container>> {
            self.containers
                .iter()
                .find(|c| c.name() == name)
                .map(Arc::clone)
        }

        pub async fn extract<R>(
            &self,
            format: &str,
            reader: R,
            options: ExtractOptions,
        ) -> Result<ExtractReport>
        where
            R: AsyncRead + Unpin + Send + 'static,
        {
            let Some(container) = self.find(format) else {
                return Err(ExtractError::Unsupported(format.to_string()));
            };
            container.extract_boxed(Box::new(reader), options).await
        }

        pub fn codec(&self, _name: &str) -> Option<Arc<dyn Codec>> {
            // TODO: Implement codec lookup
            None
        }

        pub async fn compress(&self, options: CompressOptions) -> Result<CompressReport> {
            use crate::codecs::compressor_from_name;
            use std::fs::File;
            use std::io::Write;

            let (codec_name, _container_name): (String, String) = if options.format.contains('.') {
                let parts: Vec<&str> = options.format.split('.').collect();
                (parts.get(1).unwrap_or(&"").to_string(), parts.get(0).unwrap_or(&"tar").to_string())
            } else {
                (options.format.clone(), "tar".to_string())
            };

            let compressor = compressor_from_name(&codec_name)
                .ok_or_else(|| ExtractError::Unsupported(codec_name.clone()))?;

            // Create tar archive in memory first
            let mut tar_data = Vec::new();
            {
                let mut tar_builder = tar::Builder::new(&mut tar_data);

                let source_path = &options.source;
                if source_path.is_dir() {
                    for entry in walkdir::WalkDir::new(source_path)
                        .into_iter()
                        .filter_map(|e| e.ok())
                    {
                        let path = entry.path();
                        if path.is_file() {
                            let rel_path = path.strip_prefix(source_path)
                                .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;

                            // Check include/exclude filters
                            if let Some(ref include) = options.include {
                                let path_str = rel_path.to_string_lossy();
                                if !include.iter().any(|pattern| path_str.contains(pattern)) {
                                    continue;
                                }
                            }
                            if let Some(ref exclude) = options.exclude {
                                let path_str = rel_path.to_string_lossy();
                                if exclude.iter().any(|pattern| path_str.contains(pattern)) {
                                    continue;
                                }
                            }

                            let mut file = File::open(path)
                                .map_err(|e| ExtractError::Io(e))?;
                            tar_builder.append_file(rel_path, &mut file)
                                .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
                        }
                    }
                } else if source_path.is_file() {
                    let file_name = source_path.file_name()
                        .ok_or_else(|| ExtractError::IntegrityFailure { details: "Invalid filename".into() })?;
                    let mut file = File::open(source_path)
                        .map_err(|e| ExtractError::Io(e))?;
                    tar_builder.append_file(file_name, &mut file)
                        .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
                }

                tar_builder.finish()
                    .map_err(|e| ExtractError::IntegrityFailure { details: e.to_string() })?;
            }

            let bytes_read = tar_data.len() as u64;
            let files = 1; // TODO: Count actual files

            // Compress the data
            let compressed = compressor.compress(&tar_data, options.compression_level)?;

            let bytes_written = compressed.len() as u64;
            let compression_ratio = if bytes_read > 0 {
                bytes_written as f64 / bytes_read as f64
            } else {
                0.0
            };

            // Write to destination
            let mut dest_file = File::create(&options.destination)
                .map_err(|e| ExtractError::Io(e))?;
            dest_file.write_all(&compressed)
                .map_err(|e| ExtractError::Io(e))?;
            dest_file.flush()
                .map_err(|e| ExtractError::Io(e))?;

            Ok(CompressReport {
                files,
                bytes_read,
                bytes_written,
                compression_ratio,
            })
        }

        pub async fn batch_extract(
            &self,
            archives: Vec<(PathBuf, PathBuf)>, // (input_path, output_dir)
            options: ExtractOptions,
        ) -> Result<BatchExtractReport> {
            use crate::format_detection;
            use tokio::io::BufReader;

            let mut report = BatchExtractReport::default();
            report.total_archives = archives.len() as u64;

            for (input_path, output_dir) in archives {
                // Auto-detect format
                let format = match format_detection::detect_format(&input_path) {
                    Ok(fmt) => fmt.as_str().to_string(),
                    Err(e) => {
                        let error_msg = format!("Failed to detect format for {}: {}", input_path.display(), e);
                        report.errors.push(error_msg);
                        report.failed += 1;
                        continue;
                    }
                };

                // Create output directory if it doesn't exist
                if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
                    let error_msg = format!("Failed to create output directory {}: {}", output_dir.display(), e);
                    report.errors.push(error_msg);
                    report.failed += 1;
                    continue;
                }

                // Extract the archive
                let mut extract_options = options.clone();
                extract_options.destination = output_dir;

                match tokio::fs::File::open(&input_path).await {
                    Ok(file) => {
                        let reader = BufReader::new(file);
                        match self.extract(&format, reader, extract_options).await {
                            Ok(result) => {
                                report.successful += 1;
                                report.total_files += result.entries;
                                report.total_bytes += result.bytes_written;
                                // Add warnings to errors list for visibility
                                for warning in result.warnings {
                                    report.errors.push(format!("{}: {}", input_path.display(), warning));
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to extract {}: {}", input_path.display(), e);
                                report.errors.push(error_msg);
                                report.failed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to open {}: {}", input_path.display(), e);
                        report.errors.push(error_msg);
                        report.failed += 1;
                    }
                }
            }

            Ok(report)
        }

        pub async fn batch_compress(
            &self,
            sources: Vec<(PathBuf, PathBuf, String)>, // (source, destination, format)
            options: CompressOptions,
        ) -> Result<BatchCompressReport> {
            let mut report = BatchCompressReport::default();
            report.total_sources = sources.len() as u64;

            for (source, destination, format) in sources {
                let mut compress_options = options.clone();
                compress_options.source = source.clone();
                compress_options.destination = destination.clone();
                compress_options.format = format.clone();

                match self.compress(compress_options).await {
                    Ok(result) => {
                        report.successful += 1;
                        report.total_files += result.files;
                        report.total_bytes_read += result.bytes_read;
                        report.total_bytes_written += result.bytes_written;
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to compress {}: {}", source.display(), e);
                        report.errors.push(error_msg);
                        report.failed += 1;
                    }
                }
            }

            Ok(report)
        }
    }
}
