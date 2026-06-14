use std::sync::Arc;

use chrono::Local;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{models::AppState, produto_client::ValidatedItem};

// ---------------------------------------------------------------------------
// Customer data
// ---------------------------------------------------------------------------

pub struct ClienteInfo {
    pub nome: String,
    pub telefone: String,
}

// ---------------------------------------------------------------------------
// Mock clientes client
// ---------------------------------------------------------------------------

/// Stub for the future `clientes` microservice.
///
/// TODO: Replace this entire struct with a real HTTP client that calls
///       `GET {CLIENTES_SERVICE_URL}/clientes/{id}` and deserialises the
///       response into `ClienteInfo`. Remove MOCK_NOME and MOCK_TELEFONE
///       constants once the real integration is in place.
struct ClientesMock;

const MOCK_NOME: &str = "Cliente de Teste";
const MOCK_TELEFONE: &str = "5511999999999";

impl ClientesMock {
    async fn get(_id: Uuid) -> ClienteInfo {
        tracing::warn!("notificacao: using mock clientes client — replace with real service before production");
        ClienteInfo {
            nome: MOCK_NOME.to_string(),
            telefone: MOCK_TELEFONE.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Message formatting
// ---------------------------------------------------------------------------

/// Build the WhatsApp message text for a new order.
/// Pure function — accepts `now` as a parameter for full testability.
pub fn build_message(
    order_id: i64,
    cliente: &ClienteInfo,
    items: &[ValidatedItem],
    now: chrono::DateTime<chrono::Local>,
) -> String {
    let itens_fmt: String = items
        .iter()
        .map(|i| {
            let subtotal = i.unit_price * Decimal::from(i.quantity);
            format!("• {}x {} — R$ {subtotal:.2}", i.quantity, i.nome)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let total: Decimal = items
        .iter()
        .fold(Decimal::ZERO, |acc, i| acc + i.unit_price * Decimal::from(i.quantity));

    let now = now.format("%d/%m/%Y às %H:%M");

    format!(
        "🛒 *Novo Pedido #{order_id}*\n\n\
        *Cliente:* {nome}\n\
        *Telefone:* {telefone}\n\n\
        *Itens:*\n{itens_fmt}\n\n\
        *Total:* R$ {total:.2}\n\n\
        _Recebido em {now}_",
        nome = cliente.nome,
        telefone = cliente.telefone,
    )
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Fire-and-forget: fetch customer, build message, send via Evolution API.
/// All errors are logged; none propagate to the caller.
pub async fn notify_order(
    state: Arc<AppState>,
    customer_id: Uuid,
    order_id: i64,
    items: Vec<ValidatedItem>,
) {
    let cliente = ClientesMock::get(customer_id).await;
    let message = build_message(order_id, &cliente, &items, Local::now());

    let url = format!(
        "{}/message/sendText/{}",
        state.evolution_url, state.evolution_instance
    );

    let body = serde_json::json!({
        "number": state.seller_whatsapp,
        "text": message,
    });

    match state
        .http
        .post(&url)
        .header("apikey", &state.evolution_key)
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(order_id, "WhatsApp notification sent successfully");
        }
        Ok(resp) => {
            tracing::error!(
                order_id,
                status = %resp.status(),
                "Evolution API returned non-success status"
            );
        }
        Err(e) => {
            tracing::error!(order_id, error = %e, "Failed to reach Evolution API");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn item(nome: &str, quantity: i32, unit_price: &str) -> ValidatedItem {
        ValidatedItem {
            id_product: 1,
            nome: nome.to_string(),
            quantity,
            unit_price: dec(unit_price),
        }
    }

    fn cliente(nome: &str, telefone: &str) -> ClienteInfo {
        ClienteInfo {
            nome: nome.to_string(),
            telefone: telefone.to_string(),
        }
    }

    #[test]
    fn build_message_contains_order_id() {
        let msg = build_message(42, &cliente("João", "5511999"), &[item("Prod A", 1, "10.00")], chrono::Local::now());
        assert!(msg.contains("Novo Pedido #42"), "missing order id in: {msg}");
    }

    #[test]
    fn build_message_contains_client_name_and_phone() {
        let msg = build_message(1, &cliente("Maria Silva", "5521888777"), &[item("X", 1, "1.00")], chrono::Local::now());
        assert!(msg.contains("Maria Silva"), "missing name in: {msg}");
        assert!(msg.contains("5521888777"), "missing phone in: {msg}");
    }

    #[test]
    fn build_message_computes_total_correctly() {
        // 2 × 10.00 + 3 × 5.00 = 35.00
        let items = vec![
            item("A", 2, "10.00"),
            item("B", 3, "5.00"),
        ];
        let msg = build_message(1, &cliente("T", "0"), &items, chrono::Local::now());
        assert!(msg.contains("35.00"), "expected total 35.00 in: {msg}");
    }

    #[test]
    fn build_message_lists_all_item_names() {
        let items = vec![
            item("Alto Falante Hurricane", 1, "164.00"),
            item("Radio MP3", 2, "39.90"),
        ];
        let msg = build_message(1, &cliente("T", "0"), &items, chrono::Local::now());
        assert!(msg.contains("Alto Falante Hurricane"), "missing item 1 in: {msg}");
        assert!(msg.contains("Radio MP3"), "missing item 2 in: {msg}");
    }

    #[test]
    fn build_message_per_line_shows_subtotal() {
        // 2 × 50.00 = 100.00 should appear on that line
        let msg = build_message(1, &cliente("T", "0"), &[item("Prod", 2, "50.00")], chrono::Local::now());
        assert!(msg.contains("100.00"), "expected line subtotal 100.00 in: {msg}");
    }

    #[test]
    fn build_message_contains_formatted_timestamp() {
        use chrono::TimeZone;
        // 2026-06-14 15:32:00 local time
        let now = chrono::Local.with_ymd_and_hms(2026, 6, 14, 15, 32, 0).unwrap();
        let msg = build_message(1, &cliente("T", "0"), &[item("X", 1, "1.00")], now);
        assert!(msg.contains("14/06/2026 às 15:32"), "expected timestamp in: {msg}");
    }
}
