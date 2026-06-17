use std::sync::Arc;

use errors::errors::AppError;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    models::{AppState, CompleteOrder, Order, OrderItem, Status},
    produto_client::{ValidatedItem, adjust_product_stock, update_product_stock, validate_items},
    schema::{AddItemSchema, CreateOrderSchema, OrderListQuery, UpdateOrderItemsSchema, UpdateStatusSchema},
};

/// Returns true if the transition from `current` to `next` is valid.
pub fn is_valid_transition(current: &Status, next: &Status) -> bool {
    matches!(
        (current, next),
        (Status::Processando, Status::Confirmado)
        | (Status::Processando, Status::Cancelado)
        | (Status::Confirmado, Status::Enviado)
        | (Status::Confirmado, Status::Rejeitado)
        | (Status::Confirmado, Status::Cancelado)
        | (Status::Enviado, Status::Entregue)
        | (Status::Enviado, Status::Cancelado)
        | (Status::Rejeitado, Status::Cancelado)
    )
}

fn compute_total(items: &[OrderItem]) -> Decimal {
    items.iter().fold(Decimal::ZERO, |acc, item| {
        acc + item.unit_price * Decimal::from(item.quantity)
    })
}

pub async fn get_order(db: &PgPool, order_id: i64, customer_id: Uuid) -> Result<CompleteOrder, AppError> {
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    if order.customer_id != customer_id {
        return Err(AppError::Unauthorized);
    }

    let items = sqlx::query_as!(
        OrderItem,
        r#"SELECT id, id_order, id_product, quantity, unit_price, created_at
        FROM items_pedidos WHERE id_order = $1"#,
        order_id
    )
    .fetch_all(db)
    .await
    .map_err(AppError::DbError)?;

    let total = compute_total(&items);
    Ok(CompleteOrder { order, items, total })
}

pub async fn list_orders(
    db: &PgPool,
    jwt_customer_id: Uuid,
    query: &OrderListQuery,
) -> Result<Vec<CompleteOrder>, AppError> {
    let orders = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos
        WHERE customer_id = $1
          AND ($2::order_status IS NULL OR stat = $2)
        ORDER BY created_at DESC
        LIMIT $3"#,
        jwt_customer_id,
        query.status.clone() as Option<Status>,
        query.limit.unwrap_or(50).min(200),
    )
    .fetch_all(db)
    .await
    .map_err(AppError::DbError)?;

    if orders.is_empty() {
        return Ok(vec![]);
    }

    let order_ids: Vec<i64> = orders.iter().map(|o| o.id).collect();

    let all_items = sqlx::query_as!(
        OrderItem,
        r#"SELECT id, id_order, id_product, quantity, unit_price, created_at
        FROM items_pedidos
        WHERE id_order = ANY($1)"#,
        &order_ids[..] as &[i64],
    )
    .fetch_all(db)
    .await
    .map_err(AppError::DbError)?;

    let complete_orders = orders
        .into_iter()
        .map(|order| {
            let items: Vec<OrderItem> = all_items
                .iter()
                .filter(|i| i.id_order == order.id)
                .cloned()
                .collect();
            let total = compute_total(&items);
            CompleteOrder { order, items, total }
        })
        .collect();

    Ok(complete_orders)
}

