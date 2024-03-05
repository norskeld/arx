use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

/// Small wrapper around the `indicatif` spinner.
pub struct Spinner {
  spinner: ProgressBar,
}

impl Spinner {
  /// Creates a new spinner.
  pub fn new() -> Self {
    let style = ProgressStyle::default_spinner().tick_chars("⠋⠙⠚⠒⠂⠂⠒⠲⠴⠦⠖⠒⠐⠐⠒⠓⠋·");
    let spinner = ProgressBar::new_spinner();

    spinner.set_style(style);
    spinner.enable_steady_tick(Duration::from_millis(80));

    Self { spinner }
  }

  /// Sets the message of the spinner.
  pub fn set_message<S>(&self, message: S)
  where
    S: Into<String> + AsRef<str>,
  {
    self.spinner.set_message(message.into());
  }

  /// Stops the spinner.
  #[allow(dead_code)]
  pub fn stop(&self) {
    self.spinner.finish();
  }

  /// Stops the spinner with the message.
  pub fn stop_with_message<S>(&self, message: S)
  where
    S: Into<String> + AsRef<str>,
  {
    self.spinner.finish_with_message(message.into());
  }

  /// Stops the spinner and clears the message.
  #[allow(dead_code)]
  pub fn stop_with_clear(&self) {
    self.spinner.finish_and_clear();
  }
}

impl Default for Spinner {
  fn default() -> Self {
    Self::new()
  }
}
