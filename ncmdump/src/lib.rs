mod cipher;
mod decoder;
pub mod error;
mod metadata;
mod tag;

pub use decoder::{AudioFormat, NcmFile};
pub use error::{NcmError, Result};
pub use metadata::NcmMetadata;
pub use tag::write_tags as tag_write;

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// Convert an NCM file to a standard audio file (MP3/FLAC).
///
/// Returns the path to the output file.
pub fn convert(input: &Path, output_dir: Option<&Path>) -> Result<PathBuf> {
    let mut file = File::open(input)?;
    let ncm = NcmFile::parse(&mut file)?;

    let stem = input.file_stem().unwrap_or_default();
    let ext = ncm.format.extension();
    let out_dir = output_dir.unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")));
    let output_path = out_dir.join(format!("{}.{ext}", stem.to_string_lossy()));

    {
        let out_file = File::create(&output_path)?;
        let mut writer = BufWriter::new(out_file);
        ncm.dump_audio(&mut file, &mut writer)?;
    }

    if let Some(meta) = &ncm.metadata {
        tag::write_tags(&output_path, meta, ncm.cover_image.as_deref())?;
    }

    Ok(output_path)
}
