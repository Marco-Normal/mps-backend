use mps_backend::models::Product;
use sqlx::postgres::PgPoolOptions;
use std::{error::Error, fs::File};
use tracing::{Level, event};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv()?;
    let url = dotenvy::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    event!(Level::INFO, "Connected to DB with sucess");
    event!(Level::INFO, "Running Migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    event!(Level::INFO, "Migrations ran with sucess");

    let data = "raw/data.csv";
    event!(Level::INFO, "{}", format!("Reading {data}"));
    let data = File::open(data)?;

    let mut reader = csv::Reader::from_reader(data);
    for result in reader.deserialize() {
        let record: Product = result?;
        sqlx::query(
            "INSERT INTO produtos (id, nome, marca, num_fab, unidade, valor) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(record.id)
            .bind(record.nome)
            .bind(record.marca)
            .bind(record.num_fab)
            .bind(record.unidade)
            .bind(record.valor)
            .execute(&pool).await?;
    }
    Ok(())
}
