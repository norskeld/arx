use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process;

use crossterm::style::Stylize;
use miette::Diagnostic;
use run_script::ScriptOptions;
use thiserror::Error;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use unindent::Unindent;

use crate::actions::State;
use crate::config::actions::*;
use crate::config::value::*;
use crate::path::{PathClean, Traverser};
use crate::spinner::Spinner;

#[derive(Debug, Diagnostic, Error)]
pub enum ActionError {
  #[error("{message}")]
  #[diagnostic(code(arx::actions::io))]
  Io {
    message: String,
    #[source]
    source: io::Error,
  },
}

impl Copy {
  pub async fn execute<P>(&self, root: P) -> miette::Result<()>
  where
    P: AsRef<Path>,
  {
    let destination = root.as_ref().join(&self.to);

    let traverser = Traverser::new(root.as_ref())
      .ignore_dirs(true)
      .contents_first(true)
      .pattern(&self.from);

    println!(
      "⋅ Copying: {}",
      format!("{} ╌╌ {}", &self.from, &self.to).dim()
    );

    for matched in traverser.iter().flatten() {
      let name = matched
        .path
        .file_name()
        .ok_or_else(|| miette::miette!("Path should end with valid file name."))?;

      let target = destination.join(name).clean();

      if !self.overwrite && target.is_file() {
        continue;
      }

      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).await.map_err(|source| {
          ActionError::Io {
            message: format!(
              "Failed to create directory structure for '{}'.",
              parent.display()
            ),
            source,
          }
        })?;

        fs::copy(&matched.path, &target).await.map_err(|source| {
          ActionError::Io {
            message: format!(
              "Failed to copy from '{}' to '{}'.",
              matched.path.display(),
              target.display()
            ),
            source,
          }
        })?;
      }

      println!("└─ {} ╌╌ {}", &matched.path.display(), &target.display());
    }

    Ok(())
  }
}

impl Move {
  pub async fn execute<P>(&self, root: P) -> miette::Result<()>
  where
    P: AsRef<Path>,
  {
    let destination = root.as_ref().join(&self.to);

    let traverser = Traverser::new(root.as_ref())
      .ignore_dirs(false)
      .contents_first(true)
      .pattern(&self.from);

    println!(
      "⋅ Moving: {}",
      format!("{} ╌╌ {}", &self.from, &self.to).dim()
    );

    for matched in traverser.iter().flatten() {
      let name = matched
        .path
        .file_name()
        .ok_or_else(|| miette::miette!("Path should end with valid file name."))?;

      let target = destination.join(name).clean();

      if !self.overwrite {
        if let Ok(true) = target.try_exists() {
          continue;
        }
      }

      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).await.map_err(|source| {
          ActionError::Io {
            message: format!(
              "Failed to create directory structure for '{}'.",
              parent.display()
            ),
            source,
          }
        })?;

        fs::rename(&matched.path, &target).await.map_err(|source| {
          ActionError::Io {
            message: format!(
              "Failed to move from '{}' to '{}'.",
              matched.path.display(),
              target.display()
            ),
            source,
          }
        })?;
      }

      println!("└─ {} ╌╌ {}", &matched.path.display(), &target.display());
    }

    Ok(())
  }
}

impl Delete {
  pub async fn execute<P>(&self, root: P) -> miette::Result<()>
  where
    P: AsRef<Path>,
  {
    let traverser = Traverser::new(root.as_ref())
      .ignore_dirs(false)
      .contents_first(false)
      .pattern(&self.target);

    println!("⋅ Deleting: {}", &self.target.clone().dim());

    for matched in traverser.iter().flatten() {
      let target = &matched.path.clean();

      if matched.is_file() {
        fs::remove_file(target).await.map_err(|source| {
          ActionError::Io {
            message: format!("Failed to delete file '{}'.", target.display()),
            source,
          }
        })?;
      } else if matched.is_dir() {
        fs::remove_dir_all(target).await.map_err(|source| {
          ActionError::Io {
            message: format!("Failed to delete directory '{}'.", target.display()),
            source,
          }
        })?;
      } else {
        continue;
      }

      println!("└─ {}", &target.display());
    }

    Ok(())
  }
}

impl Echo {
  pub async fn execute(&self, state: &State) -> miette::Result<()> {
    let message = if self.trim {
      self.message.trim()
    } else {
      &self.message
    };

    let mut message = message.unindent();

    if let Some(injects) = &self.injects {
      for inject in injects {
        if let Some(Value::String(value)) = state.get(inject) {
          message = message.replace(&format!("{{{inject}}}"), value);
        }
      }
    }

    Ok(println!("{message}"))
  }
}

