use std::fmt::Display;
use std::process;

use crossterm::style::Stylize;
use inquire::formatter::StringFormatter;
use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};
use inquire::InquireError;

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
