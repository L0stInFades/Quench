#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use zipx_core::containers::{ExtractOptions, ExtractReport};
use zipx_core::format_detection;
use zipx_core::pipeline::{CompressOptions, CompressReport, Extractor};
use zipx_core::resilience::IntegrityPolicy;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExternalExtractReport {
    entries: u64,
    bytes_written: u64,
    warnings: Vec<String>,
}

fn resource_7za_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let base = exe
        .parent()
        .ok_or_else(|| "Unable to resolve app directory".to_string())?;
    let candidate = base.join("tools").join("7za.exe");
    if candidate.exists() {
        return Ok(candidate);
    }
    let alt = base.parent().map(|p| p.join("tools").join("7za.exe"));
    if let Some(path) = alt {
        if path.exists() {
            return Ok(path);
        }
    }
    Ok(candidate)
}

fn run_7za_extract(archive: &Path, destination: &Path) -> Result<ExternalExtractReport, String> {
    let exe = resource_7za_path()?;
    if !exe.exists() {
        return Err("7za.exe is missing from app resources".to_string());
    }
    let output = Command::new(exe)
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", destination.display()))
        .arg(archive)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("7z extract failed: {}", stderr.trim()));
    }

    Ok(ExternalExtractReport {
        entries: 0,
        bytes_written: 0,
        warnings: Vec::new(),
    })
}

fn run_7za_compress(source: &Path, destination: &Path) -> Result<CompressReport, String> {
    let exe = resource_7za_path()?;
    if !exe.exists() {
        return Err("7za.exe is missing from app resources".to_string());
    }
    let output = Command::new(exe)
        .arg("a")
        .arg("-t7z")
        .arg(destination)
        .arg(source)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("7z compress failed: {}", stderr.trim()));
    }

    let bytes_written = std::fs::metadata(destination)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(CompressReport {
        files: 0,
        bytes_read: 0,
        bytes_written,
        compression_ratio: 0.0,
    })
}

#[tauri::command]
async fn detect_format(path: String) -> Result<String, String> {
    let path_obj = std::path::PathBuf::from(path);
    format_detection::detect_format(&path_obj)
        .map(|fmt| fmt.as_str().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn extract_archive(
    path: String,
    destination: String,
    format: String,
) -> Result<ExtractReport, String> {
    // Auto-detect format if "auto" is specified
    let detected_format = if format == "auto" {
        let path_obj = std::path::PathBuf::from(&path);
        match format_detection::detect_format(&path_obj) {
            Ok(fmt) => fmt.as_str().to_string(),
            Err(_) => format,
        }
    } else {
        format
    };

    if detected_format == "7z" || detected_format == "rar" {
        let report = run_7za_extract(Path::new(&path), Path::new(&destination))?;
        return Ok(ExtractReport {
            entries: report.entries,
            bytes_written: report.bytes_written,
            warnings: report.warnings,
        });
    }

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| e.to_string())?;
    let reader = tokio::io::BufReader::new(file);
    let mut options = ExtractOptions::default();
    options.destination = std::path::PathBuf::from(destination);
    options.integrity = IntegrityPolicy::default();
    let extractor = Extractor::with_defaults();
    extractor
        .extract(&detected_format, reader, options)
        .await
        .map(|report| report)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn compress_archive(
    source: String,
    destination: String,
    format: String,
    level: Option<u32>,
) -> Result<CompressReport, String> {
    if format == "7z" {
        return run_7za_compress(Path::new(&source), Path::new(&destination));
    }
    if format == "rar" {
        return Err("RAR compression is not supported".to_string());
    }
    let mut options = CompressOptions::default();
    options.source = std::path::PathBuf::from(source);
    options.destination = std::path::PathBuf::from(destination);
    options.format = format;
    options.compression_level = level;
    let extractor = Extractor::with_defaults();
    extractor
        .compress(options)
        .await
        .map(|report| report)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn main() {
    tauri::Builder::<tauri::Wry>::new()
        .invoke_handler(tauri::generate_handler![
            detect_format,
            extract_archive,
            compress_archive,
            get_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