pub async fn create_order(
    state: Arc<AppState>,
    customer_id: Uuid,
    body: CreateOrderSchema,
) -> Result<CompleteOrder, AppError> {
    if body.items.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "Order must have at least one item".to_string(),
        ));
    }

    let add_items: Vec<AddItemSchema> = body
        .items
        .iter()
        .map(|i| AddItemSchema {
            id_product: i.id_product,
            quantity: i.quantity,
        })
        .collect();

    let validated = validate_items(&state.http, &state.produtos_url, &add_items).await?;

    let mut tx = state.db.begin().await.map_err(AppError::DbError)?;

    let order = sqlx::query_as!(
        Order,
        r#"INSERT INTO pedidos (customer_id) VALUES ($1)
        RETURNING id, customer_id, stat as "stat: Status", created_at, updated_at"#,
        customer_id,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::DbError)?;

    let mut items = Vec::new();
    for v in &validated {
        let item = sqlx::query_as!(
            OrderItem,
            r#"INSERT INTO items_pedidos (id_order, id_product, quantity, unit_price)
            VALUES ($1, $2, $3, $4)
            RETURNING id, id_order, id_product, quantity, unit_price, created_at"#,
            order.id,
            v.id_product,
            v.quantity,
            v.unit_price,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::DbError)?;
        items.push(item);
    }

    tx.commit().await.map_err(AppError::DbError)?;

    // Decrease stock for each validated item after successful commit
    {
        let client = state.http.clone();
        let produtos_url = state.produtos_url.clone();
        let items = validated.clone();
        tokio::spawn(async move {
            for v in &items {
                let new_estoque = (v.current_estoque - v.quantity).max(0);
                update_product_stock(&client, &produtos_url, v.id_product, new_estoque).await;
            }
        });
    }

    // Notify seller via WhatsApp — fire-and-forget, order already committed.
    // JoinHandle intentionally dropped: notify_order handles all errors internally.
    {
        let state_n = Arc::clone(&state);
        let order_id = order.id;
        tokio::spawn(async move {
            crate::notificacao::notify_order(state_n, customer_id, order_id, validated).await;
        });
    }

    let total = compute_total(&items);
    Ok(CompleteOrder { order, items, total })
}

pub async fn update_status(
    state: &AppState,
    order_id: i64,
    customer_id: Uuid,
    body: UpdateStatusSchema,
) -> Result<Order, AppError> {
    let db = &state.db;
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    if order.customer_id != customer_id {
        return Err(AppError::Unauthorized);
    }

    if !is_valid_transition(&order.stat, &body.status) {
        return Err(AppError::UnprocessableEntity(format!(
            "Cannot transition from {:?} to {:?}",
            order.stat, body.status
        )));
    }

    let is_cancelling = matches!(body.status, Status::Cancelado | Status::Rejeitado);

    let updated = sqlx::query_as!(
        Order,
        r#"UPDATE pedidos SET stat = $1, updated_at = NOW()
        WHERE id = $2 AND stat = $3
        RETURNING id, customer_id, stat as "stat: Status", created_at, updated_at"#,
        body.status as Status,
        order_id,
        order.stat as Status,
    )
    .fetch_one(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::UnprocessableEntity(
            "Order status changed concurrently, please retry".to_string(),
        ),
            _ => AppError::DbError(e),
    })?;

    if is_cancelling {
        let items = sqlx::query!(
            "SELECT id_product, quantity FROM items_pedidos WHERE id_order = $1",
            order_id
        )
        .fetch_all(db)
        .await
        .map_err(AppError::DbError)?;

        let client = state.http.clone();
        let url = state.produtos_url.clone();
        tokio::spawn(async move {
            for item in &items {
                adjust_product_stock(&client, &url, item.id_product, item.quantity).await;
            }
        });
    }

    Ok(updated)
}

