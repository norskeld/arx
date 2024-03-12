use std::collections::HashMap;
use std::io;

use crossterm::style::Stylize;
use miette::Diagnostic;
use thiserror::Error;
use tokio::fs;

use crate::config::{ActionSingle, ActionSuite, Actions, Config, Value};

#[derive(Debug, Diagnostic, Error)]
pub enum ExecutorError {
  #[error("{message}")]
  #[diagnostic(code(arx::actions::executor::io))]
  Io {
    message: String,
    #[source]
    source: io::Error,
  },
}

#[derive(Debug)]
pub struct State {
  /// A map of replacements and associated values.
  values: HashMap<String, Value>,
}

impl State {
  /// Create a new state.
  pub fn new() -> Self {
    Self { values: HashMap::new() }
  }

  /// Get a value from the state.
  pub fn get(&self, name: &str) -> Option<&Value> {
    self.values.get(name)
  }

  /// Set a value in the state.
  pub fn set<N: Into<String> + AsRef<str>>(&mut self, name: N, replacement: Value) {
    self.values.insert(name.into(), replacement);
  }
}

impl Default for State {
  fn default() -> Self {
    Self::new()
  }
}

/// An executor.
#[derive(Debug)]
pub struct Executor {
  /// The config to use for execution.
  config: Config,
}

impl Executor {
  /// Create a new executor.
  pub fn new(config: Config) -> Self {
    Self { config }
  }

  /// Execute the actions.
  pub async fn execute(&self) -> miette::Result<()> {
    match &self.config.actions {
      | Actions::Suite(suites) => self.suite(suites).await?,
      | Actions::Flat(actions) => self.flat(actions).await?,
      | Actions::Empty => return Ok(()),
    };

    // Delete the config file if needed.
    if self.config.options.delete {
      fs::remove_file(&self.config.config)
        .await
        .map_err(|source| {
          ExecutorError::Io {
            message: "Failed to delete config file.".to_string(),
            source,
          }
        })?;
    }

    Ok(())
  }

  /// Execute a suite of actions.
  async fn suite(&self, suites: &[ActionSuite]) -> miette::Result<()> {
    let mut state = State::new();

    for ActionSuite { name, actions, .. } in suites {
      let hint = "Suite".cyan();
      let name = name.clone().green();

      println!("[{hint}: {name}]\n");

      // Man, I hate how peekable iterators work in Rust.
      let mut it = actions.iter().peekable();

      while let Some(action) = it.next() {
        self.single(action, &mut state).await?;

        // Do not print a trailing newline if the current and the next actions are prompts to
        // slightly improve visual clarity. Essentially, this way prompts are grouped.
        if !matches!(
          (action, it.peek()),
          (ActionSingle::Prompt(_), Some(ActionSingle::Prompt(_)))
        ) {
          println!();
        }
      }
    }

    Ok(())
  }

  /// Execute a flat list of actions.
  async fn flat(&self, actions: &[ActionSingle]) -> miette::Result<()> {
    let mut state = State::new();

    for action in actions {
      self.single(action, &mut state).await?;
      println!();
    }

    Ok(())
  }

  /// Execute a single action.
  async fn single(&self, action: &ActionSingle, state: &mut State) -> miette::Result<()> {
    let root = &self.config.root;

    match action {
      | ActionSingle::Copy(action) => action.execute(root).await,
      | ActionSingle::Move(action) => action.execute(root).await,
      | ActionSingle::Delete(action) => action.execute(root).await,
      | ActionSingle::Echo(action) => action.execute(state).await,
      | ActionSingle::Run(action) => action.execute(root, state).await,
      | ActionSingle::Prompt(action) => action.execute(state).await,
      | ActionSingle::Replace(action) => action.execute(root, state).await,
      | ActionSingle::Unknown(action) => action.execute().await,
    }
  }
}
