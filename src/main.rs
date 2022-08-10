use imp::app::{self, AppError};

#[tokio::main]
async fn main() -> Result<(), AppError> {
  app::run().await
}

// use std::env;
// use std::fs;

// use kdl::KdlDocument;
// use imp::config::{self, Action};

// fn main() -> std::io::Result<()> {
//   let filename = env::current_dir()?.join("imp.kdl");

//   let contents = fs::read_to_string(filename)?;
//   let doc: KdlDocument = contents.parse().expect("Failed to parse config file.");

//   let replacements = config::get_replacements(&doc);
//   let actions = config::get_actions(&doc);

//   replacements.map(|items| {
//     items.iter().for_each(|item| {
//       let tag = &item.tag;
//       let description = &item.description;

//       println!("{tag} = {description}");
//     })
//   });

//   actions.map(|action| {
//     if let Action::Suite(suites) = action {
//       let (resolved, unresolved) = config::resolve_requirements(&suites);

//       println!("Resolved: {resolved:#?}");
//       println!("Unresolved: {unresolved:#?}");
//     }
//   });

//   Ok(())
// }
