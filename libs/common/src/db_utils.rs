use std::env;

use miette::IntoDiagnostic;
use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn create_pool(connections: u32) -> PgPool {
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPoolOptions::new()
        .max_connections(connections)
        .connect(&db_url)
        .await
        .expect("Failed to create pool")
}

pub async fn table_exists(pool: &PgPool, table: &str) -> miette::Result<bool> {
    sqlx::query_scalar(&format!(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = '{table}')"
    ))
    .fetch_one(pool)
    .await
    .into_diagnostic()
}
