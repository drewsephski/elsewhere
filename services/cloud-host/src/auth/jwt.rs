use std::collections::HashMap;
use std::sync::Arc;

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct JwtVerifierConfig {
    pub jwks_url: String,
    pub issuer: String,
    pub audience: String,
}

#[derive(Debug, Clone, Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    crv: Option<String>,
    x: Option<String>,
    y: Option<String>,
    alg: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Claims {
    sub: String,
    iss: String,
    aud: serde_json::Value,
    exp: i64,
}

pub struct JwtVerifier {
    client: Client,
    config: JwtVerifierConfig,
    keys: RwLock<HashMap<String, DecodingKey>>,
}

impl JwtVerifier {
    pub fn new(config: JwtVerifierConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("jwks http client"),
            config,
            keys: RwLock::new(HashMap::new()),
        }
    }

    pub async fn verify_bearer_token(&self, token: &str) -> Result<String, ApiError> {
        let header = decode_header(token).map_err(|_| ApiError::Unauthorized)?;
        let kid = header.kid.ok_or(ApiError::Unauthorized)?;
        let alg = header.alg;
        if alg != Algorithm::ES256 {
            return Err(ApiError::Unauthorized);
        }

        let key = self.decoding_key_for_kid(&kid).await?;
        let mut validation = Validation::new(Algorithm::ES256);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);
        validation.validate_exp = true;
        validation.leeway = 0;

        let data =
            decode::<Claims>(token, &key, &validation).map_err(|_| ApiError::Unauthorized)?;
        if data.claims.sub.trim().is_empty() {
            return Err(ApiError::Unauthorized);
        }
        Ok(data.claims.sub)
    }

    async fn decoding_key_for_kid(&self, kid: &str) -> Result<DecodingKey, ApiError> {
        {
            let cache = self.keys.read().await;
            if let Some(key) = cache.get(kid) {
                return Ok(key.clone());
            }
        }
        self.refresh_jwks().await?;
        let cache = self.keys.read().await;
        cache.get(kid).cloned().ok_or(ApiError::Unauthorized)
    }

    async fn refresh_jwks(&self) -> Result<(), ApiError> {
        let response = self
            .client
            .get(&self.config.jwks_url)
            .send()
            .await
            .map_err(|e| ApiError::Internal(format!("jwks fetch failed: {e}")))?;
        if !response.status().is_success() {
            return Err(ApiError::Internal(format!(
                "jwks fetch status {}",
                response.status()
            )));
        }
        let body: JwksResponse = response
            .json()
            .await
            .map_err(|e| ApiError::Internal(format!("jwks parse failed: {e}")))?;

        let mut next = HashMap::new();
        for jwk in body.keys {
            if jwk.kty != "EC" {
                continue;
            }
            if jwk.crv.as_deref() != Some("P-256") {
                continue;
            }
            let Some(kid) = jwk.kid.filter(|k| !k.is_empty()) else {
                continue;
            };
            let Some(x) = jwk.x.filter(|v| !v.is_empty()) else {
                continue;
            };
            let Some(y) = jwk.y.filter(|v| !v.is_empty()) else {
                continue;
            };
            let key =
                DecodingKey::from_ec_components(&x, &y).map_err(|_| ApiError::Unauthorized)?;
            next.insert(kid, key);
        }
        if next.is_empty() {
            return Err(ApiError::Internal(
                "jwks contained no usable ES256 keys".into(),
            ));
        }
        *self.keys.write().await = next;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn from_test_decoding_key(
        kid: &str,
        key: DecodingKey,
        config: JwtVerifierConfig,
    ) -> Arc<Self> {
        let mut map = HashMap::new();
        map.insert(kid.to_string(), key);
        Arc::new(Self {
            client: Client::new(),
            config,
            keys: RwLock::new(map),
        })
    }
}