pub async fn update_items(
    state: &AppState,
    order_id: i64,
    customer_id: Uuid,
    body: UpdateOrderItemsSchema,
) -> Result<CompleteOrder, AppError> {
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    if order.customer_id != customer_id {
        return Err(AppError::Unauthorized);
    }

    if order.stat != Status::Processando {
        return Err(AppError::UnprocessableEntity(
            "Items can only be modified when order status is 'processando'".to_string(),
        ));
    }

    // Guard: at least one operation must be specified
    if body.add.is_none() && body.update.is_none() && body.remove.is_none() {
        return Err(AppError::UnprocessableEntity(
            "At least one of add, update, or remove must be specified".to_string(),
        ));
    }

    // Guard: update quantities must be positive
    if let Some(ref updates) = body.update {
        for u in updates {
            if u.quantity <= 0 {
                return Err(AppError::UnprocessableEntity(
                    format!("quantity must be positive, got {}", u.quantity),
                ));
            }
        }
    }

    // Validate new items before opening transaction
    let validated_adds: Vec<ValidatedItem> = if let Some(ref add) = body.add {
        validate_items(&state.http, &state.produtos_url, add).await?
    } else {
        vec![]
    };

    // Fetch quantities for items being removed (used for stock restoration)
    let removed_items: Vec<(i32, i32)> = if let Some(ref remove_ids) = body.remove {
        let mut items = Vec::new();
        for &item_id in remove_ids {
            let row = sqlx::query!(
                "SELECT id_product, quantity FROM items_pedidos WHERE id = $1 AND id_order = $2",
                item_id,
                order_id
            )
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::DbError)?;
            if let Some(r) = row {
                items.push((r.id_product, r.quantity));
            }
        }
        items
    } else {
        vec![]
    };

    // Fetch old quantities for items being updated (used for stock delta)
    let update_deltas: Vec<(i32, i32)> = if let Some(ref updates) = body.update {
        let mut deltas = Vec::new();
        for u in updates {
            let row = sqlx::query!(
                "SELECT id_product, quantity FROM items_pedidos WHERE id = $1 AND id_order = $2",
                u.id,
                order_id
            )
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::DbError)?;
            if let Some(r) = row {
                let delta = r.quantity - u.quantity;
                if delta != 0 {
                    deltas.push((r.id_product, delta));
                }
            }
        }
        deltas
    } else {
        vec![]
    };

    let mut tx = state.db.begin().await.map_err(AppError::DbError)?;

    // Remove items
    if let Some(remove_ids) = &body.remove {
        for &item_id in remove_ids {
            let del = sqlx::query!(
                "DELETE FROM items_pedidos WHERE id = $1 AND id_order = $2",
                item_id,
                order_id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::DbError)?;

            if del.rows_affected() == 0 {
                return Err(AppError::NotFound {
                    service: "OrderItem".to_string(),
                    id: item_id.to_string(),
                });
            }
        }
    }

    // Update quantities
    if let Some(updates) = &body.update {
        for u in updates {
            let upd = sqlx::query!(
                "UPDATE items_pedidos SET quantity = $1 WHERE id = $2 AND id_order = $3",
                u.quantity,
                u.id,
                order_id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::DbError)?;

            if upd.rows_affected() == 0 {
                return Err(AppError::NotFound {
                    service: "OrderItem".to_string(),
                    id: u.id.to_string(),
                });
            }
        }
    }

    // Add new items
    for v in &validated_adds {
        sqlx::query!(
            r#"INSERT INTO items_pedidos (id_order, id_product, quantity, unit_price)
            VALUES ($1, $2, $3, $4)"#,
            order_id,
            v.id_product,
            v.quantity,
            v.unit_price,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::DbError)?;
    }

    // Update order updated_at
    sqlx::query!("UPDATE pedidos SET updated_at = NOW() WHERE id = $1", order_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::DbError)?;

    tx.commit().await.map_err(AppError::DbError)?;

    // Adjust stock for all changes after successful commit
    {
        let client = state.http.clone();
        let url = state.produtos_url.clone();
        let add_items = validated_adds.clone();
        let remove_stock = removed_items.clone();
        let update_stock = update_deltas.clone();
        tokio::spawn(async move {
            for v in &add_items {
                let new_estoque = (v.current_estoque - v.quantity).max(0);
                update_product_stock(&client, &url, v.id_product, new_estoque).await;
            }
            for (id_product, quantity) in &remove_stock {
                adjust_product_stock(&client, &url, *id_product, *quantity).await;
            }
            for (id_product, delta) in &update_stock {
                adjust_product_stock(&client, &url, *id_product, *delta).await;
            }
        });
    }

    get_order(&state.db, order_id, customer_id).await
}

