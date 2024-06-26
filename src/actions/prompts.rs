use std::fmt::Display;
use std::process;

use crossterm::style::Stylize;
use inquire::formatter::StringFormatter;
use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};
use inquire::{Confirm, CustomType, Editor, InquireError, Select, Text};

use crate::actions::State;
use crate::config::prompts::*;
use crate::config::{Number, Value};

/// Helper module holding useful functions.
mod helpers {
  use super::*;

  /// Returns configured theme.
  pub fn theme<'r>() -> RenderConfig<'r> {
    let default = RenderConfig::default();
    let stylesheet = StyleSheet::default();

    let prompt_prefix = Styled::new("?").with_fg(Color::LightYellow);
    let answered_prefix = Styled::new("✓").with_fg(Color::LightGreen);

    default
      .with_prompt_prefix(prompt_prefix)
      .with_answered_prompt_prefix(answered_prefix)
      .with_default_value(stylesheet.with_fg(Color::DarkGrey))
  }

  /// Returns a formatter that shows `<empty>` if the input is empty.
  pub fn empty_formatter<'s>() -> StringFormatter<'s> {
    &|input| {
      if input.is_empty() {
        "<empty>".dark_grey().to_string()
      } else {
        input.to_string()
      }
    }
  }

  /// Helper method that generates `(name, hint, help)`.
  pub fn messages<S>(name: S, hint: S) -> (String, String, String)
  where
    S: Into<String> + AsRef<str> + Display,
  {
    let name = name.into();
    let hint = format!("{}:", &hint);
    let help = format!("The answer will be mapped to: {}", &name);

    (name, hint, help)
  }

  /// Handle interruption/cancelation events.
  pub fn interrupt(err: InquireError) {
    match err {
      | InquireError::OperationCanceled => {
        process::exit(0);
      },
      | InquireError::OperationInterrupted => {
        println!("{}", "<interrupted>".red());
        process::exit(0);
      },
      | _ => {},
    }
  }
}

impl ConfirmPrompt {
  /// Execute the prompt and populate the state.
  pub async fn execute(&self, state: &mut State) -> miette::Result<()> {
    let (name, hint, help) = helpers::messages(&self.name, &self.hint);

    let mut prompt = Confirm::new(&hint)
      .with_help_message(&help)
      .with_render_config(helpers::theme());

    if let Some(default) = self.default {
      prompt = prompt.with_default(default);
    }

    match prompt.prompt() {
      | Ok(value) => state.set(name, Value::Bool(value)),
      | Err(err) => helpers::interrupt(err),
    }

    Ok(())
  }
}

impl InputPrompt {
  /// Execute the prompt and populate the state.
  pub async fn execute(&self, state: &mut State) -> miette::Result<()> {
    let (name, hint, help) = helpers::messages(&self.name, &self.hint);

    let mut prompt = Text::new(&hint)
      .with_help_message(&help)
      .with_formatter(helpers::empty_formatter())
      .with_render_config(helpers::theme());

    if let Some(default) = &self.default {
      prompt = prompt.with_default(default);
    } else {
      prompt = prompt.with_validator(inquire::required!("This field is required."));
    }

    match prompt.prompt() {
      | Ok(value) => state.set(name, Value::String(value)),
      | Err(err) => helpers::interrupt(err),
    }

    Ok(())
  }
}

impl NumberPrompt {
  /// Execute the prompt and populate the state.
  pub async fn execute(&self, state: &mut State) -> miette::Result<()> {
    let (name, hint, help) = helpers::messages(&self.name, &self.hint);

    let mut prompt = CustomType::<Number>::new(&hint)
      .with_help_message(&help)
      .with_formatter(&|input| input.to_string())
      .with_render_config(helpers::theme());

    if let Some(default) = &self.default {
      prompt = prompt.with_default(default.to_owned());
    } else {
      // NOTE: This is a bit confusing, but essentially this message will be showed when no input
      // was provided by the user.
      prompt = prompt.with_error_message("This field is required.");
    }

    match prompt.prompt() {
      | Ok(value) => state.set(name, Value::Number(value)),
      | Err(err) => helpers::interrupt(err),
    }

    Ok(())
  }
}

impl SelectPrompt {
  /// Execute the prompt and populate the state.
  pub async fn execute(&self, state: &mut State) -> miette::Result<()> {
    let (name, hint, help) = helpers::messages(&self.name, &self.hint);

    let options = self.options.iter().map(String::to_string).collect();

    let prompt = Select::new(&hint, options)
      .with_help_message(&help)
      .with_render_config(helpers::theme());

    match prompt.prompt() {
      | Ok(value) => state.set(name, Value::String(value)),
      | Err(err) => helpers::interrupt(err),
    }

    Ok(())
  }
}

impl EditorPrompt {
  /// Execute the prompt and populate the state.
  pub async fn execute(&self, state: &mut State) -> miette::Result<()> {
    let (name, hint, help) = helpers::messages(&self.name, &self.hint);

    let mut prompt = Editor::new(&hint)
      .with_help_message(&help)
      .with_render_config(helpers::theme());

    if let Some(default) = &self.default {
      prompt = prompt.with_predefined_text(default);
    }

    match prompt.prompt() {
      | Ok(value) => state.set(name, Value::String(value)),
      | Err(err) => helpers::interrupt(err),
    }

    Ok(())
  }
}
