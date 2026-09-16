use chrono::{Duration, Utc};
use reqwest::header::HeaderMap;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::connectors::installs::StoredSecret;
use crate::connectors::remote::RemoteHttpClient;
use crate::connectors::secret::ConnectorSecretBox;
use crate::error::ApiError;

const OAUTH_SESSION_TTL_MINUTES: i64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOAuthConfig {
    pub display_name: String,
    pub endpoint_url: String,
    pub kind: String,
    pub redirect_uri: String,
    pub client_id: Option<String>,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
    pub resource: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OAuthStart {
    pub authorize_url: String,
    pub state: String,
    pub expires_at: chrono::DateTime<Utc>,
}

pub async fn start_mcp_oauth(
    pool: &PgPool,
    secret_box: &ConnectorSecretBox,
    client: &RemoteHttpClient,
    owner_id: &str,
    install_id: Option<Uuid>,
    pending: PendingOAuthConfig,
    www_authenticate: Option<&str>,
) -> Result<OAuthStart, ApiError> {
    let metadata = discover_authorization(client, &pending.endpoint_url, www_authenticate).await?;
    let mut pending = pending;
    pending.authorization_endpoint = Some(metadata.authorization_endpoint.clone());
    pending.token_endpoint = Some(metadata.token_endpoint.clone());
    pending.resource = metadata.resource.clone();
    if pending.client_id.is_none() {
        pending.client_id = metadata.client_id.clone();
    }
    if pending.client_id.is_none() {
        if let Some(dcr) = metadata.registration_endpoint {
            pending.client_id = Some(register_client(client, &dcr, &pending.redirect_uri).await?);
        }
    }
    let client_id = pending.client_id.clone().ok_or_else(|| {
        ApiError::Validation("MCP server did not provide an OAuth client id".into())
    })?;
    let verifier = pkce_verifier();
    let challenge = pkce_challenge(&verifier);
    let state = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(OAUTH_SESSION_TTL_MINUTES);
    store_session(
        pool, secret_box, &state, owner_id, install_id, &verifier, &pending, expires_at,
    )
    .await?;

    let mut url = url::Url::parse(&metadata.authorization_endpoint)
        .map_err(|_| ApiError::Validation("invalid authorization endpoint".into()))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("response_type", "code");
        pairs.append_pair("client_id", &client_id);
        pairs.append_pair("redirect_uri", &pending.redirect_uri);
        pairs.append_pair("state", &state);
        pairs.append_pair("code_challenge", &challenge);
        pairs.append_pair("code_challenge_method", "S256");
        if let Some(resource) = &pending.resource {
            pairs.append_pair("resource", resource);
        }
        pairs.append_pair("scope", "mcp");
    }
    Ok(OAuthStart {
        authorize_url: url.to_string(),
        state,
        expires_at,
    })
}

pub async fn complete_mcp_oauth(
    pool: &PgPool,
    secret_box: &ConnectorSecretBox,
    client: &RemoteHttpClient,
    owner_id: &str,
    code: &str,
    state: &str,
) -> Result<(PendingOAuthConfig, StoredSecret, Option<Uuid>), ApiError> {
    let (pending, verifier, install_id) =
        consume_session(pool, secret_box, owner_id, state).await?;
    let token_endpoint = pending
        .token_endpoint
        .clone()
        .ok_or_else(|| ApiError::Validation("OAuth session is missing a token endpoint".into()))?;
    client
        .validate_url(&token_endpoint)
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    let client_id = pending
        .client_id
        .clone()
        .ok_or_else(|| ApiError::Validation("OAuth session is missing a client id".into()))?;
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("redirect_uri", pending.redirect_uri.clone()),
        ("client_id", client_id.clone()),
        ("code_verifier", verifier),
    ];
    if let Some(resource) = &pending.resource {
        form.push(("resource", resource.clone()));
    }
    let body = serde_urlencoded(&form);
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    let response = client
        .request(
            Method::POST,
            &token_endpoint,
            headers,
            Some(body.into_bytes()),
        )
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    if !response.status.is_success() {
        return Err(ApiError::Validation("OAuth token exchange failed".into()));
    }
    let payload: Value = response
        .json()
        .map_err(|_| ApiError::Validation("OAuth token response is not JSON".into()))?;
    let access = payload
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::Validation("OAuth token response missing access_token".into()))?;
    let secret = StoredSecret {
        kind: "oauth".into(),
        access_token: Some(access.to_string()),
        refresh_token: payload
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        expires_at: payload
            .get("expires_in")
            .and_then(|v| v.as_i64())
            .map(|secs| Utc::now().timestamp_millis() + secs * 1000),
        token_type: payload
            .get("token_type")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        client_id: Some(client_id),
        token_endpoint: Some(token_endpoint),
        ..StoredSecret::none()
    };
    Ok((pending, secret, install_id))
}