pub async fn delete_order(
    db: &PgPool,
    order_id: i64,
    customer_id: Uuid,
) -> Result<Order, AppError> {
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    if order.customer_id != customer_id {
        return Err(AppError::Unauthorized);
    }

    if !matches!(order.stat, Status::Processando | Status::Cancelado) {
        return Err(AppError::UnprocessableEntity(
            "Order can only be deleted when status is 'processando' or 'cancelado'".to_string(),
        ));
    }

    let result = sqlx::query!(
        "DELETE FROM pedidos WHERE id = $1 AND stat = $2",
        order_id,
        order.stat.clone() as Status,
    )
    .execute(db)
    .await
    .map_err(AppError::DbError)?;

    if result.rows_affected() == 0 {
        return Err(AppError::UnprocessableEntity(
            "Order status changed concurrently, please retry".to_string(),
        ));
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions_are_accepted() {
        assert!(is_valid_transition(&Status::Processando, &Status::Confirmado));
        assert!(is_valid_transition(&Status::Processando, &Status::Cancelado));
        assert!(is_valid_transition(&Status::Confirmado, &Status::Enviado));
        assert!(is_valid_transition(&Status::Confirmado, &Status::Rejeitado));
        assert!(is_valid_transition(&Status::Confirmado, &Status::Cancelado));
        assert!(is_valid_transition(&Status::Enviado, &Status::Entregue));
        assert!(is_valid_transition(&Status::Enviado, &Status::Cancelado));
        assert!(is_valid_transition(&Status::Rejeitado, &Status::Cancelado));
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        assert!(!is_valid_transition(&Status::Entregue, &Status::Cancelado));
        assert!(!is_valid_transition(&Status::Cancelado, &Status::Confirmado));
        assert!(!is_valid_transition(&Status::Processando, &Status::Entregue));
        assert!(!is_valid_transition(&Status::Enviado, &Status::Processando));
        assert!(!is_valid_transition(&Status::Entregue, &Status::Processando));
    }

    #[test]
    fn complete_orders_assembled_correctly() {
        use chrono::Utc;
        use uuid::Uuid;

        let customer = Uuid::new_v4();
        let now = Utc::now();

        let orders = vec![
            Order {
                id: 1,
                customer_id: customer,
                stat: Status::Processando,
                created_at: now,
                updated_at: now,
            },
            Order {
                id: 2,
                customer_id: customer,
                stat: Status::Confirmado,
                created_at: now,
                updated_at: now,
            },
        ];

        let all_items = vec![
            OrderItem { id: 10, id_order: 1, id_product: 100, quantity: 2, unit_price: Decimal::new(1000, 2), created_at: now },
            OrderItem { id: 11, id_order: 2, id_product: 200, quantity: 1, unit_price: Decimal::new(5000, 2), created_at: now },
            OrderItem { id: 12, id_order: 1, id_product: 101, quantity: 3, unit_price: Decimal::new(500, 2),  created_at: now },
        ];

        let complete: Vec<CompleteOrder> = orders
            .into_iter()
            .map(|order| {
                let items: Vec<OrderItem> = all_items
                    .iter()
                    .filter(|i| i.id_order == order.id)
                    .cloned()
                    .collect();
                let total = compute_total(&items);
                CompleteOrder { order, items, total }
            })
            .collect();

        // Order 1: 2×10.00 + 3×5.00 = 35.00
        assert_eq!(complete[0].items.len(), 2);
        assert_eq!(complete[0].total, Decimal::new(3500, 2));

        // Order 2: 1×50.00 = 50.00
        assert_eq!(complete[1].items.len(), 1);
        assert_eq!(complete[1].total, Decimal::new(5000, 2));
    }

    #[test]
    fn compute_total_sums_correctly() {
        use chrono::Utc;
        let items = vec![
            OrderItem {
                id: 1,
                id_order: 1,
                id_product: 1,
                quantity: 3,
                unit_price: Decimal::new(1500, 2), // 15.00
                created_at: Utc::now(),
            },
            OrderItem {
                id: 2,
                id_order: 1,
                id_product: 2,
                quantity: 2,
                unit_price: Decimal::new(1000, 2), // 10.00
                created_at: Utc::now(),
            },
        ];
        let total = compute_total(&items);
        assert_eq!(total, Decimal::new(6500, 2)); // 65.00
    }
}
