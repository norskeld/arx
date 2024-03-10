use crate::config::value::Number;

#[derive(Debug)]
pub struct InputPrompt {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description.
  pub hint: String,
  /// Default value if input is empty.
  pub default: Option<String>,
}

#[derive(Debug)]
pub struct NumberPrompt {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description.
  pub hint: String,
  /// Default value if input is empty.
  pub default: Option<Number>,
}

#[derive(Debug)]
pub struct SelectPrompt {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description.
  pub hint: String,
  /// List of options.
  pub options: Vec<String>,
}

#[derive(Debug)]
pub struct ConfirmPrompt {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description of the prompt.
  pub hint: String,
  /// Default value.
  pub default: Option<bool>,
}

#[derive(Debug)]
pub struct EditorPrompt {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description.
  pub hint: String,
  /// Default value if input is empty.
  pub default: Option<String>,
}
