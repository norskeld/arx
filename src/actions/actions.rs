use unindent::Unindent;

use crate::actions::Replacements;
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
  pub async fn execute(&self, replacements: &Replacements) -> anyhow::Result<()> {
    let message = if self.trim {
      self.message.trim()
    } else {
      &self.message
    };

    let mut message = message.unindent();

    if let Some(injects) = &self.injects {
      for inject in injects {
        if let Some(value) = replacements.get(inject) {
          message = message.replace(&format!("{{{inject}}}"), value);
        }
      }
    }

    Ok(println!("{message}"))
  }
}

impl Run {
  pub async fn execute(&self, _replacements: &Replacements) -> anyhow::Result<()> {
    Ok(println!("run action"))
  }
}

impl Prompt {
  // TODO: This will require mutable reference to `Executor` or `prompts`.
  pub async fn execute(&self, _replacements: &mut Replacements) -> anyhow::Result<()> {
    Ok(println!("prompt action"))
  }
}

impl Replace {
  pub async fn execute(&self, _replacements: &Replacements) -> anyhow::Result<()> {
    Ok(println!("replace action"))
  }
}

impl Unknown {
  pub async fn execute(&self) -> anyhow::Result<()> {
    Ok(println!("unknown action {}", self.name))
  }
}
