use unindent::Unindent;

use crate::actions::{State, Value};
use crate::manifest::actions::*;

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
  pub async fn execute(&self, _state: &State) -> anyhow::Result<()> {
    Ok(println!("run action"))
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
