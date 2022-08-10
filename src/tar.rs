use std::ffi::OsString;
use std::mem;
use std::path::{Component, Path, PathBuf};

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
pub(crate) fn unpack(bytes: &[u8], destination: &String) -> Result<Vec<PathBuf>, AppError> {
  let mut archive = Archive::new(GzDecoder::new(bytes));
  let mut written_paths = Vec::new();

  // Get iterator over the entries.
  let raw_entries = archive
    .entries()
    .map_err(|_| AppError("Couldn't get entries from the tarball.".to_string()))?;

  for mut entry in raw_entries.flatten() {
    let entry_path = entry
      .path()
      .map_err(|_| AppError("Couldn't obtain the entry's path.".to_string()))?;

    let fixed_path = fix_entry_path(&entry_path, destination)?;

    entry.set_preserve_mtime(true);
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

/// Produces a "fixed" path for an entry.
#[inline(always)]
fn fix_entry_path(entry_path: &Path, destination: &String) -> Result<PathBuf, AppError> {
  // Convert repo name from [String] to [OsString] to create a path [Component].
  let repo_name = OsString::from(destination);

  // Get the entry path components, the first one will be replaced.
  let mut components = entry_path.components().collect::<Vec<_>>();

  if !components.is_empty() {
    // Replace the first (root) component with the component containing actual repo name.
    let _ = mem::replace(&mut components[0], Component::Normal(&repo_name));

    let path = components
      .iter()
      .fold(PathBuf::new(), |acc, next| acc.join(next));

    return Ok(path);
  }

  Err(AppError(
    "Couldn't get the first component of the entry's path, because it's empty.".to_string(),
  ))
}
