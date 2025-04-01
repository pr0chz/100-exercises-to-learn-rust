use outro_08::server::{listen, run_server};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = listen(Some(8080)).await?;
    run_server(listener).await
}
