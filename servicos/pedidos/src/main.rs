use common::db_utils::create_pool;
use dotenvy::dotenv;
use miette::{IntoDiagnostic, WrapErr};
use pedidos::{models::AppState, router::create_router};
use std::sync::Arc;
use tracing_appender::rolling::Rotation;
use tracing_subscriber::{filter, fmt, prelude::*};

#[tokio::main]
async fn main() -> miette::Result<()> {
    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(10)
        .filename_prefix("pedidos_api.log")
        .build("/var/log")
        .into_diagnostic()
        .wrap_err("Failed to initialize log file appender")?;

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(fmt::layer().with_writer(non_blocking).json())
        .with(filter::EnvFilter::try_from_env("PEDIDOS_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    dotenv().ok();

    let pool = create_pool(10).await;
    let http = reqwest::Client::new();

    let produtos_url = std::env::var("PRODUTOS_SERVICE_URL")
        .into_diagnostic()
        .wrap_err("PRODUTOS_SERVICE_URL must be set")?;

    let jwt_secret = std::env::var("JWT_SECRET")
        .into_diagnostic()
        .wrap_err("JWT_SECRET must be set")?;

    let state = Arc::new(AppState {
        db: pool,
        http,
        produtos_url,
        jwt_secret,
    });

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .into_diagnostic()
        .wrap_err("Failed to bind to port 3001")?;

    tracing::info!("pedidos service listening on port 3001");
    axum::serve(listener, app).await.into_diagnostic()
}
