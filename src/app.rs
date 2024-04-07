use std::fs;
use std::io;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use crossterm::style::Stylize;
use miette::Diagnostic;
use thiserror::Error;

use crate::actions::Executor;
use crate::cache::Cache;
use crate::config::{Config, ConfigOptionsOverrides};
use crate::report;
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

#[derive(Debug, Default)]
pub struct AppState {
  /// Whether to cleanup on failure or not.
  pub cleanup: bool,
  /// Cleanup path, will be set to the destination acquired after creating [RemoteRepository] or
  /// [LocalRepository].
  pub cleanup_path: Option<PathBuf>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
  #[command(subcommand)]
  pub command: BaseCommand,

  /// Cleanup on failure. No-op if failed because target directory already exists.
  #[arg(global = true, short = 'C', long)]
  cleanup: bool,

  /// Delete arx config after scaffolding is complete.
  #[arg(global = true, short, long)]
  delete: Option<bool>,

  /// Use cached template if available.
  #[arg(global = true, short = 'c', long, default_value = "true")]
  cache: bool,

  /// Skip running actions.
  #[arg(global = true, short, long)]
  skip: bool,
}

#[derive(Clone, Debug, Subcommand)]
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
  },
}

#[derive(Debug)]
pub struct App {
  cli: Cli,
  state: AppState,
}

impl App {
  pub fn new() -> Self {
    Self {
      cli: Cli::parse(),
      state: AppState::default(),
    }
  }

  /// Runs the app and prints any errors.
  pub async fn run(&mut self) {
    miette::set_hook(Box::new(|_| {
      Box::new(
        miette::MietteHandlerOpts::new()
          .terminal_links(false)
          .context_lines(3)
          .tab_width(4)
          .build(),
      )
    }))
    .expect("Failed to set up the miette hook");

    let scaffold_res = self.scaffold().await;

    if scaffold_res.is_err() {
      report::try_report(scaffold_res);
      report::try_report(self.cleanup());
    }
  }

  /// Kicks of the scaffolding process.
  pub async fn scaffold(&mut self) -> miette::Result<()> {
    // Build override options.
    let overrides = ConfigOptionsOverrides { delete: self.cli.delete };

    // Cleanup on failure.
    self.state.cleanup = self.cli.cleanup;

    // Retrieve the template.
    let destination = match self.cli.command.clone() {
      | BaseCommand::Remote { src, path, meta } => {
        let mut remote = RemoteRepository::new(src, meta)?;

        // Try to fetch refs early. If we can't get them, there's no point in continuing.
        remote.fetch_refs()?;

        // Try to resolve a ref to specific hash.
        let hash = remote.resolve_hash()?;

        let name = path.as_ref().unwrap_or(&remote.repo);
        let destination = PathBuf::from(name);

        // Set cleanup path to the destination.
        self.state.cleanup_path = Some(destination.clone());

        // Check if destination already exists before downloading.
        if let Ok(true) = &destination.try_exists() {
          // We do not want to remove already existing directory.
          self.state.cleanup = false;

          miette::bail!(
            "Failed to scaffold: '{}' already exists.",
            destination.display()
          );
        }

        let mut cache = Cache::init()?;
        let mut bytes = None;

        let source = remote.get_source();
        let mut should_fetch = !self.cli.cache;

        if self.cli.cache {
          println!("{}", "~ Attempting to read from cache".dim());

          if let Some(cached) = cache.read(&source, &hash)? {
            println!("{}", "~ Found in cache, reading".dim());
            bytes = Some(cached);
          } else {
            println!("{}", "~ Nothing found in cache, fetching".dim());
            should_fetch = true;
          }
        }

        if should_fetch {
          bytes = Some(remote.fetch().await?);
        }

        // Decompress and unpack the tarball. If somehow the tarball is empty, bail.
        if let Some(bytes) = bytes {
          if should_fetch {
            cache.write(&source, &hash, &bytes)?;
          }

          let unpacker = Unpacker::new(bytes);
          unpacker.unpack_to(&destination)?;
        } else {
          miette::bail!("Failed to scaffold: zero bytes.");
        }

        destination
      },
      | BaseCommand::Local { src, path, meta } => {
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

        // Set cleanup path to the destination.
        self.state.cleanup_path = Some(destination.clone());

        // Check if destination already exists before performing local clone.
        if let Ok(true) = &destination.try_exists() {
          // We do not want to remove already existing directory.
          self.state.cleanup = false;

          miette::bail!(
            "Failed to scaffold: '{}' already exists.",
            destination.display()
          );
        }

        // Copy the directory.
        local.copy(&destination)?;

        // .git directory path.
        let inner_git = destination.join(".git");

        // If we copied a repository, we also need to checkout the ref.
        if let Ok(true) = inner_git.try_exists() {
          println!("{}", "~ Cloned repository".dim());

          // Checkout the ref.
          local.checkout(&destination)?;

          println!("{} {}", "~ Checked out ref:".dim(), local.meta.0.dim());

          // At last, remove the inner .git directory.
          fs::remove_dir_all(inner_git).map_err(|source| {
            AppError::Io {
              message: "Failed to remove inner .git directory.".to_string(),
              source,
            }
          })?;

          println!("{}", "~ Removed inner .git directory".dim());
        } else {
          println!("{}", "~ Copied directory".dim());
        }

        destination
      },
    };

    if self.cli.skip {
      println!("{}", "~ Skipping running actions".dim());
      return Ok(());
    }

    // Read the config (if it is present).
    let mut config = Config::new(&destination);

    if config.load()? {
      println!();

      config.override_with(overrides);

      // Create executor and kick off execution.
      let executor = Executor::new(config);

      executor.execute().await
    } else {
      Ok(())
    }
  }

  /// Cleanup on failure.
  pub fn cleanup(&self) -> miette::Result<()> {
    if self.state.cleanup {
      if let Some(destination) = &self.state.cleanup_path {
        fs::remove_dir_all(destination).map_err(|source| {
          AppError::Io {
            message: format!("Failed to remove directory: '{}'.", destination.display()),
            source,
          }
        })?;
      }
    }

    Ok(())
  }
}

impl Default for App {
  fn default() -> Self {
    Self::new()
  }
}
