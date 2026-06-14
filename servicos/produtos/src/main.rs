use common::db_utils::create_pool;
use dotenvy::dotenv;
use miette::IntoDiagnostic;
use produtos::{models::AppState, router::create_router};
use std::sync::Arc;
use tracing_appender::rolling::Rotation;

use tracing_subscriber::{filter, fmt, prelude::*};
#[tokio::main]
async fn main() -> miette::Result<()> {
    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(10)
        .filename_prefix("products_api.log")
        .build("/var/log")
        .into_diagnostic()?;
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(fmt::layer().with_writer(non_blocking).json())
        .with(filter::EnvFilter::try_from_env("PRODUCTS_LOG").unwrap_or_else(|_| "info".into()))
        .init();
    dotenv().ok();
    let pool = create_pool(10).await;
    let app = create_router(Arc::new(AppState { db: pool.clone() }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.into_diagnostic()
}
