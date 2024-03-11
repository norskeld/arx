use std::fmt::{self, Display};
use std::str::FromStr;

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Diagnostic, Error)]
#[error("`{0}` is not a valid number.")]
#[diagnostic(code(arx::config::prompts::parse))]
pub struct NumberParseError(pub String);

/// Value of a number prompt.
#[derive(Clone, Debug)]
pub enum Number {
  /// Integer value.
  Integer(i64),
  /// Floating point value.
  Float(f64),
}

impl Display for Number {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Integer(int) => write!(f, "{int}"),
      | Self::Float(float) => write!(f, "{float}"),
    }
  }
}

impl FromStr for Number {
  type Err = NumberParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    s.parse::<i64>()
      .map(Self::Integer)
      .or_else(|_| s.parse::<f64>().map(Self::Float))
      .map_err(|_| NumberParseError(s.to_string()))
  }
}

/// Replacement value.
#[derive(Debug)]
pub enum Value {
  /// A string value.
  String(String),
  // A number value.
  Number(Number),
  /// A boolean value.
  Bool(bool),
}

impl Display for Value {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::String(string) => write!(f, "{string}"),
      | Self::Number(number) => write!(f, "{number}"),
      | Self::Bool(boolean) => write!(f, "{boolean}"),
    }
  }
}
