use std::fs;
use std::path::{Path, PathBuf};

use flate2::bufread::GzDecoder;
use tar::Archive;

use crate::app::AppError;

#[cfg(target_os = "windows")]
const USE_XATTRS: bool = false;

#[cfg(not(target_os = "windows"))]
const USE_XATTRS: bool = true;

#[cfg(target_os = "windows")]
const USE_PERMISSIONS: bool = false;

#[cfg(not(target_os = "windows"))]
const USE_PERMISSIONS: bool = true;

/// Unpacks a given tar archive.
pub(crate) fn unpack(bytes: &[u8], dest: &String) -> Result<Vec<PathBuf>, AppError> {
  let mut archive = Archive::new(GzDecoder::new(bytes));
  let mut written_paths = Vec::new();
  let dest_path = PathBuf::from(dest);

  // Get iterator over the entries.
  let raw_entries = archive
    .entries()
    .map_err(|_| AppError("Couldn't get entries from the tarball.".to_string()))?;

  // Create output structure (if necessary).
  create_output_structure(&dest_path)?;

  for mut entry in raw_entries.flatten() {
    let entry_path = entry
      .path()
      .map_err(|_| AppError("Couldn't get the entry's path.".to_string()))?;

    let fixed_path = fix_entry_path(&entry_path, &dest_path);

    entry.set_preserve_permissions(USE_PERMISSIONS);
    entry.set_unpack_xattrs(USE_XATTRS);

    entry
      .unpack(&fixed_path)
      // Side effect: collecting written paths for logging purposes.
      .inspect(|_| written_paths.push(fixed_path))
      .map_err(|_| AppError("Couldn't unpack the entry.".to_string()))?;
  }

  // Deduplicate, because it **will** contain duplicates.
  written_paths.dedup();

  Ok(written_paths)
}

/// Recursively creates the output structure if there's more than 1 component in the destination
/// path AND if the destination path does not exist.
#[inline(always)]
fn create_output_structure(dest_path: &PathBuf) -> Result<(), AppError> {
  // FIXME: The use of `exists` method here is a bit worrisome, since it can open possibilities for
  //  TOCTOU attacks, so should probably replace with `try_exists`.
  if dest_path.iter().count().gt(&1) && !dest_path.exists() {
    fs::create_dir_all(&dest_path)
      .map_err(|_| AppError("Couldn't create the output structure.".to_string()))?;
  }

  Ok(())
}

/// Produces a "fixed" path for an entry.
#[inline(always)]
fn fix_entry_path(entry_path: &Path, dest_path: &PathBuf) -> PathBuf {
  let dest_path = PathBuf::from(dest_path);

  dest_path
    .components()
    .chain(entry_path.components().skip(1))
    .fold(PathBuf::new(), |acc, next| acc.join(next))
}
