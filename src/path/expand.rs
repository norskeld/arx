use std::env::{self, VarError};

fn home() -> Option<String> {
  Some(
    dirs::home_dir()
      .map(|path| path.display().to_string())
      .unwrap_or_else(|| "~".to_string()),
  )
}

fn context(name: &str) -> Result<Option<String>, VarError> {
  match env::var(name) {
    | Ok(value) => Ok(Some(value.into())),
    | Err(VarError::NotPresent) => Ok(Some("".into())),
    | Err(err) => Err(err),
  }
}

/// Expands tilde and environment variables in given `path`.
pub fn expand(path: &str) -> String {
  shellexpand::full_with_context(path, home, context)
    .map(|expanded| expanded.to_string())
    .unwrap_or_else(|_| path.to_string())
}
