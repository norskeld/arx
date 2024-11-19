use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
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
  /// Whether to clean up on failure or not.
  pub cleanup: bool,
  /// Clean up path, will be set to the destination acquired after creating [RemoteRepository] or
  /// [LocalRepository].
  pub cleanup_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
pub enum Cli {
  /// Scaffold from a remote repository.
  #[command(visible_alias = "r")]
  Remote(RepositoryArgs),
  /// Scaffold from a local repository.
  #[command(visible_alias = "l")]
  Local(RepositoryArgs),
  /// Commands for interacting with the cache.
  #[command(visible_alias = "c")]
  Cache {
    #[command(subcommand)]
    command: CacheCommand,
  },
}

#[derive(Clone, Debug, Args)]
pub struct RepositoryArgs {
  /// Repository to use for scaffolding.
  src: String,
  /// Directory to scaffold to.
  path: Option<String>,
  /// Scaffold from a specified ref (branch, tag, or commit).
  #[arg(name = "REF", short = 'r', long = "ref")]
  meta: Option<String>,
  /// Clean up on failure. No-op if failed because target directory already exists.
  #[arg(short = 'C', long)]
  cleanup: bool,
  /// Delete config after scaffolding is complete.
  #[arg(short, long)]
  delete: Option<bool>,
  /// Skip reading config and running actions.
  #[arg(short, long)]
  skip: bool,
  /// Use cached template if available.
  #[arg(short = 'c', long, default_value = "true")]
  cache: bool,
}

#[derive(Clone, Debug, Subcommand)]
pub enum CacheCommand {
  /// List cache entries.
  List,
  /// Remove cache entries.
  Remove {
    /// List of cache entries to remove.
    entries: Vec<String>,
    /// Interactive mode.
    #[arg(short, long)]
    interactive: bool,
    /// Remove all cache entries.
    #[arg(short, long, conflicts_with_all = ["entries", "interactive"])]
    all: bool,
  },
}

#[derive(Debug)]
pub struct App {
  /// Parsed CLI options and commands.
  cli: Cli,
  /// Current state of the application.
  state: AppState,
}

impl App {
  #[allow(clippy::new_without_default)]
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
    match self.cli.clone() {
      | Cli::Remote(args) => self.scaffold_remote(args).await,
      | Cli::Local(args) => self.scaffold_local(args).await,
      | Cli::Cache { command } => self.handle_cache(command),
    }
  }

  async fn scaffold_remote(&mut self, args: RepositoryArgs) -> miette::Result<()> {
    let mut remote = RemoteRepository::new(args.src, args.meta)?;

    // Try to fetch refs early. If we can't get them, there's no point in continuing.
    remote.fetch_refs()?;

    // Try to resolve a ref to specific hash.
    let hash = remote.resolve_hash()?;

    let name = args.path.as_ref().unwrap_or(&remote.repo);
    let destination = PathBuf::from(name);

    // Cleanup on failure.
    self.state.cleanup = args.cleanup;
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
    let mut should_fetch = !args.cache;

    if args.cache {
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
        cache.write(&source, &remote.meta.to_string(), &hash, &bytes)?;
      }

      let unpacker = Unpacker::new(bytes);
      unpacker.unpack_to(&destination)?;
    } else {
      miette::bail!("Failed to scaffold: zero bytes.");
    }

    self
      .scaffold_execute(
        &destination,
        args.skip,
        ConfigOptionsOverrides { delete: args.delete },
      )
      .await
  }

  async fn scaffold_local(&mut self, args: RepositoryArgs) -> miette::Result<()> {
    let local = LocalRepository::new(args.src, args.meta);

    let destination = if let Some(destination) = args.path {
      PathBuf::from(destination)
    } else {
      local
        .source
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_default()
    };

    // Cleanup on failure.
    self.state.cleanup = args.cleanup;
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

    self
      .scaffold_execute(
        &destination,
        args.skip,
        ConfigOptionsOverrides { delete: args.delete },
      )
      .await
  }

  async fn scaffold_execute(
    &mut self,
    destination: &Path,
    should_skip: bool,
    overrides: ConfigOptionsOverrides,
  ) -> miette::Result<()> {
    if should_skip {
      println!("{}", "~ Skipping running actions".dim());
      return Ok(());
    }

    // Read the config (if it is present).
    let mut config = Config::new(destination);

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

  fn handle_cache(&mut self, command: CacheCommand) -> miette::Result<()> {
    let mut cache = Cache::init()?;

    match command {
      | CacheCommand::List => Ok(cache.list()?),
      | CacheCommand::Remove { entries, interactive, all } => {
        if all {
          cache.remove_all()
        } else if interactive {
          cache.remove_interactive()
        } else {
          cache.remove(entries)
        }
      },
    }
  }

  /// Clean up on failure.
  fn cleanup(&self) -> miette::Result<()> {
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
