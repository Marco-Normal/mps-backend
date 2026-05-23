use std::sync::Arc;

use dotenvy::dotenv;
use mps_backend::{models::AppState, router::create_router};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let db_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = match PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await
    {
        Ok(pool) => pool,
        Err(err) => {
            println!("Failed to connect to DB. {}", err);
            std::process::exit(1)
        }
    };

    let app = create_router(Arc::new(AppState { db: pool.clone() }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
