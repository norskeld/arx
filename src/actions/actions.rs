use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::style::Stylize;
use run_script::ScriptOptions;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use unindent::Unindent;

use crate::actions::{State, Value};
use crate::manifest::actions::*;
use crate::path::PathClean;
use crate::path::Traverser;
use crate::spinner::Spinner;

impl Copy {
  pub async fn execute<P>(&self, root: P) -> anyhow::Result<()>
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
        .expect("Path should end with valid file name");

      let target = destination.join(name).clean();

      if !self.overwrite && target.is_file() {
        continue;
      }

      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).await?;
        fs::copy(&matched.path, &target).await?;
      }

      println!("└─ {} ╌╌ {}", &matched.path.display(), &target.display());
    }

    Ok(())
  }
}

impl Move {
  pub async fn execute<P>(&self, root: P) -> anyhow::Result<()>
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
        .expect("Path should end with valid file name");

      let target = destination.join(name).clean();

      if !self.overwrite {
        if let Ok(true) = target.try_exists() {
          continue;
        }
      }

      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).await?;
        fs::rename(&matched.path, &target).await?;
      }

      println!("└─ {} ╌╌ {}", &matched.path.display(), &target.display());
    }

    Ok(())
  }
}

impl Delete {
  pub async fn execute<P>(&self, root: P) -> anyhow::Result<()>
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
        fs::remove_file(target).await?;
      } else if matched.is_dir() {
        fs::remove_dir_all(target).await?;
      } else {
        continue;
      }

      println!("└─ {}", &target.display());
    }

    Ok(())
  }
}

impl Echo {
  pub async fn execute(&self, state: &State) -> anyhow::Result<()> {
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
  pub async fn execute<P>(&self, root: P, state: &State) -> anyhow::Result<()>
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
    let (code, output, err) = run_script::run_script!(command, options)?;
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
  pub async fn execute(&self, state: &mut State) -> anyhow::Result<()> {
    match self {
      | Self::Confirm(prompt) => prompt.execute(state).await,
      | Self::Input(prompt) => prompt.execute(state).await,
      | Self::Select(prompt) => prompt.execute(state).await,
      | Self::Editor(prompt) => prompt.execute(state).await,
    }
  }
}

impl Replace {
  pub async fn execute<P>(&self, root: P, state: &State) -> anyhow::Result<()>
  where
    P: AsRef<Path>,
  {
    let spinner = Spinner::new();
    let start = Instant::now();

    // If no glob pattern specified, traverse all files.
    let pattern = self.glob.clone().unwrap_or("**/*".to_string());

    let traverser = Traverser::new(root.as_ref())
      .ignore_dirs(true)
      .contents_first(true)
      .pattern(&pattern);

    if !self.replacements.is_empty() {
      spinner.set_message("Performing replacements");

      for matched in traverser.iter().flatten() {
        let mut buffer = String::new();
        let mut file = File::open(&matched.path).await?;

        file.read_to_string(&mut buffer).await?;

        for replacement in &self.replacements {
          if let Some(Value::String(value)) = state.get(replacement) {
            buffer = buffer.replace(&format!("{{{replacement}}}"), value);
          }
        }

        let mut result = OpenOptions::new()
          .write(true)
          .truncate(true)
          .open(&matched.path)
          .await?;

        result.write_all(buffer.as_bytes()).await?;
      }

      // Add artificial delay if replacements were performed too fast.
      let elapsed = start.elapsed();

      // This way we spent at least 1 second before stopping the spinner.
      if elapsed < Duration::from_millis(750) {
        thread::sleep(Duration::from_millis(1_000) - elapsed);
      }

      spinner.stop_with_message("Successfully performed replacements\n");
    }

    Ok(())
  }
}

impl Unknown {
  pub async fn execute(&self) -> anyhow::Result<()> {
    Ok(println!("unknown action {}", self.name))
  }
}
