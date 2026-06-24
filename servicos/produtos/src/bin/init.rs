use std::fs::File;

use common::db_utils::{create_pool, table_exists};

use miette::IntoDiagnostic;
use produtos::{models::Product, normalization::normalize_string};
use tracing::{Level, info, span};

#[tokio::main]
pub async fn main() -> miette::Result<()> {
    tracing_subscriber::FmtSubscriber::new();
    info!("Initializing pool");
    let pool = create_pool(1).await;
    info!("Checking if migrations are needed...");
    if !table_exists(&pool, "produtos").await? {
        sqlx::migrate!().run(&pool).await.into_diagnostic()?;

        let data = "raw/data.csv";
        let data = File::open(data).into_diagnostic()?;
        let mut reader = csv::Reader::from_reader(data);
        for result in reader.deserialize() {
            let record: Product = result.into_diagnostic()?;
            let span = span!(Level::INFO, "Populating", id = record.id);
            let (nome_norm, marca_norm) = (
                normalize_string(&record.nome),
                normalize_string(&record.marca),
            );
            let _guard = span.enter();
            sqlx::query(
            r#"INSERT INTO produtos (id, nome, nome_norm, marca, marca_norm, num_fab, unidade, valor, descricao, estoque)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        )
        .bind(record.id)
        .bind(record.nome)
        .bind(nome_norm)
        .bind(record.marca)
        .bind(marca_norm)
        .bind(record.num_fab)
        .bind(record.unidade)
        .bind(record.valor)
        .bind(record.descricao)
        .bind(record.estoque)
        .execute(&pool)
        .await
        .into_diagnostic()?;
            info!(id = record.id, "Insertion complete");
        }
        let max_id: Option<i32> = sqlx::query_scalar("SELECT MAX(id) FROM produtos")
            .fetch_one(&pool)
            .await
            .into_diagnostic()?;
        info!(max_id, "Setting max id...");
        if let Some(max) = max_id {
            info!("{}", format!("Max id: {max}"));
            sqlx::query("SELECT setval('produtos_id_seq', $1)")
                .bind(max)
                .execute(&pool)
                .await
                .into_diagnostic()?;
        }
        let app_user = dotenvy::var("APP_USER").expect("APP USER MUST BE SET");
        let migrator = dotenvy::var("MIGRATION_USER").unwrap_or_else(|_| "migrator".to_string());
        // After running migrations, grant DML to app_user on all tables
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
        // Future tables created by migrator auto-grant DML to app_user
        sqlx::raw_sql(&format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE {migrator} IN SCHEMA public \
             GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO {app_user}"
        ))
        .execute(&pool)
        .await
        .into_diagnostic()?;
        sqlx::raw_sql(&format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE {migrator} IN SCHEMA public \
             GRANT USAGE, SELECT ON SEQUENCES TO {app_user}"
        ))
        .execute(&pool)
        .await
        .into_diagnostic()?;
        sqlx::raw_sql( & format!(
            "UPDATE produtos
            SET estoque=10;"
        ))
        info!("Initialization complete");
        return Ok(());
    }
    Ok(())
}
