use std::fs;
use std::io;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use crossterm::style::Stylize;
use miette::Diagnostic;
use thiserror::Error;

use crate::actions::Executor;
use crate::config::{Config, ConfigOptionsOverrides};
use crate::repository::{LocalRepository, RemoteRepository};
use crate::unpacker::Unpacker;

#[derive(Debug, Diagnostic, Error)]
pub enum AppError {
  #[error("{message}")]
  #[diagnostic(code(actions::app::io))]
  Io {
    message: String,
    #[source]
    source: io::Error,
  },
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
  #[command(subcommand)]
  pub command: BaseCommand,
}

#[derive(Debug, Subcommand)]
pub enum BaseCommand {
  /// Scaffold from a remote repository.
  #[command(visible_alias = "r")]
  Remote {
    /// Repository to use for scaffolding.
    src: String,

    /// Directory to scaffold to.
    path: Option<String>,

    /// Scaffold from a specified ref (branch, tag, or commit).
    #[arg(name = "REF", short = 'r', long = "ref")]
    meta: Option<String>,

    /// Delete arx config after scaffolding.
    #[arg(short, long)]
    delete: Option<bool>,
  },
  /// Scaffold from a local repository.
  #[command(visible_alias = "l")]
  Local {
    /// Repository to use for scaffolding.
    src: String,

    /// Directory to scaffold to.
    path: Option<String>,

    /// Scaffold from a specified ref (branch, tag, or commit).
    #[arg(name = "REF", short = 'r', long = "ref")]
    meta: Option<String>,

    /// Delete arx config after scaffolding.
    #[arg(short, long)]
    delete: Option<bool>,
  },
}

#[derive(Debug)]
pub struct App {
  cli: Cli,
}

impl App {
  pub fn new() -> Self {
    Self { cli: Cli::parse() }
  }

  pub async fn run(self) -> miette::Result<()> {
    // Slightly tweak miette.
    miette::set_hook(Box::new(|_| {
      Box::new(
        miette::MietteHandlerOpts::new()
          .terminal_links(false)
          .context_lines(3)
          .tab_width(4)
          .build(),
      )
    }))?;

    // Load the config.
    let config = match self.cli.command {
      | BaseCommand::Remote { src, path, meta, delete } => {
        let options = ConfigOptionsOverrides { delete };
        Self::remote(src, path, meta, options).await?
      },
      | BaseCommand::Local { src, path, meta, delete } => {
        let options = ConfigOptionsOverrides { delete };
        Self::local(src, path, meta, options).await?
      },
    };

    // Create executor and kick off execution.
    let executor = Executor::new(config);
    executor.execute().await?;

    Ok(())
  }

  /// Preparation flow for remote repositories.
  async fn remote(
    src: String,
    path: Option<String>,
    meta: Option<String>,
    overrides: ConfigOptionsOverrides,
  ) -> miette::Result<Config> {
    // Parse repository.
    let remote = RemoteRepository::new(src, meta)?;

    let name = path.unwrap_or(remote.repo.clone());
    let destination = PathBuf::from(name);

    // Check if destination already exists before downloading.
    if let Ok(true) = &destination.try_exists() {
      miette::bail!(
        "Failed to scaffold: '{}' already exists.",
        destination.display()
      );
    }

    // Fetch the tarball as bytes (compressed).
    let tarball = remote.fetch().await?;

    // Decompress and unpack the tarball.
    let unpacker = Unpacker::new(tarball);
    unpacker.unpack_to(&destination)?;

    // Now we need to read the config (if it is present).
    let mut config = Config::new(&destination);

    config.load()?;
    config.override_with(overrides);

    Ok(config)
  }

  /// Preparation flow for local repositories.
  async fn local(
    src: String,
    path: Option<String>,
    meta: Option<String>,
    overrides: ConfigOptionsOverrides,
  ) -> miette::Result<Config> {
    // Create repository.
    let local = LocalRepository::new(src, meta);

    let destination = if let Some(destination) = path {
      PathBuf::from(destination)
    } else {
      local
        .source
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_default()
    };

    // Check if destination already exists before performing local clone.
    if let Ok(true) = &destination.try_exists() {
      miette::bail!(
        "Failed to scaffold: '{}' already exists.",
        destination.display()
      );
    }

    // Copy the directory.
    local.copy(&destination)?;

    println!("{}", "~ Cloned repository".dim());

    // Checkout the ref.
    local.checkout(&destination)?;

    println!("{} {}", "~ Checked out ref:".dim(), local.meta.0.dim());

    // Delete inner .git directory.
    let inner_git = destination.join(".git");

    if let Ok(true) = inner_git.try_exists() {
      fs::remove_dir_all(inner_git).map_err(|source| {
        AppError::Io {
          message: "Failed to remove inner .git directory.".to_string(),
          source,
        }
      })?;

      println!("{}", "~ Removed inner .git directory\n".dim());
    }

    // Now we need to read the config (if it is present).
    let mut config = Config::new(&destination);

    config.load()?;
    config.override_with(overrides);

    Ok(config)
  }
}

impl Default for App {
  fn default() -> Self {
    Self::new()
  }
}