impl Run {
  pub async fn execute<P>(&self, root: P, state: &State) -> miette::Result<()>
  where
    P: Into<PathBuf> + AsRef<Path>,
  {
    let mut command = self.command.clone();
    let spinner = Spinner::new();

    if let Some(injects) = &self.injects {
      for inject in injects {
        if let Some(Value::String(value)) = state.get(inject) {
          command = command.replace(&format!("{{{inject}}}"), value);
        }
      }
    }

    let name = self
      .name
      .clone()
      .or_else(|| {
        let lines = command.trim().lines().count();

        if lines > 1 {
          Some(command.trim().lines().next().unwrap().to_string() + "...")
        } else {
          Some(command.clone())
        }
      })
      .unwrap();

    let options = ScriptOptions {
      working_directory: Some(root.into()),
      ..ScriptOptions::new()
    };

    spinner.set_message(format!("{}", name.clone().grey()));

    // Actually run the script.
    let (code, output, err) = run_script::run_script!(command, options)
      .map_err(|_| miette::miette!("Failed to run script."))?;

    let has_failed = code > 0;

    // Re-format depending on the exit code.
    let name = if has_failed { name.red() } else { name.green() };

    // Stopping before printing output/errors, otherwise the spinner message won't be cleared.
    spinner.stop_with_message(format!("{name}\n",));

    if has_failed {
      if !err.is_empty() {
        eprintln!("{err}");
      }

      process::exit(1);
    }

    Ok(println!("{}", output.trim()))
  }
}

impl Prompt {
  pub async fn execute(&self, state: &mut State) -> miette::Result<()> {
    match self {
      | Self::Confirm(prompt) => prompt.execute(state).await,
      | Self::Editor(prompt) => prompt.execute(state).await,
      | Self::Input(prompt) => prompt.execute(state).await,
      | Self::Number(prompt) => prompt.execute(state).await,
      | Self::Select(prompt) => prompt.execute(state).await,
    }
  }
}

impl Replace {
  pub async fn execute<P>(&self, root: P, state: &State) -> miette::Result<()>
  where
    P: AsRef<Path>,
  {
    // If no glob pattern specified, traverse all files.
    let pattern = self.glob.clone().unwrap_or("**/*".to_string());

    let traverser = Traverser::new(root.as_ref())
      .ignore_dirs(true)
      .contents_first(true)
      .pattern(&pattern);

    if !self.replacements.is_empty() {
      let mut performed = HashSet::new();

      println!("⋅ Applying replacements:");

      for matched in traverser.iter().flatten() {
        let mut buffer = String::new();
        let mut should_write = false;

        let mut file = File::open(&matched.path).await.map_err(|source| {
          ActionError::Io {
            message: format!("Failed to open file '{}'.", &matched.path.display()),
            source,
          }
        })?;

        file.read_to_string(&mut buffer).await.map_err(|source| {
          ActionError::Io {
            message: format!("Failed to read file '{}'.", &matched.path.display()),
            source,
          }
        })?;

        for replacement in &self.replacements {
          if let Some(value) = state.get(replacement) {
            buffer = buffer.replace(&format!("{{{replacement}}}"), value.to_string().as_str());
            should_write = true;

            performed.insert(replacement.to_string());
          }
        }

        if should_write {
          let mut result = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&matched.path)
            .await
            .map_err(|source| {
              ActionError::Io {
                message: format!(
                  "Failed to open file '{}' for writing.",
                  &matched.path.display()
                ),
                source,
              }
            })?;

          result
            .write_all(buffer.as_bytes())
            .await
            .map_err(|source| {
              ActionError::Io {
                message: format!("Failed to write to the file '{}'.", &matched.path.display()),
                source,
              }
            })?;
        }
      }

      // Report whether replacements were performed or not.
      for replacement in &self.replacements {
        let state = if performed.contains(replacement) {
          "✓".green()
        } else {
          "✗".red()
        };

        println!("└─ {state} ╌ {replacement}");
      }
    }

    Ok(())
  }
}

impl Unknown {
  pub async fn execute(&self) -> miette::Result<()> {
    let name = self.name.as_str().yellow();
    let message = format!("? Unknown action: {name}").yellow();

    Ok(println!("{message}"))
  }
}
