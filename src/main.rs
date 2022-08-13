use arx::app::{self, AppError};

#[tokio::main]
async fn main() -> Result<(), AppError> {
  app::run().await
}
