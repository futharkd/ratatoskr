use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ratatoskr::init_tracing();
    let config_path = env::args()
        .nth(1)
        .or_else(|| env::var("RATATOSKR_CONFIG").ok())
        .unwrap_or_else(|| "examples/ratatoskr.example.toml".to_string());
    ratatoskr::run(&config_path).await
}
