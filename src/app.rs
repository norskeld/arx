use std::fmt;

use clap::Parser;

use crate::parser;
use crate::repository::Repository;
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

  /// Download a specific branch.
  #[clap(short, long, display_order = 3)]
  branch: Option<String>,

  /// Download at specific commit.
  #[clap(short, long, display_order = 4)]
  commit: Option<String>,

  /// Download at specific tag.
  #[clap(short, long, display_order = 5)]
  tag: Option<String>,
}

pub async fn run() -> Result<(), AppError> {
  let options = App::parse();
  let repository = parser::shortcut(&options.target)?;
  let bytes = repository.fetch().await?;

  let Repository { repo, .. } = repository;

  tar::unpack(&bytes, &options.path.unwrap_or(repo))?;

  Ok(())
}
