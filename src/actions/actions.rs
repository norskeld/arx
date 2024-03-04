use std::path::{Path, PathBuf};
use std::process;

use crossterm::style::Stylize;
use run_script::ScriptOptions;
use unindent::Unindent;

use crate::actions::{State, Value};
use crate::manifest::actions::*;
use crate::spinner::Spinner;

impl Copy {
  pub async fn execute(&self) -> anyhow::Result<()> {
    Ok(println!("cp action"))
  }
}

impl Move {
  pub async fn execute(&self) -> anyhow::Result<()> {
    Ok(println!("mv action"))
  }
}

impl Delete {
  pub async fn execute(&self) -> anyhow::Result<()> {
    Ok(println!("rm action"))
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
        if let Some(Value::String(value)) = state.values.get(inject) {
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
        if let Some(Value::String(value)) = state.values.get(inject) {
          // In format strings we escape `{` and `}` by doubling them.
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
  pub async fn execute(&self, _state: &State) -> anyhow::Result<()> {
    Ok(println!("replace action"))
  }
}

impl Unknown {
  pub async fn execute(&self) -> anyhow::Result<()> {
    Ok(println!("unknown action {}", self.name))
  }
}
