# ZipX

High-throughput, resilient compression and extraction tool targeting 300MB/s single-core and 800MB/s 4-core for tar.* and zip, with pluggable codecs (zstd, lz4, brotli) and a Tauri + Svelte UI shell. Core is Rust for safety and streaming I/O.

## Features
- **Multi-format support**: tar.zst, tar.lz4, tar.br, zip, and more
- **Compression & Extraction**: Full support for both creating and extracting archives
- **Multiple codecs**: Zstandard (zstd), LZ4, and Brotli compression
- **Automatic format detection**: Detects archive type from magic bytes and file extensions
- **Batch processing**: Process multiple archives in one command
- **Integrity checking**: CRC32 and HMAC verification
- **Resilient extraction**: Skip bad blocks, retry attempts, error recovery
- **High performance**: Parallel processing with configurable concurrency
- **Cross-platform UI**: Modern Tauri + Svelte interface with auto-detection
- **CLI**: Full-featured command-line interface for automation

## Workspace
- `core`: core library (codecs, containers, resilience, scheduler, extractor/compressor orchestrator, format detection)
- `cli`: CLI wrapper around the core
- `ui`: Tauri shell with Svelte front-end (`src-tauri` Rust backend)
- Root `Cargo.toml`: workspace deps aligned across crates

## Architecture
- **Codec layer**: traits + built-ins for zstd/lz4/brotli compression and decompression; CRC/HMAC verification hooks.
- **Container layer**: tar.* streams through codec + integrity guard to avoid whole-archive buffering; zip spools to a temp file for bounded memory and supports warning/skip on per-entry failures.
- **Format Detection**: Automatic detection from magic bytes (ZIP, 7z, RAR, zstd, LZ4, Brotli, Gzip, tar) and file extensions.
- **Resilience**: `IntegrityPolicy` for crc32/hmac, retry attempts, and skip-bad-block toggles, backed by guarded readers.
- **Scheduler**: rayon thread pool for chunk/entry parallelism; Tokio for async I/O; tracing for metrics plumbing.
- **Pipeline**: `Extractor` registry picks container by format (`tar.zst`, `tar.lz4`, `tar.br`, `zip`).
- **Batch Processing**: Process multiple archives or compress multiple sources in parallel.

## CLI

### Extract archives
```bash
# Extract with auto-detection (recommended)
cargo run -p zipx-cli -- extract --input path/to/archive.tar.zst --output ./out

# Extract with specific format
cargo run -p zipx-cli -- extract --input path/to/archive.zip --output ./out --format zip

# Force auto-detection with --auto flag
cargo run -p zipx-cli -- extract --input path/to/archive.unknown --output ./out --auto
```

### Compress files/directories
```bash
# Compress a directory with zstd (default level 3)
cargo run -p zipx-cli -- compress --input ./mydir --output ./archive.tar.zst

# Maximum compression
cargo run -p zipx-cli -- compress --input ./mydir --output ./archive.tar.zst --level 20

# Fast compression with LZ4
cargo run -p zipx-cli -- compress --input ./mydir --output ./archive.tar.lz4 --format tar.lz4

# Compress with filters (include/exclude patterns)
cargo run -p zipx-cli -- compress --input ./mydir --output ./archive.tar.zst --include "*.txt,*.md" --exclude "*.log"
```

### Batch operations
```bash
# Batch extract multiple archives (auto-detected formats)
cargo run -p zipx-cli -- batch-extract --inputs archive1.tar.zst archive2.zip archive3.tar.lz4 --output-dir ./extracted

# Batch compress multiple directories
cargo run -p zipx-cli -- batch-compress --inputs dir1 dir2 dir3 --output-dir ./compressed --format tar.zst
```

## UI (Tauri + Svelte)
- Install Node deps, then `npm run tauri:dev` (dev server) or `npm run tauri:build` (bundle).
- Front-end provides both compression and extraction interfaces with:
  - Mode selection (Extract/Compress)
  - **Auto-detection format option** - automatically detects archive type
  - Real-time format detection display
  - Format selection (auto, tar.zst, tar.lz4, tar.br, zip)
  - Compression level control (1-20)
  - Real-time progress and throughput display
  - Warning and error reporting

## Building
```bash
# Build all packages
cargo build --release

# Build specific packages
cargo build --release -p zipx-core
cargo build --release -p zipx-cli

# Build UI (from ui directory)
cd ui
npm install
npm run tauri build
```

## Usage Examples

### CLI Examples
```bash
# Auto-detection extraction (recommended)
./target/release/zipx-cli extract -i backup.tar.zst -o ./restored

# Maximum compression
./target/release/zipx-cli compress -i mydata -o backup.tar.zst --level 20

# Fast compression with LZ4
./target/release/zipx-cli compress -i mydata -o backup.tar.lz4 --format tar.lz4

# Batch process archives
./target/release/zipx-cli batch-extract -i *.tar.zst --output-dir ./extracted

# Parallel extraction (4 threads)
./target/release/zipx-cli extract -i bigfile.tar.zst -o ./out --concurrency 4
```

## Format Detection

ZipX automatically detects archive formats using:
- **Magic bytes**: Reads the first few bytes to identify format
- **File extensions**: Falls back to extension detection

Supported formats:
- **Compressed**: tar.zst, tar.lz4, tar.br, tar.gz, tgz
- **Containers**: tar, zip
- **Detected**: 7z, rar (for future expansion)

## Performance Tips
- Use **LZ4** for fastest compression/decompression
- Use **zstd** for best compression ratio with good speed
- Use **Brotli** for maximum compression (slower)
- Increase **concurrency** for multi-core systems
- Use **batch operations** for multiple files

## Next steps
- Add chunked/parallel decode for zip (per-entry concurrency, optional mmap/temp reuse) and per-block checksum/hmac propagation.
- Wire password handling + prompt flow; extend retries to codec/container layer with redundant-block fallbacks.
- Extend container coverage (7z, rar) and codecs (lzma2, ppmd, zstd dict/par, zstd dicts/par).
- Add progress reporting callbacks for real-time UI updates
- Perf harness (throughput + CPU/memory) and fault-injection tests for corruption/password/CRC errors.
- Surface tracing/telemetry to UI (progress, throughput, CPU/mem snapshots) and expose policy toggles (skip/retry/hmac).
