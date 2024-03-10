use std::collections::HashSet;

use crate::config::prompts::*;

/// Copies a file or directory. Glob-friendly. Overwrites by default.
#[derive(Debug)]
pub struct Copy {
  /// Source(s) to copy.
  pub from: String,
  /// Where to copy to.
  pub to: String,
  /// Whether to overwrite or not. Defaults to `true`.
  pub overwrite: bool,
}

/// Moves a file or directory. Glob-friendly. Overwrites by default.
#[derive(Debug)]
pub struct Move {
  /// Source(s) to move.
  pub from: String,
  /// Where to move to.
  pub to: String,
  /// Whether to overwrite or not. Defaults to `true`.
  pub overwrite: bool,
}

/// Deletes a file or directory. Glob-friendly.
#[derive(Debug)]
pub struct Delete {
  /// Target to delete.
  pub target: String,
}

/// Echoes a message to stdout.
#[derive(Debug)]
pub struct Echo {
  /// Message to output.
  pub message: String,
  /// An optional list of placeholders to be injected into the command.
  ///
  /// ```kdl
  /// echo "Hello {R_PM}" {
  ///   inject "R_PM"
  /// }
  /// ```
  ///
  /// All placeholders are processed _before_ running a command.
  pub injects: Option<HashSet<String>>,
  /// Whether to trim multiline message or not. Defaults to `true`.
  pub trim: bool,
}

/// Runs an arbitrary command in the shell.
#[derive(Debug)]
pub struct Run {
  /// Command name. Optional, defaults either to the command itself or to the first line of
  /// the multiline command.
  pub name: Option<String>,
  /// Command to run in the shell.
  pub command: String,
  /// An optional list of placeholders to be injected into the command. Consider the following
  /// example:
  ///
  /// We use `inject` to disambiguate whether `{R_PM}` is part of a command or is a placeholder
  /// that should be replaced with something.
  ///
  /// ```kdl
  /// run "{R_PM} install {R_PM_ARGS}" {
  ///   inject "R_PM" "R_PM_ARGS"
  /// }
  /// ```
  ///
  /// All placeholders are processed _before_ running a command.
  pub injects: Option<HashSet<String>>,
}

/// Prompt actions.
#[derive(Debug)]
pub enum Prompt {
  Input(InputPrompt),
  Number(NumberPrompt),
  Select(SelectPrompt),
  Confirm(ConfirmPrompt),
  Editor(EditorPrompt),
}

/// Execute given replacements using values provided by prompts. Optionally, only apply
/// replacements to files matching the provided glob.
#[derive(Debug)]
pub struct Replace {
  /// Replacements to apply.
  pub replacements: HashSet<String>,
  /// Optional glob to limit files to apply replacements to.
  pub glob: Option<String>,
}

/// Fallback action for pattern matching ergonomics and reporting purposes.
#[derive(Debug)]
pub struct Unknown {
  pub name: String,
}
