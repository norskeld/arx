use crossterm::style::Stylize;
use miette::Severity;

/// Prints an error message and exits the program if given an error.
pub fn try_report<T>(fallible: miette::Result<T>) {
  if let Err(err) = fallible {
    let severity = match err.severity().unwrap_or(Severity::Error) {
      | Severity::Advice => "Advice:".cyan(),
      | Severity::Warning => "Warning:".yellow(),
      | Severity::Error => "Error:".red(),
    };

    if err.code().is_some() {
      eprintln!("{severity} {err:?}");
    } else {
      eprintln!("{severity}\n");
      eprintln!("{err:?}");
    }
  }
}
