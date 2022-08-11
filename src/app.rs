use std::fmt;

use clap::Parser;

use crate::parser;
use crate::repository::{Repository, RepositoryMeta};
use crate::tar;

/// Newtype for app errors which get propagated across the app.
#[derive(Debug)]
pub struct AppError(pub String);

impl fmt::Display for AppError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{message}", message = self.0)
  }
}

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct App {
  /// Repository to download.
  #[clap(name = "target")]
  target: String,

  /// Directory to download to.
  #[clap(name = "path")]
  path: Option<String>,

  /// Init git repository.
  #[clap(short, long, display_order = 0)]
  git: bool,

  /// Remove imp config after download.
  #[clap(short, long, display_order = 1)]
  remove: bool,

  /// Do not run actions defined in the repository.
  #[clap(short, long, display_order = 2)]
  ignore: bool,

  /// Download at specific ref (branch, tag, commit).
  #[clap(short, long, display_order = 3)]
  meta: Option<String>,
}

pub async fn run() -> Result<(), AppError> {
  let options = App::parse();

  // Parse repository information from the CLI argument.
  let repository = parser::shortcut(&options.target)?;

  // Now check if any specific meta (ref) was passed, if so, then use it; otherwise use parsed meta.
  let meta = options.meta.map_or(repository.meta, RepositoryMeta);
  let repository = Repository { meta, ..repository };

  // Fetch the tarball as bytes (compressed).
  let tarball = repository.fetch().await?;

  tar::unpack(&tarball, &options.path.unwrap_or(repository.repo))?;

  Ok(())
}
