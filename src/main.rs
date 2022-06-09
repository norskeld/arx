use std::env;
use std::fs;

use kdl::KdlDocument;
use imp::config::{self, Action};

// use imp::app::{self, AppError};

// #[tokio::main]
// async fn main() -> Result<(), AppError> {
//   app::run().await
// }

fn main() -> std::io::Result<()> {
  let filename = env::current_dir()?.join("imp.kdl");

  let contents = fs::read_to_string(filename)?;
  let doc: KdlDocument = contents.parse().expect("Failed to parse config file.");

  let actions = config::get_actions(&doc);

  actions.map(|action| {
    if let Action::Suite(suites) = action {
      let (resolved, unresolved) = config::resolve_requirements(&suites);

      println!("Resolved: {resolved:#?}");
      println!("Unresolved: {unresolved:#?}");
    }
  });

  Ok(())
}
