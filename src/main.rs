use arx::app::App;
use crossterm::style::Stylize;
use miette::Severity;

#[tokio::main]
async fn main() {
  let app = App::new();

  if let Err(err) = app.run().await {
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

    std::process::exit(1);
  }
}
