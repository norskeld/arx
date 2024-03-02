#[derive(Debug)]
pub struct Input {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description.
  pub hint: String,
  /// Default value if input is empty.
  pub default: Option<String>,
}

#[derive(Debug)]
pub struct Select {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description.
  pub hint: String,
  /// List of options.
  pub options: Vec<String>,
  /// Default value. If none or invalid option is provided, the first one is selected.
  pub default: Option<String>,
}

#[derive(Debug)]
pub struct Confirm {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description of the prompt.
  pub hint: String,
  /// Default value.
  pub default: Option<bool>,
}

#[derive(Debug)]
pub struct Editor {
  /// Name of the variable that will store the answer.
  pub name: String,
  /// Short description.
  pub hint: String,
  /// Default value if input is empty.
  pub default: Option<String>,
}
