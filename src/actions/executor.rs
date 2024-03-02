use std::collections::HashMap;

use console::style;

use crate::manifest::{ActionSingle, ActionSuite, Actions, Manifest};

/// Alias for a map of string replacements.
pub type Replacements = HashMap<String, String>;

pub struct Executor {
  manifest: Manifest,
}

impl Executor {
  pub fn new(manifest: Manifest) -> Self {
    Self { manifest }
  }

  pub async fn execute(&self) -> anyhow::Result<()> {
    let executor = match &self.manifest.actions {
      | Actions::Suite(suites) => self.execute_suite(suites).await,
      | Actions::Flat(actions) => self.execute_flat(actions).await,
      | Actions::Empty => return Ok(println!("No actions found.")),
    };

    executor
  }

  async fn execute_suite(&self, suites: &[ActionSuite]) -> anyhow::Result<()> {
    let mut replacements = HashMap::<String, String>::new();

    for ActionSuite { name, actions, .. } in suites.iter() {
      println!(
        "{symbol} {title}: {name}\n",
        symbol = style("◆").blue().bold(),
        title = style("Running suite").blue(),
        name = style(name).green()
      );

      for action in actions.iter() {
        self.execute_single(action, &mut replacements).await?;
        println!();
      }
    }

    Ok(())
  }

  async fn execute_flat(&self, actions: &[ActionSingle]) -> anyhow::Result<()> {
    let mut injects = HashMap::<String, String>::new();

    for action in actions.iter() {
      self.execute_single(action, &mut injects).await?;
      println!();
    }

    Ok(())
  }

  async fn execute_single(
    &self,
    action: &ActionSingle,
    replacements: &mut Replacements,
  ) -> anyhow::Result<()> {
    let executor = match action {
      | ActionSingle::Copy(action) => action.execute().await,
      | ActionSingle::Move(action) => action.execute().await,
      | ActionSingle::Delete(action) => action.execute().await,
      | ActionSingle::Echo(action) => action.execute(&replacements).await,
      | ActionSingle::Run(action) => action.execute(&replacements).await,
      | ActionSingle::Prompt(action) => action.execute(replacements).await,
      | ActionSingle::Replace(action) => action.execute(&replacements).await,
      | ActionSingle::Unknown(action) => action.execute().await,
    };

    executor
  }
}
