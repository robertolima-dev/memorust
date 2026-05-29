use memors::server::run_server;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    run_server("127.0.0.1:6379").await
}
