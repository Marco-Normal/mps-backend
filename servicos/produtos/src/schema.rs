use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};

/// Custom deserializer that distinguishes absent from explicit `null`:
/// - absent  → outer `None`   (via `#[serde(default)]`)
/// - `null`  → `Some(None)`
/// - `"str"` → `Some(Some("str"))`
fn deserialize_option_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(de)?))
}

#[derive(Deserialize, Debug)]
pub struct ProductSchema {
    pub nome: String,
    pub marca: String,
    pub num_fab: Option<String>,
    pub unidade: String,
    pub valor: Decimal,
    pub descricao: Option<String>,
    pub estoque: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct UpdateProductSchema {
    pub nome: Option<String>,
    pub marca: Option<String>,
    /// `None` = absent (keep existing), `Some(None)` = explicit null (clear), `Some(Some(s))` = update
    #[serde(default, deserialize_with = "deserialize_option_option")]
    pub num_fab: Option<Option<String>>,
    pub unidade: Option<String>,
    pub valor: Option<Decimal>,
    /// `None` = absent (keep existing), `Some(None)` = explicit null (clear), `Some(Some(s))` = update
    #[serde(default, deserialize_with = "deserialize_option_option")]
    pub descricao: Option<Option<String>>,
    pub estoque: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct ProductSearchSchema {
    pub q: String,
    pub limit: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descricao_absent_deserializes_to_none() {
        let s = r#"{"nome": "test", "valor": 1.0, "marca": "X", "unidade": "PC"}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert!(schema.descricao.is_none(), "absent field should be None (outer)");
    }

    #[test]
    fn descricao_null_deserializes_to_some_none() {
        let s = r#"{"descricao": null}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert_eq!(schema.descricao, Some(None), "explicit null should be Some(None)");
    }

    #[test]
    fn descricao_string_deserializes_to_some_some() {
        let s = r#"{"descricao": "hello"}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert_eq!(schema.descricao, Some(Some("hello".to_string())));
    }

    #[test]
    fn num_fab_absent_deserializes_to_none() {
        let s = r#"{}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert!(schema.num_fab.is_none());
    }

    #[test]
    fn num_fab_null_deserializes_to_some_none() {
        let s = r#"{"num_fab": null}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert_eq!(schema.num_fab, Some(None));
    }
}
