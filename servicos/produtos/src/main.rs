use common::db_utils::create_pool;
use dotenvy::dotenv;
use miette::IntoDiagnostic;
use produtos::{models::AppState, router::create_router};
use std::sync::Arc;

#[tokio::main]
async fn main() -> miette::Result<()> {
    dotenv().ok();
    let pool = create_pool(10).await;
    let app = create_router(Arc::new(AppState { db: pool.clone() }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.into_diagnostic()
}
