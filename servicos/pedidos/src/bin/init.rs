use common::db_utils::{create_pool, table_exists};
use miette::IntoDiagnostic;
use tracing::info;

#[tokio::main]
pub async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Initializing pedidos database pool");
    let pool = create_pool(1).await;
    info!("Checking if migrations are needed...");
    if !table_exists(&pool, "pedidos").await? {
        sqlx::migrate!().run(&pool).await.into_diagnostic()?;
        let app_user = dotenvy::var("APP_USER").expect("APP_USER must be set");
        sqlx::raw_sql(&format!(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {app_user}"
        ))
        .execute(&pool)
        .await
        .into_diagnostic()?;
        sqlx::raw_sql(&format!(
            "GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO {app_user}"
        ))
        .execute(&pool)
        .await
        .into_diagnostic()?;
        info!("Initialization complete");
        return Ok(());
    }
    info!("Migrations already applied, nothing to do");
    Ok(())
}
