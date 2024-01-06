use std::fs;
use std::path::{Path, PathBuf};

use flate2::bufread::GzDecoder;
use tar::Archive;
use thiserror::Error;

#[cfg(target_os = "windows")]
const USE_XATTRS: bool = false;

#[cfg(not(target_os = "windows"))]
const USE_XATTRS: bool = true;

#[cfg(target_os = "windows")]
const USE_PERMISSIONS: bool = false;

#[cfg(not(target_os = "windows"))]
const USE_PERMISSIONS: bool = true;

#[derive(Debug, Error, PartialEq)]
pub enum UnpackError {
  #[error("Couldn't get entries from the tarball.")]
  UnableGetEntries,
  #[error("Couldn't get the entry's path.")]
  UnableGetEntryPath,
  #[error("Couldn't create the output structure.")]
  UnableCreateStructure,
  #[error("Couldn't unpack the entry.")]
  UnableUnpackEntry,
}

pub struct Unpacker {
  bytes: Vec<u8>,
}

impl Unpacker {
  pub fn new(bytes: Vec<u8>) -> Self {
    Self { bytes }
  }

  /// Unpacks the tar archive to the given [Path].
  pub fn unpack_to(&self, path: &Path) -> Result<Vec<PathBuf>, UnpackError> {
    let mut archive = Archive::new(GzDecoder::new(&self.bytes[..]));
    let mut written_paths = Vec::new();

    // Get iterator over the entries.
    let raw_entries = archive
      .entries()
      .map_err(|_| UnpackError::UnableGetEntries)?;

    // Create output structure (if necessary).
    fs::create_dir_all(&path).map_err(|_| UnpackError::UnableCreateStructure)?;

    for mut entry in raw_entries.flatten() {
      let entry_path = entry.path().map_err(|_| UnpackError::UnableGetEntryPath)?;

      let fixed_path = fix_entry_path(&entry_path, path);

      entry.set_preserve_permissions(USE_PERMISSIONS);
      entry.set_unpack_xattrs(USE_XATTRS);

      entry
        .unpack(&fixed_path)
        .map_err(|_| UnpackError::UnableUnpackEntry)?;

      written_paths.push(fixed_path);
    }

    // Deduplicate, because it **will** contain duplicates.
    written_paths.dedup();

    Ok(written_paths)
  }
}

impl From<Vec<u8>> for Unpacker {
  fn from(bytes: Vec<u8>) -> Self {
    Unpacker::new(bytes)
  }
}

/// Produces a "fixed" path for an entry.
#[inline(always)]
fn fix_entry_path(entry_path: &Path, dest_path: &Path) -> PathBuf {
  dest_path
    .components()
    .chain(entry_path.components().skip(1))
    .fold(PathBuf::new(), |acc, next| acc.join(next))
}