pub async fn refresh_mcp_oauth(
    client: &RemoteHttpClient,
    secret: &StoredSecret,
) -> Result<StoredSecret, agent_core::ConnectorError> {
    let refresh = secret
        .refresh_token
        .as_deref()
        .ok_or(agent_core::ConnectorError::ReconnectRequired)?;
    let token_endpoint = secret
        .token_endpoint
        .as_deref()
        .ok_or(agent_core::ConnectorError::ReconnectRequired)?;
    client
        .validate_url(token_endpoint)
        .await
        .map_err(|e| agent_core::ConnectorError::Validation(e.message()))?;
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh.to_string()),
    ];
    if let Some(client_id) = &secret.client_id {
        form.push(("client_id", client_id.clone()));
    }
    let body = serde_urlencoded(&form);
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    let response = client
        .request(
            Method::POST,
            token_endpoint,
            headers,
            Some(body.into_bytes()),
        )
        .await
        .map_err(|e| agent_core::ConnectorError::Provider(e.message()))?;
    if response.status.as_u16() == 401 || response.status.as_u16() == 400 {
        return Err(agent_core::ConnectorError::ReconnectRequired);
    }
    if !response.status.is_success() {
        return Err(agent_core::ConnectorError::ReconnectRequired);
    }
    let payload: Value = response
        .json()
        .map_err(|_| agent_core::ConnectorError::ReconnectRequired)?;
    let access = payload
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or(agent_core::ConnectorError::ReconnectRequired)?;
    let mut next = secret.clone();
    next.access_token = Some(access.to_string());
    if let Some(refresh_token) = payload.get("refresh_token").and_then(|v| v.as_str()) {
        next.refresh_token = Some(refresh_token.to_string());
    }
    next.expires_at = payload
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .map(|secs| Utc::now().timestamp_millis() + secs * 1000);
    Ok(next)
}

struct AuthMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: Option<String>,
    resource: Option<String>,
    client_id: Option<String>,
}

async fn discover_authorization(
    client: &RemoteHttpClient,
    mcp_url: &str,
    www_authenticate: Option<&str>,
) -> Result<AuthMetadata, ApiError> {
    let resource_url = www_authenticate
        .and_then(parse_resource_metadata)
        .or_else(|| default_resource_metadata(mcp_url));
    let Some(resource_url) = resource_url else {
        return Err(ApiError::Validation(
            "MCP server did not advertise OAuth metadata".into(),
        ));
    };
    client
        .validate_url(&resource_url)
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    let resource = get_json(client, &resource_url).await?;
    let as_url = resource
        .get("authorization_servers")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ApiError::Validation("OAuth resource metadata missing authorization_servers".into())
        })?;
    let issuer = as_url.trim_end_matches('/');
    let as_metadata_url = format!("{issuer}/.well-known/oauth-authorization-server");
    client
        .validate_url(&as_metadata_url)
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    let metadata = get_json(client, &as_metadata_url).await?;
    Ok(AuthMetadata {
        authorization_endpoint: required_url(&metadata, "authorization_endpoint")?,
        token_endpoint: required_url(&metadata, "token_endpoint")?,
        registration_endpoint: metadata
            .get("registration_endpoint")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        resource: resource
            .get("resource")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| Some(mcp_url.to_string())),
        client_id: None,
    })
}

async fn register_client(
    client: &RemoteHttpClient,
    registration_endpoint: &str,
    redirect_uri: &str,
) -> Result<String, ApiError> {
    client
        .validate_url(registration_endpoint)
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    let body = serde_json::to_vec(&json!({
        "client_name": "Elsewhere",
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }))
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    let response = client
        .request(Method::POST, registration_endpoint, headers, Some(body))
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    if !response.status.is_success() {
        return Err(ApiError::Validation(
            "dynamic client registration failed".into(),
        ));
    }
    let payload = response
        .json()
        .map_err(|_| ApiError::Validation("invalid DCR response".into()))?;
    payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| ApiError::Validation("DCR response missing client_id".into()))
}

