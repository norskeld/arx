use std::collections::HashMap;

use crossterm::style::Stylize;

use crate::manifest::{ActionSingle, ActionSuite, Actions, Manifest};

/// Replacement value.
#[derive(Debug)]
pub enum Value {
  /// A string value.
  String(String),
  /// A boolean value.
  Bool(bool),
}

#[derive(Debug)]
pub struct State {
  /// A map of replacements and associated values.
  pub values: HashMap<String, Value>,
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
  /// The manifest to use for execution.
  manifest: Manifest,
}

impl Executor {
  /// Create a new executor.
  pub fn new(manifest: Manifest) -> Self {
    Self { manifest }
  }

  /// Execute the actions.
  pub async fn execute(&self) -> anyhow::Result<()> {
    let _ = match &self.manifest.actions {
      | Actions::Suite(suites) => self.suite(suites).await,
      | Actions::Flat(actions) => self.flat(actions).await,
      | Actions::Empty => return Ok(println!("No actions found.")),
    };

    Ok(())
  }

  /// Execute a suite of actions.
  async fn suite(&self, suites: &[ActionSuite]) -> anyhow::Result<State> {
    let mut state = State::new();

    for ActionSuite { name, actions, .. } in suites {
      let symbol = "+".blue().bold();
      let title = "Suite".blue();
      let name = name.clone().green();

      println!("{symbol} {title}: {name}\n");

      for action in actions {
        self.single(action, &mut state).await?;
        println!();
      }
    }

    Ok(state)
  }

  /// Execute a flat list of actions.
  async fn flat(&self, actions: &[ActionSingle]) -> anyhow::Result<State> {
    let mut state = State::new();

    for action in actions {
      self.single(action, &mut state).await?;
      println!();
    }

    Ok(state)
  }

  /// Execute a single action.
  async fn single(&self, action: &ActionSingle, state: &mut State) -> anyhow::Result<()> {
    let root = &self.manifest.root;

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
