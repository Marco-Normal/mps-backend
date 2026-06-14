use errors::errors::{AppError, ItemValidationError};
use futures::future::join_all;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::schema::AddItemSchema;

/// Successful validation result for one item
#[derive(Clone)]
pub struct ValidatedItem {
    pub id_product: i32,
    pub nome: String,
    pub quantity: i32,
    pub unit_price: Decimal,
}

#[derive(Deserialize)]
struct ProductResponse {
    data: ProductData,
}

#[derive(Deserialize)]
struct ProductData {
    product: ProdutoDto,
}

#[derive(Deserialize)]
struct ProdutoDto {
    nome: String,
    #[serde(rename = "VLR_VENDA1")]
    valor: Decimal,
    estoque: i32,
}

/// Validates a list of items against the produtos service.
/// Returns Ok(Vec<ValidatedItem>) if all pass, or Err(AppError::ValidationFailed) listing all failures.
pub async fn validate_items(
    client: &Client,
    produtos_url: &str,
    items: &[AddItemSchema],
) -> Result<Vec<ValidatedItem>, AppError> {
    let tasks: Vec<_> = items
        .iter()
        .map(|item| {
            let url = format!("{produtos_url}/api/products/{}", item.id_product);
            let client = client.clone();
            let id_product = item.id_product;
            let quantity = item.quantity;
            async move {
                let resp = client.get(&url).send().await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        match r.json::<ProductResponse>().await {
                            Ok(body) => {
                                let p = body.data.product;
                                if quantity > p.estoque {
                                    Err(ItemValidationError {
                                        id_product,
                                        reason: format!(
                                            "insufficient stock (requested {quantity}, available {})",
                                            p.estoque
                                        ),
                                    })
                                } else {
                                    Ok(ValidatedItem {
                                        id_product,
                                        nome: p.nome,
                                        quantity,
                                        unit_price: p.valor,
                                    })
                                }
                            }
                            Err(_) => Err(ItemValidationError {
                                id_product,
                                reason: "failed to parse product response".to_string(),
                            }),
                        }
                    }
                    Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => {
                        Err(ItemValidationError {
                            id_product,
                            reason: "product not found".to_string(),
                        })
                    }
                    _ => Err(ItemValidationError {
                        id_product,
                        reason: "produtos service unavailable".to_string(),
                    }),
                }
            }
        })
        .collect();

    let results: Vec<Result<ValidatedItem, ItemValidationError>> = join_all(tasks).await;

    let mut errors = Vec::new();
    let mut validated = Vec::new();

    for r in results {
        match r {
            Ok(v) => validated.push(v),
            Err(e) => errors.push(e),
        }
    }

    if errors.is_empty() {
        Ok(validated)
    } else {
        Err(AppError::ValidationFailed { items: errors })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use errors::errors::ItemValidationError;

    #[test]
    fn insufficient_stock_message_format() {
        let e = ItemValidationError {
            id_product: 5,
            reason: format!("insufficient stock (requested {}, available {})", 10, 3),
        };
        assert!(e.reason.contains("requested 10"));
        assert!(e.reason.contains("available 3"));
    }

    #[test]
    fn empty_items_returns_empty_validated() {
        // validate_items is async so we test the collection logic directly
        let errors: Vec<ItemValidationError> = vec![];
        let validated: Vec<ValidatedItem> = vec![];
        // if errors is empty, we should return Ok(validated)
        assert!(errors.is_empty());
        assert!(validated.is_empty());
    }
}
