use std::fmt;
use std::path::PathBuf;

use clap::Parser;

use crate::config::{self, Action};
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

  // Get destination path.
  let destination = options
    .path
    .map(PathBuf::from)
    .unwrap_or(PathBuf::from(repository.repo));

  // Decompress and unpack the tarball.
  tar::unpack(&tarball, &destination)?;

  // Read the kdl config.
  let arx_config = config::resolve_arx_config(&destination)?;

  // Get replacements and actions.
  let replacements = config::get_replacements(&arx_config);
  let actions = config::get_actions(&arx_config);

  replacements.map(|items| {
    items.iter().for_each(|item| {
      let tag = &item.tag;
      let description = &item.description;

      println!("{tag} = {description}");
    })
  });

  actions.map(|action| {
    match action {
      | Action::Suite(suites) => {
        let (resolved, unresolved) = config::resolve_requirements(&suites);

        println!("-- Action suites:");
        println!("Resolved: {resolved:#?}");
        println!("Unresolved: {unresolved:#?}");
      },
      | Action::Single(actions) => {
        println!("-- Actions:");
        println!("Resolved: {actions:#?}");
      },
    }
  });

  Ok(())
}
