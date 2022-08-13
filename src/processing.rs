use std::path::PathBuf;

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::app::AppError;
use crate::config::Replacement;

/// Given a list of unpacked files and pairs of replacement, value, perform substitutions in files.
///
/// TODO: Come up with something better, because this is so lame...
pub async fn process_replacements(
  unpacked: &[PathBuf],
  replacements: &[Replacement],
) -> Result<(), AppError> {
  for unpacked_entry in unpacked.iter() {
    let mut buffer = String::new();

    let mut file = File::open(unpacked_entry)
      .await
      .map_err(|err| AppError(err.to_string()))?;

    let metadata = file
      .metadata()
      .await
      .map_err(|err| AppError(err.to_string()))?;

    if metadata.is_file() {
      file
        .read_to_string(&mut buffer)
        .await
        .map_err(|err| AppError(err.to_string()))?;

      for Replacement { tag, .. } in replacements.iter() {
        // In `format!` macro `{` should be doubled to be properly escaped.
        let replacement_tag = format!("{{{{ {tag} }}}}");

        // TODO: This will contain value from a prompt mapped to a specific replacement tag.
        let replacement_value = "@";

        buffer = buffer.replace(&replacement_tag, replacement_value);
      }

      let mut result = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(unpacked_entry)
        .await
        .map_err(|err| AppError(err.to_string()))?;

      result
        .write_all(buffer.as_bytes())
        .await
        .map_err(|err| AppError(err.to_string()))?;
    }
  }

  Ok(())
}
