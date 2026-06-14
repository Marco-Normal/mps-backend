use common::db_utils::create_pool;
use dotenvy::dotenv;
use miette::{IntoDiagnostic, WrapErr};
use pedidos::{models::AppState, router::create_router};
use std::sync::Arc;
use tracing_appender::rolling::Rotation;
use tracing_subscriber::{filter, fmt, prelude::*};

#[tokio::main]
async fn main() -> miette::Result<()> {
    let log_dir = std::env::var("PEDIDOS_LOG_DIR").unwrap_or_else(|_| "./logs".to_string());
    std::fs::create_dir_all(&log_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create log dir: {log_dir}"))?;

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(10)
        .filename_prefix("pedidos_api.log")
        .build(&log_dir)
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

    let evolution_url = std::env::var("EVOLUTION_API_URL")
        .into_diagnostic()
        .wrap_err("EVOLUTION_API_URL must be set")?;

    let evolution_key = std::env::var("EVOLUTION_API_KEY")
        .into_diagnostic()
        .wrap_err("EVOLUTION_API_KEY must be set")?;

    let evolution_instance = std::env::var("EVOLUTION_INSTANCE_NAME")
        .into_diagnostic()
        .wrap_err("EVOLUTION_INSTANCE_NAME must be set")?;

    let seller_whatsapp = std::env::var("SELLER_WHATSAPP")
        .into_diagnostic()
        .wrap_err("SELLER_WHATSAPP must be set")?;

    let frontend_url = std::env::var("FRONTEND_URL")
        .into_diagnostic()
        .wrap_err("FRONTEND_URL must be set")?
        .parse::<axum::http::HeaderValue>()
        .into_diagnostic()
        .wrap_err("FRONTEND_URL is not a valid HTTP origin header value")?;

    let state = Arc::new(AppState {
        db: pool,
        http,
        produtos_url,
        jwt_secret,
        evolution_url,
        evolution_key,
        evolution_instance,
        seller_whatsapp,
        frontend_url,
    });

    let app = create_router(state);

    let port = std::env::var("PEDIDOS_PORT").unwrap_or_else(|_| "3001".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to bind to port {port}"))?;

    tracing::info!(port, "pedidos service listening");
    axum::serve(listener, app).await.into_diagnostic()
}
