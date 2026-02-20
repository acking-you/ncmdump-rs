#![allow(unsafe_code, private_interfaces, non_snake_case)]

use std::ffi::{CStr, c_char, c_int};
use std::path::{Path, PathBuf};

use ncmdump::{NcmFile, NcmMetadata};

struct NeteaseCrypt {
    path: PathBuf,
    dump_path: Option<PathBuf>,
    metadata: Option<NcmMetadata>,
    cover: Option<Vec<u8>>,
    key_box: [u8; 256],
    audio_offset: u64,
    format: ncmdump::AudioFormat,
}

/// # Safety
/// `path` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CreateNeteaseCrypt(path: *const c_char) -> *mut NeteaseCrypt {
    std::panic::catch_unwind(|| {
        let c_str = unsafe { CStr::from_ptr(path) };
        let Ok(path_str) = c_str.to_str() else {
            return std::ptr::null_mut();
        };
        let p = Path::new(path_str);
        let Ok(mut file) = std::fs::File::open(p) else {
            return std::ptr::null_mut();
        };
        let Ok(ncm) = NcmFile::parse(&mut file) else {
            return std::ptr::null_mut();
        };
        let handle = Box::new(NeteaseCrypt {
            path: p.to_path_buf(),
            dump_path: None,
            metadata: ncm.metadata,
            cover: ncm.cover_image,
            key_box: ncm.key_box,
            audio_offset: ncm.audio_offset,
            format: ncm.format,
        });
        Box::into_raw(handle)
    })
    .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `handle` must be a valid pointer from `CreateNeteaseCrypt`.
/// `output_path` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Dump(handle: *mut NeteaseCrypt, output_path: *const c_char) -> c_int {
    std::panic::catch_unwind(|| {
        if handle.is_null() {
            return 1;
        }
        let nc = unsafe { &mut *handle };
        let out_dir = if output_path.is_null() {
            nc.path.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            let c_str = unsafe { CStr::from_ptr(output_path) };
            let Ok(s) = c_str.to_str() else { return 1 };
            PathBuf::from(s)
        };

        let stem = nc.path.file_stem().unwrap_or_default();
        let ext = nc.format.extension();
        let dump_path = out_dir.join(format!("{}.{ext}", stem.to_string_lossy()));

        let Ok(mut infile) = std::fs::File::open(&nc.path) else {
            return 1;
        };

        let ncm = NcmFile::from_parts(nc.key_box, nc.audio_offset);

        let Ok(outfile) = std::fs::File::create(&dump_path) else {
            return 1;
        };
        let mut writer = std::io::BufWriter::new(outfile);
        if ncm.dump_audio(&mut infile, &mut writer).is_err() {
            return 1;
        }
        nc.dump_path = Some(dump_path);
        0
    })
    .unwrap_or(1)
}

/// # Safety
/// `handle` must be a valid pointer from `CreateNeteaseCrypt`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn FixMetadata(handle: *mut NeteaseCrypt) {
    let _ = std::panic::catch_unwind(|| {
        if handle.is_null() {
            return;
        }
        let nc = unsafe { &*handle };
        let Some(dump_path) = &nc.dump_path else {
            return;
        };
        let Some(meta) = &nc.metadata else {
            return;
        };
        let _ = ncmdump::tag_write(dump_path, meta, nc.cover.as_deref());
    });
}

/// # Safety
/// `handle` must be a valid pointer from `CreateNeteaseCrypt`, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DestroyNeteaseCrypt(handle: *mut NeteaseCrypt) {
    if !handle.is_null() {
        let _ = std::panic::catch_unwind(|| {
            drop(unsafe { Box::from_raw(handle) });
        });
    }
}
