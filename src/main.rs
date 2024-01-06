use arx::app::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let mut app = App::new();
  app.run().await
}
