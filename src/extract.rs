use std::io::Read;
use std::path::Path;
use std::borrow::Cow;
use std::rc::Rc;

use flate2::bufread::GzDecoder;
use tar::Archive;

use crate::app::AppError;

pub(crate) fn extract(bytes: &[u8]) -> Result<(), AppError> {
  let tar = GzDecoder::new(bytes);
  let mut archive = Archive::new(tar);

  let entries = archive
    .entries()
    .map_err(|_| AppError(format!("Couldn't get entries from the tarball.")))?;

  Ok(())
}
