use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{Accessor, TagExt};

use crate::error::{NcmError, Result};
use crate::metadata::NcmMetadata;

/// PNG magic bytes for MIME detection.
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Write metadata tags and optional cover art to an audio file.
#[allow(clippy::missing_panics_doc)]
pub fn write_tags(path: &Path, metadata: &NcmMetadata, cover: Option<&[u8]>) -> Result<()> {
    let mut tagged_file = Probe::open(path)
        .map_err(|e| NcmError::Tag(e.to_string()))?
        .read()
        .map_err(|e| NcmError::Tag(e.to_string()))?;

    let has_primary = tagged_file.primary_tag().is_some();
    // primary_tag_mut() is guaranteed Some when primary_tag() was Some
    let tag = if has_primary {
        tagged_file.primary_tag_mut().unwrap()
    } else {
        tagged_file
            .first_tag_mut()
            .ok_or_else(|| NcmError::Tag("no tag found in file".into()))?
    };

    tag.set_title(metadata.music_name.clone());
    tag.set_artist(metadata.artist_names());
    tag.set_album(metadata.album.clone());

    if let Some(img_data) = cover {
        let mime = if img_data.starts_with(&PNG_MAGIC) {
            MimeType::Png
        } else {
            MimeType::Jpeg
        };
        let pic = Picture::unchecked(img_data.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(mime)
            .build();
        tag.push_picture(pic);
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| NcmError::Tag(e.to_string()))?;

    Ok(())
}
