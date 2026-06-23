use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use chrono::{Days, Utc};
use common::api_response::ApiResponse;
use errors::errors::AppError;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tracing::info;

use crate::models::AppState;

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct DaysQuery {
    pub days: Option<i64>,
}

#[derive(Deserialize)]
pub struct LimitQuery {
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// Response types — camelCase for frontend consumption
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total_sales: Decimal,
    pub total_sales_change: f64,
    pub total_clients: i64,
    pub total_clients_change: f64,
    pub total_products: i64,
    pub total_products_change: f64,
    pub avg_order_value: Decimal,
    pub avg_order_value_change: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalesDataPoint {
    pub date: String,
    pub sales: Decimal,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopProduct {
    pub id: i32,
    pub name: String,
    pub units_sold: i64,
    pub revenue: Decimal,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Completed order statuses (as text, cast from order_status enum in queries).
const COMPLETED_STATUSES: &[&str] = &["confirmado", "enviado", "entregue"];

/// Helper SQL snippet for filtering completed orders.
/// We use `stat::text` cast because raw text bindings don't auto-cast to the
/// custom `order_status` enum type. {n} will be replaced with the parameter index.
fn completed_filter(param: u8) -> String {
    format!("p.stat::text = ANY(${param})")
}

/// Returns the sum total of all order items for completed orders in the given
/// period (which_start is inclusive, which_end is exclusive).
async fn period_sales(
    db: &PgPool,
    which_start: &chrono::NaiveDateTime,
    which_end: &chrono::NaiveDateTime,
) -> Result<Decimal, AppError> {
    let sql = format!(
        r#"
        SELECT COALESCE(SUM(ip.quantity * ip.unit_price), 0) as total
        FROM items_pedidos ip
        JOIN pedidos p ON p.id = ip.id_order
        WHERE p.created_at >= $1 AND p.created_at < $2
          AND {}
        "#,
        completed_filter(3),
    );
    let row = sqlx::query(&sql)
        .bind(which_start)
        .bind(which_end)
        .bind(COMPLETED_STATUSES)
        .fetch_one(db)
        .await
        .map_err(AppError::DbError)?;
    Ok(row.get("total"))
}

/// Returns distinct customer count in the given period.
async fn period_clients(
    db: &PgPool,
    which_start: &chrono::NaiveDateTime,
    which_end: &chrono::NaiveDateTime,
) -> Result<i64, AppError> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(DISTINCT customer_id) as count
        FROM pedidos
        WHERE created_at >= $1 AND created_at < $2
        "#,
    )
    .bind(which_start)
    .bind(which_end)
    .fetch_one(db)
    .await
    .map_err(AppError::DbError)?;
    Ok(row.get::<Option<i64>, _>("count").unwrap_or(0))
}

/// Returns average order value per order (only completed orders).
async fn period_avg_order_value(
    db: &PgPool,
    which_start: &chrono::NaiveDateTime,
    which_end: &chrono::NaiveDateTime,
) -> Result<Decimal, AppError> {
    let sql = format!(
        r#"
        SELECT COALESCE(AVG(sub.total), 0) as avg_val
        FROM (
            SELECT SUM(ip.quantity * ip.unit_price) as total
            FROM items_pedidos ip
            JOIN pedidos p ON p.id = ip.id_order
            WHERE p.created_at >= $1 AND p.created_at < $2
              AND {}
            GROUP BY p.id
        ) sub
        "#,
        completed_filter(3),
    );
    let row = sqlx::query(&sql)
        .bind(which_start)
        .bind(which_end)
        .bind(COMPLETED_STATUSES)
        .fetch_one(db)
        .await
        .map_err(AppError::DbError)?;
    Ok(row.get("avg_val"))
}

/// Returns total product count from the produtos service.
async fn fetch_product_count(http: &reqwest::Client, produtos_url: &str) -> Result<i64, AppError> {
    let url = format!("{produtos_url}/api/products/count");
    #[derive(serde::Deserialize)]
    struct CountResponse {
        data: CountData,
    }
    #[derive(serde::Deserialize)]
    struct CountData {
        count: i64,
    }
    let resp = http
        .get(&url)
        .send()
        .await
        .map_err(|_| AppError::Internal("produtos service unreachable".to_string()))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal("failed to fetch product count".to_string()));
    }

    let body: CountResponse = resp
        .json()
        .await
        .map_err(|_| AppError::Internal("failed to parse product count response".to_string()))?;

    Ok(body.data.count)
}

fn percentage_change(current: Decimal, previous: Decimal) -> f64 {
    if previous.is_zero() {
        return 0.0;
    }
    let change = (current - previous) / previous * Decimal::from(100);
    change.to_f64().unwrap_or(0.0)
}

