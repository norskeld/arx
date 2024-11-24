use decaff::app::App;

#[tokio::main]
async fn main() {
  App::new().run().await
}
