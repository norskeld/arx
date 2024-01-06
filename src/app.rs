use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;

use crate::manifest::Manifest;
use crate::path::PathUtils;
use crate::repository::{Repository, RepositoryMeta};
use crate::unpacker::Unpacker;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct Cli {
  /// Repository to download.
  #[clap(name = "target")]
  pub target: String,

  /// Directory to download to.
  #[clap(name = "path")]
  pub path: Option<String>,

  /// Delete arx config after download.
  #[clap(short, long, display_order = 1)]
  pub delete: bool,

  /// Download using specified ref (branch, tag, commit).
  #[clap(short, long, display_order = 3)]
  pub meta: Option<String>,
}

pub struct App;

impl App {
  pub fn new() -> Self {
    Self
  }

  pub async fn run(&mut self) -> anyhow::Result<()> {
    // Parse CLI options.
    let options = Cli::parse();

    // Parse repository information from the CLI argument.
    let repository = Repository::from_str(&options.target)?;

    // Check if any specific meta (ref) was passed, if so, then use it; otherwise use parsed meta.
    let meta = options.meta.map_or(repository.meta(), RepositoryMeta);
    let repository = repository.with_meta(meta);

    // TODO: Check if destination already exists before downloading or performing local clone.

    // Depending on the repository type, either download and unpack or make a local clone.
    let destination = match repository {
      | Repository::Remote(remote) => {
        let name = options.path.unwrap_or(remote.repo.clone());
        let destination = PathBuf::from(name);

        // Fetch the tarball as bytes (compressed).
        let tarball = remote.fetch().await?;

        // Decompress and unpack the tarball.
        let unpacker = Unpacker::new(tarball);
        unpacker.unpack_to(&destination)?;

        destination
      },
      | Repository::Local(local) => {
        // TODO: Check if source exists and valid.
        let source = PathBuf::from(local.source.clone()).expand();

        let destination = if let Some(destination) = options.path {
          PathBuf::from(destination).expand()
        } else {
          source
            .file_name()
            .map(|name| name.into())
            .unwrap_or_default()
        };

        // Copy the directory.
        local.copy(&destination)?;
        local.checkout(&destination)?;

        // Delete inner .git.
        let inner_git = destination.join(".git");

        if inner_git.exists() {
          println!("Removing {}", inner_git.display());
          fs::remove_dir_all(inner_git)?;
        }

        // TODO: Check if source is a plain directory or git repo. If the latter, then we should
        // also do a checkout.

        destination
      },
    };

    // Now we need to read the manifest (if it is present).
    let mut manifest = Manifest::with_options(&destination);
    manifest.load()?;

    Ok(())
  }
}