fn percentage_change_i64(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        return 0.0;
    }
    ((current - previous) as f64) / (previous as f64) * 100.0
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn dashboard_stats_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    info!("Fetching admin dashboard stats");

    let now = Utc::now().naive_utc();
    let current_start = now - Days::new(30);
    let previous_end = current_start;
    let previous_start = current_start - Days::new(30);

    let (current_sales, previous_sales) = tokio::join!(
        period_sales(&state.db, &current_start, &now),
        period_sales(&state.db, &previous_start, &previous_end),
    );
    let (current_clients, previous_clients) = tokio::join!(
        period_clients(&state.db, &current_start, &now),
        period_clients(&state.db, &previous_start, &previous_end),
    );
    let (current_avg, previous_avg) = tokio::join!(
        period_avg_order_value(&state.db, &current_start, &now),
        period_avg_order_value(&state.db, &previous_start, &previous_end),
    );

    let current_sales = current_sales?;
    let previous_sales = previous_sales?;
    let current_clients = current_clients?;
    let previous_clients = previous_clients?;
    let current_avg = current_avg?;
    let previous_avg = previous_avg?;

    let total_products = fetch_product_count(&state.http, &state.produtos_url).await?;

    let stats = DashboardStats {
        total_sales: current_sales,
        total_sales_change: percentage_change(current_sales, previous_sales),
        total_clients: current_clients,
        total_clients_change: percentage_change_i64(current_clients, previous_clients),
        total_products,
        total_products_change: 0.0,
        avg_order_value: current_avg,
        avg_order_value_change: percentage_change(current_avg, previous_avg),
    };

    Ok(Json(ApiResponse::ok(serde_json::json!({ "stats": stats }))))
}

pub async fn sales_data_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DaysQuery>,
) -> Result<impl IntoResponse, AppError> {
    let days = query.days.unwrap_or(30).min(365).max(1);
    info!(days, "Fetching sales chart data");

    let cutoff = Utc::now().naive_utc() - Days::new(days as u64);

    let sql = format!(
        r#"
        SELECT
            TO_CHAR(p.created_at, 'YYYY-MM-DD') as date_str,
            COALESCE(SUM(ip.quantity * ip.unit_price), 0) as total
        FROM pedidos p
        JOIN items_pedidos ip ON ip.id_order = p.id
        WHERE p.created_at >= $1
          AND {}
        GROUP BY TO_CHAR(p.created_at, 'YYYY-MM-DD')
        ORDER BY date_str
        "#,
        completed_filter(2),
    );
    let rows = sqlx::query(&sql)
        .bind(cutoff)
        .bind(COMPLETED_STATUSES)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::DbError)?;

    let data: Vec<SalesDataPoint> = rows
        .iter()
        .map(|row| SalesDataPoint {
            date: row.get("date_str"),
            sales: row.get("total"),
        })
        .collect();

    Ok(Json(ApiResponse::ok(serde_json::json!({ "sales": data }))))
}

pub async fn top_products_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LimitQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(5).min(50).max(1);
    info!(limit, "Fetching top products");

    let sql = format!(
        r#"
        SELECT
            ip.id_product,
            COALESCE(SUM(ip.quantity), 0)::BIGINT as units_sold,
            COALESCE(SUM(ip.quantity * ip.unit_price), 0) as revenue
        FROM items_pedidos ip
        JOIN pedidos p ON p.id = ip.id_order
        WHERE {}
        GROUP BY ip.id_product
        ORDER BY revenue DESC
        LIMIT $2
        "#,
        completed_filter(1),
    );
    let rows = sqlx::query(&sql)
        .bind(COMPLETED_STATUSES)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::DbError)?;

    // Fetch product names from produtos service
    let http = &state.http;
    let produtos_url = &state.produtos_url;

    let mut products = Vec::with_capacity(rows.len());
    for row in &rows {
        let id_product: i32 = row.get("id_product");
        let name = fetch_product_name(http, produtos_url, id_product).await;
        products.push(TopProduct {
            id: id_product,
            name,
            units_sold: row.get("units_sold"),
            revenue: row.get("revenue"),
        });
    }

    Ok(Json(ApiResponse::ok(serde_json::json!({ "products": products }))))
}

/// Fetch a single product's name from the produtos service.
async fn fetch_product_name(http: &reqwest::Client, produtos_url: &str, id: i32) -> String {
    let url = format!("{produtos_url}/api/products/{id}");

    #[derive(serde::Deserialize)]
    struct ProductResponse {
        data: ProductData,
    }
    #[derive(serde::Deserialize)]
    struct ProductData {
        product: ProductDto,
    }
    #[derive(serde::Deserialize)]
    struct ProductDto {
        #[serde(rename = "Descricao")]
        nome: String,
    }

    match http.get(&url).send().await {
        Ok(r) if r.status().is_success() => match r.json::<ProductResponse>().await {
            Ok(body) => body.data.product.nome,
            Err(_) => format!("Product #{id}"),
        },
        _ => format!("Product #{id}"),
    }
}
