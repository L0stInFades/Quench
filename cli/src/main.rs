use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tokio::io::BufReader;
use tracing_subscriber::EnvFilter;
use zipx_core::containers::ExtractOptions;
use zipx_core::format_detection;
use zipx_core::pipeline::{CompressOptions, Extractor};
use zipx_core::resilience::IntegrityPolicy;

#[derive(Parser)]
#[command(name = "zipx", version = "0.1.0", author = "ZipX Team", about = "High-throughput extractor CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract an archive to a destination directory
    Extract {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "auto")]
        format: String,
        #[arg(long, default_value = "zstd")]
        codec: String,
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
        #[arg(long, help = "Auto-detect format from file")]
        auto: bool,
    },
    /// Compress files/directories into an archive
    Compress {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "tar.zst")]
        format: String,
        #[arg(long)]
        level: Option<u32>,
        #[arg(long)]
        include: Option<Vec<String>>,
        #[arg(long)]
        exclude: Option<Vec<String>>,
    },
    /// Batch extract multiple archives
    BatchExtract {
        #[arg(short, long)]
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output_dir: PathBuf,
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
    },
    /// Batch compress multiple sources
    BatchCompress {
        #[arg(short, long)]
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output_dir: PathBuf,
        #[arg(long, default_value = "tar.zst")]
        format: String,
        #[arg(long)]
        level: Option<u32>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("zipx_core=info".parse()?))
        .init();

    let args = Cli::parse();
    let extractor = Extractor::with_defaults();

    match args.command {
        Commands::Extract { input, output, format, concurrency, auto, .. } => {
            // Auto-detect format if requested or format is "auto"
            let detected_format = if auto || format == "auto" {
                match format_detection::detect_format(&input) {
                    Ok(fmt) => {
                        println!("Detected format: {}", fmt.as_str());
                        fmt.as_str().to_string()
                    }
                    Err(e) => {
                        eprintln!("Warning: Could not auto-detect format: {}", e);
                        eprintln!("Falling back to specified format: {}", format);
                        format
                    }
                }
            } else {
                format
            };

            let file = tokio::fs::File::open(&input).await?;
            let reader = BufReader::new(file);
            let mut options = ExtractOptions::default();
            options.destination = output;
            options.concurrency = concurrency;
            options.integrity = IntegrityPolicy::strict();
            let report = extractor.extract(&detected_format, reader, options).await?;
            println!("Extracted {} entries ({} bytes)", report.entries, report.bytes_written);
            if !report.warnings.is_empty() {
                eprintln!("Warnings ({}):", report.warnings.len());
                for w in report.warnings {
                    eprintln!("- {w}");
                }
            }
        }
        Commands::Compress { input, output, format, level, include, exclude } => {
            let mut options = CompressOptions::default();
            options.source = input;
            options.destination = output;
            options.format = format;
            options.compression_level = level;
            options.include = include;
            options.exclude = exclude;
            let report = extractor.compress(options).await?;
            println!("Compressed {} files ({} bytes -> {} bytes, ratio: {:.2}%)",
                report.files,
                report.bytes_read,
                report.bytes_written,
                report.compression_ratio * 100.0
            );
        }
        Commands::BatchExtract { inputs, output_dir, concurrency } => {
            if inputs.is_empty() {
                eprintln!("Error: No input files specified");
                return Ok(());
            }

            let mut extract_options = ExtractOptions::default();
            extract_options.concurrency = concurrency;
            extract_options.integrity = IntegrityPolicy::strict();

            // Create archive list with output directories
            let archives: Vec<_> = inputs.into_iter().map(|input| {
                let output = output_dir.join(input.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .replace(&format!(".{}", input.extension().unwrap_or_default().to_string_lossy()), ""));
                (input, output)
            }).collect();

            println!("Batch extracting {} archives...", archives.len());
            let report = extractor.batch_extract(archives, extract_options).await?;

            println!("Batch extraction complete:");
            println!("  Total: {}", report.total_archives);
            println!("  Successful: {}", report.successful);
            println!("  Failed: {}", report.failed);
            println!("  Total files extracted: {}", report.total_files);
            println!("  Total bytes: {}", report.total_bytes);

            if !report.errors.is_empty() {
                eprintln!("\nErrors ({}):", report.errors.len());
                for error in &report.errors {
                    eprintln!("- {}", error);
                }
            }
        }
        Commands::BatchCompress { inputs, output_dir, format, level } => {
            if inputs.is_empty() {
                eprintln!("Error: No input files specified");
                return Ok(());
            }

            let mut compress_options = CompressOptions::default();
            compress_options.format = format;
            compress_options.compression_level = level;

            // Create source list with output paths
            let sources: Vec<_> = inputs.into_iter().map(|input| {
                let output = output_dir.join(format!("{}.{}", input.file_name()
                    .unwrap_or_default()
                    .to_string_lossy(), compress_options.format));
                let format_str = compress_options.format.clone();
                (input, output, format_str)
            }).collect();

            println!("Batch compressing {} sources...", sources.len());
            let report = extractor.batch_compress(sources, compress_options).await?;

            println!("Batch compression complete:");
            println!("  Total: {}", report.total_sources);
            println!("  Successful: {}", report.successful);
            println!("  Failed: {}", report.failed);
            println!("  Total files processed: {}", report.total_files);
            println!("  Total bytes read: {}", report.total_bytes_read);
            println!("  Total bytes written: {}", report.total_bytes_written);

            if !report.errors.is_empty() {
                eprintln!("\nErrors ({}):", report.errors.len());
                for error in &report.errors {
                    eprintln!("- {}", error);
                }
            }
        }
    }

    Ok(())
}
