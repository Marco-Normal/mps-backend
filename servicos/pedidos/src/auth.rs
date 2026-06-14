use axum::{
    RequestPartsExt,
    extract::FromRequestParts,
    http::request::Parts,
};
use axum_extra::{TypedHeader, headers::{Authorization, authorization::Bearer}};
use errors::errors::AppError;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub customer_id: Uuid,
    pub exp: usize,
}

pub struct JwtCustomer(pub Uuid);

impl<S> FromRequestParts<S> for JwtCustomer
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::Unauthorized)?;

        let secret = std::env::var("JWT_SECRET").map_err(|_| AppError::Unauthorized)?;

        let token_data = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?;

        Ok(JwtCustomer(token_data.claims.customer_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_token(customer_id: Uuid, secret: &str) -> String {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize + 3600;
        let claims = Claims { customer_id, exp };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn valid_token_decodes_customer_id() {
        let id = Uuid::new_v4();
        let secret = "test_secret";
        let token = make_token(id, secret);

        let decoded = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .unwrap();

        assert_eq!(decoded.claims.customer_id, id);
    }

    #[test]
    fn wrong_secret_fails() {
        let id = Uuid::new_v4();
        let token = make_token(id, "secret_a");

        let result = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(b"secret_b"),
            &Validation::default(),
        );

        assert!(result.is_err());
    }
}
