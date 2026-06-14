use axum::{
    RequestPartsExt,
    extract::{FromRef, FromRequestParts, State},
    http::request::Parts,
};
use axum_extra::{TypedHeader, headers::{Authorization, authorization::Bearer}};
use errors::errors::AppError;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::models::AppState;

#[derive(Debug, Deserialize, Serialize)]
pub struct Claims {
    pub customer_id: Uuid,
    pub exp: u64,
}

#[derive(Debug)]
pub struct JwtCustomer(pub Uuid);

impl<S> FromRequestParts<S> for JwtCustomer
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::Unauthorized)?;

        let State(app_state): State<Arc<AppState>> = parts
            .extract_with_state(state)
            .await
            .map_err(|_| AppError::Unauthorized)?;

        let token_data = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(app_state.jwt_secret.as_bytes()),
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

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn make_token(customer_id: Uuid, secret: &str, exp_offset_secs: i64) -> String {
        let exp = if exp_offset_secs >= 0 {
            now_secs() + exp_offset_secs as u64
        } else {
            now_secs().saturating_sub((-exp_offset_secs) as u64)
        };
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
        let token = make_token(id, secret, 3600);

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
        let token = make_token(id, "secret_a", 3600);

        let result = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(b"secret_b"),
            &Validation::default(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn expired_token_is_rejected() {
        let id = Uuid::new_v4();
        let secret = "test_secret";
        // Token expired 2 hours ago — well beyond the 60-second leeway
        let token = make_token(id, secret, -7200);

        let result = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        );

        assert!(result.is_err());
    }
}