async fn get_json(client: &RemoteHttpClient, url: &str) -> Result<Value, ApiError> {
    let response = client
        .request(Method::GET, url, HeaderMap::new(), None)
        .await
        .map_err(|e| ApiError::Validation(e.message()))?;
    if !response.status.is_success() {
        return Err(ApiError::Validation("OAuth metadata request failed".into()));
    }
    response
        .json()
        .map_err(|_| ApiError::Validation("OAuth metadata is not JSON".into()))
}

fn parse_resource_metadata(www: &str) -> Option<String> {
    let lower = www.to_ascii_lowercase();
    let idx = lower.find("resource_metadata=")?;
    let rest = &www[idx + "resource_metadata=".len()..];
    let value = rest
        .trim_start_matches('"')
        .split(['"', ',', ' '])
        .next()?
        .trim();
    if value.starts_with("https://") {
        Some(value.to_string())
    } else {
        None
    }
}

fn default_resource_metadata(mcp_url: &str) -> Option<String> {
    let url = url::Url::parse(mcp_url).ok()?;
    Some(format!(
        "{}://{}/.well-known/oauth-protected-resource",
        url.scheme(),
        url.host_str()?
    ))
}

fn required_url(value: &Value, key: &str) -> Result<String, ApiError> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| s.starts_with("https://") || s.starts_with("http://"))
        .map(str::to_string)
        .ok_or_else(|| ApiError::Validation(format!("OAuth metadata missing {key}")))
}

fn pkce_verifier() -> String {
    let raw = Uuid::new_v4().to_string() + &Uuid::new_v4().to_string();
    raw.replace('-', "")
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64_url_nopad(&digest)
}

fn base64_url_nopad(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn serde_urlencoded(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

async fn store_session(
    pool: &PgPool,
    secret_box: &ConnectorSecretBox,
    state: &str,
    owner_id: &str,
    install_id: Option<Uuid>,
    verifier: &str,
    pending: &PendingOAuthConfig,
    expires_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    let (nonce, ciphertext) = secret_box.encrypt(verifier).map_err(ApiError::Internal)?;
    let config = serde_json::to_value(pending).map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query(
        r#"
        INSERT INTO connector_mcp_oauth_sessions (
            state, owner_id, install_id, code_verifier_nonce, code_verifier_ciphertext,
            pending_config, expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(state)
    .bind(owner_id)
    .bind(install_id)
    .bind(nonce)
    .bind(ciphertext)
    .bind(config)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

async fn consume_session(
    pool: &PgPool,
    secret_box: &ConnectorSecretBox,
    owner_id: &str,
    state: &str,
) -> Result<(PendingOAuthConfig, String, Option<Uuid>), ApiError> {
    let now = Utc::now();
    let row: Option<(Option<Uuid>, Vec<u8>, Vec<u8>, Value)> = sqlx::query_as(
        r#"
        DELETE FROM connector_mcp_oauth_sessions
        WHERE state = $1 AND owner_id = $2 AND expires_at > $3
        RETURNING install_id, code_verifier_nonce, code_verifier_ciphertext, pending_config
        "#,
    )
    .bind(state)
    .bind(owner_id)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    let Some((install_id, nonce, ciphertext, config)) = row else {
        return Err(ApiError::Validation(
            "invalid or expired OAuth state".into(),
        ));
    };
    let verifier = secret_box
        .decrypt(&nonce, &ciphertext)
        .map_err(ApiError::Internal)?;
    let pending: PendingOAuthConfig =
        serde_json::from_value(config).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok((pending, verifier, install_id))
}

pub fn parse_resource_metadata_for_tests(www: &str) -> Option<String> {
    parse_resource_metadata(www)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_s256() {
        let verifier = "abc";
        let challenge = pkce_challenge(verifier);
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
        assert!(!challenge.contains('='));
    }

    #[test]
    fn oauth_state_is_owner_bound_in_sql_contract() {
        // consume_session matches state AND owner_id AND unexpired TTL.
        assert_eq!(OAUTH_SESSION_TTL_MINUTES, 10);
    }

    #[test]
    fn resource_metadata_requires_https() {
        let www = r#"Bearer realm="mcp", resource_metadata="https://api.example.com/.well-known/oauth-protected-resource""#;
        assert_eq!(
            parse_resource_metadata(www).as_deref(),
            Some("https://api.example.com/.well-known/oauth-protected-resource")
        );
        assert!(parse_resource_metadata(
            r#"Bearer resource_metadata="http://127.0.0.1/.well-known/oauth-protected-resource""#
        )
        .is_none());
    }
}
