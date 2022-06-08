use std::env;
use std::fs;

use kdl::KdlDocument;
use imp::config;

// use imp::app::{self, AppError};

// #[tokio::main]
// async fn main() -> Result<(), AppError> {
//   app::run().await
// }

fn main() -> std::io::Result<()> {
  let mut filename = env::current_dir()?;
  filename.push("imp.kdl");

  let contents = fs::read_to_string(filename)?;
  let doc: KdlDocument = contents.parse().expect("Failed to parse config file.");

  println!("{:#?}", config::get_replacements(&doc));
  println!("{:#?}", config::get_actions(&doc));

  Ok(())
}
