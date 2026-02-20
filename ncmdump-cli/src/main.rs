use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "ncmdump", version, about = "Convert NCM files to MP3/FLAC")]
struct Cli {
    /// NCM files to convert
    files: Vec<PathBuf>,

    /// Process all NCM files in directory
    #[arg(short, long, value_name = "PATH")]
    directory: Option<PathBuf>,

    /// Recursive directory traversal (with -d)
    #[arg(short, long)]
    recursive: bool,

    /// Output directory
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Remove source file after successful conversion
    #[arg(short = 'm', long = "remove")]
    remove: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut files: Vec<PathBuf> = cli.files;

    if let Some(dir) = &cli.directory {
        if cli.recursive {
            for entry in WalkDir::new(dir)
                .into_iter()
                .filter_map(std::result::Result::ok)
            {
                if entry.path().extension().is_some_and(|e| e == "ncm") {
                    files.push(entry.into_path());
                }
            }
        } else {
            for entry in std::fs::read_dir(dir).context("failed to read directory")? {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "ncm") {
                    files.push(path);
                }
            }
        }
    }

    if files.is_empty() {
        eprintln!("No NCM files specified. Use --help for usage.");
        std::process::exit(1);
    }

    let output_dir = cli.output.as_deref();

    for file in &files {
        match ncmdump::convert(file, output_dir) {
            Ok(out) => {
                println!("{} -> {}", file.display(), out.display());
                if cli.remove {
                    if let Err(e) = std::fs::remove_file(file) {
                        eprintln!("warning: failed to remove {}: {e}", file.display());
                    }
                }
            }
            Err(e) => {
                eprintln!("error: {}: {e}", file.display());
            }
        }
    }

    Ok(())
}
