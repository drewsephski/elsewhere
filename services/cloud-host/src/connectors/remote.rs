//! Connector-specific SSRF-safe outbound HTTP.
//!
//! This is independent of browser URL validation and is stricter:
//! HTTPS for hosted endpoints, no redirects, pinned DNS, no proxy.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use reqwest::{Client, Method, StatusCode};
use serde_json::Value;
use url::Url;

pub const DEFAULT_REMOTE_TIMEOUT: Duration = Duration::from_secs(15);
pub const MAX_REMOTE_BODY_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct RemotePolicy {
    pub allow_loopback: bool,
    pub timeout: Duration,
    pub max_body_bytes: usize,
}

impl RemotePolicy {
    pub fn production() -> Self {
        Self {
            allow_loopback: false,
            timeout: DEFAULT_REMOTE_TIMEOUT,
            max_body_bytes: MAX_REMOTE_BODY_BYTES,
        }
    }

    pub fn for_tests() -> Self {
        Self {
            allow_loopback: true,
            timeout: Duration::from_secs(5),
            max_body_bytes: MAX_REMOTE_BODY_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteError {
    Blocked(String),
    Timeout,
    Network(String),
    Redirect,
    TooLarge,
    Http(u16),
}

impl RemoteError {
    pub fn message(&self) -> String {
        match self {
            RemoteError::Blocked(m) => m.clone(),
            RemoteError::Timeout => "remote request timed out".into(),
            RemoteError::Network(m) => format!("remote request failed: {m}"),
            RemoteError::Redirect => "remote redirects are not allowed".into(),
            RemoteError::TooLarge => "remote response is too large".into(),
            RemoteError::Http(code) => format!("remote endpoint returned HTTP {code}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedRemoteTarget {
    pub url: Url,
    pub host: String,
    pub pinned_addrs: Vec<SocketAddr>,
}

#[derive(Debug, Clone)]
pub struct RemoteResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl RemoteResponse {
    pub fn text(&self) -> Result<String, RemoteError> {
        String::from_utf8(self.body.clone())
            .map_err(|_| RemoteError::Network("remote response is not valid UTF-8".into()))
    }

    pub fn json(&self) -> Result<Value, RemoteError> {
        serde_json::from_slice(&self.body)
            .map_err(|_| RemoteError::Network("remote response is not JSON".into()))
    }
}

#[derive(Debug, Clone)]
pub struct RemoteHttpClient {
    policy: RemotePolicy,
}

impl RemoteHttpClient {
    pub fn new(policy: RemotePolicy) -> Self {
        Self { policy }
    }

    pub async fn validate_url(&self, raw: &str) -> Result<ValidatedRemoteTarget, RemoteError> {
        validate_connector_url(raw, self.policy.allow_loopback).await
    }

    pub async fn request(
        &self,
        method: Method,
        raw_url: &str,
        headers: HeaderMap,
        body: Option<Vec<u8>>,
    ) -> Result<RemoteResponse, RemoteError> {
        let target = self.validate_url(raw_url).await?;
        self.request_validated(method, &target, headers, body).await
    }

    pub async fn request_validated(
        &self,
        method: Method,
        target: &ValidatedRemoteTarget,
        headers: HeaderMap,
        body: Option<Vec<u8>>,
    ) -> Result<RemoteResponse, RemoteError> {
        let client = pinned_client(target, &self.policy)?;
        let mut req = client.request(method, target.url.as_str());
        req = req.headers(headers);
        if let Some(body) = body {
            req = req.body(body);
        }
        let response = req.send().await.map_err(map_reqwest_error)?;
        if response.status().is_redirection() {
            return Err(RemoteError::Redirect);
        }
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.bytes().await.map_err(map_reqwest_error)?;
        if bytes.len() > self.policy.max_body_bytes {
            return Err(RemoteError::TooLarge);
        }
        Ok(RemoteResponse {
            status,
            headers,
            body: bytes.to_vec(),
        })
    }
}

fn pinned_client(
    target: &ValidatedRemoteTarget,
    policy: &RemotePolicy,
) -> Result<Client, RemoteError> {
    let mut builder = Client::builder()
        .redirect(Policy::none())
        .timeout(policy.timeout)
        .connect_timeout(Duration::from_secs(5))
        .no_proxy()
        .https_only(!policy.allow_loopback);
    if !target.pinned_addrs.is_empty() {
        builder = builder.resolve_to_addrs(&target.host, &target.pinned_addrs);
    }
    builder
        .build()
        .map_err(|e| RemoteError::Network(e.to_string()))
}

fn map_reqwest_error(err: reqwest::Error) -> RemoteError {
    if err.is_timeout() {
        RemoteError::Timeout
    } else if err.is_redirect() {
        RemoteError::Redirect
    } else {
        RemoteError::Network(sanitize_network_error(&err.to_string()))
    }
}

fn sanitize_network_error(raw: &str) -> String {
    let mut out = raw.to_string();
    for needle in ["Bearer ", "token=", "Authorization:", "Cookie:"] {
        if let Some(idx) = out.find(needle) {
            out.replace_range(idx.., "[redacted]");
        }
    }
    if out.len() > 180 {
        out.truncate(180);
        out.push('…');
    }
    out
}

pub async fn validate_connector_url(
    raw: &str,
    allow_loopback: bool,
) -> Result<ValidatedRemoteTarget, RemoteError> {
    if raw.len() > 2048 {
        return Err(RemoteError::Blocked("url is too long".into()));
    }
    let url = Url::parse(raw).map_err(|_| RemoteError::Blocked("url is not valid".into()))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(RemoteError::Blocked(
            "url must not contain embedded credentials".into(),
        ));
    }
    let scheme = url.scheme();
    if allow_loopback {
        if scheme != "https" && scheme != "http" {
            return Err(RemoteError::Blocked("url must be http or https".into()));
        }
    } else if scheme != "https" {
        return Err(RemoteError::Blocked(
            "connected app endpoints must use https".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| RemoteError::Blocked("url is missing a host".into()))?
        .to_string();
    if blocked_hostname(&host) {
        return Err(RemoteError::Blocked("url targets a blocked host".into()));
    }

    let port = url.port_or_known_default().unwrap_or(443);
    let pinned_addrs = resolve_and_pin(&host, port, allow_loopback).await?;
    Ok(ValidatedRemoteTarget {
        url,
        host,
        pinned_addrs,
    })
}

async fn resolve_and_pin(
    host: &str,
    port: u16,
    allow_loopback: bool,
) -> Result<Vec<SocketAddr>, RemoteError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_blocked_ip(ip, allow_loopback) {
            return Err(RemoteError::Blocked(
                "url targets a private or metadata address".into(),
            ));
        }
        return Ok(vec![SocketAddr::new(ip, port)]);
    }

    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| RemoteError::Blocked("url host could not be resolved".into()))?;
    let mut pinned = Vec::new();
    for addr in addrs {
        if is_blocked_ip(addr.ip(), allow_loopback) {
            return Err(RemoteError::Blocked(
                "url resolves to a private or metadata address".into(),
            ));
        }
        pinned.push(addr);
    }
    if pinned.is_empty() {
        return Err(RemoteError::Blocked(
            "url host could not be resolved".into(),
        ));
    }
    Ok(pinned)
}

pub fn is_blocked_ip(ip: IpAddr, allow_loopback: bool) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4, allow_loopback),
        IpAddr::V6(v6) => is_blocked_ipv6(v6, allow_loopback),
    }
}

fn is_blocked_ipv4(v4: Ipv4Addr, allow_loopback: bool) -> bool {
    if v4.is_loopback() {
        return !allow_loopback;
    }
    v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_multicast()
        || v4.is_broadcast()
        || is_cgnat_ipv4(v4)
        || is_cloud_metadata_ipv4(v4)
}

fn is_blocked_ipv6(v6: Ipv6Addr, allow_loopback: bool) -> bool {
    if let Some(v4) = v6.to_ipv4_mapped() {
        return is_blocked_ipv4(v4, allow_loopback);
    }
    if v6.is_loopback() {
        return !allow_loopback;
    }
    v6.is_unspecified()
        || v6.is_multicast()
        || is_unique_local_ipv6(v6)
        || is_link_local_ipv6(v6)
        || is_cloud_metadata_ipv6(v6)
}

fn is_cgnat_ipv4(v4: Ipv4Addr) -> bool {
    let [a, b, _, _] = v4.octets();
    a == 100 && (64..=127).contains(&b)
}

fn is_cloud_metadata_ipv4(v4: Ipv4Addr) -> bool {
    v4.octets() == [169, 254, 169, 254] || v4.octets() == [100, 100, 100, 200]
}

fn is_unique_local_ipv6(v6: Ipv6Addr) -> bool {
    let o = v6.octets();
    o[0] == 0xfc || o[0] == 0xfd
}

fn is_link_local_ipv6(v6: Ipv6Addr) -> bool {
    let o = v6.octets();
    o[0] == 0xfe && (o[1] & 0xc0) == 0x80
}

fn is_cloud_metadata_ipv6(v6: Ipv6Addr) -> bool {
    // AWS IMDS IPv6: fd00:ec2::254
    v6 == Ipv6Addr::new(0xfd00, 0xec2, 0, 0, 0, 0, 0, 0x254)
}

fn blocked_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "metadata.google.internal"
        || host == "metadata.goog"
        || host.ends_with(".internal")
        || host == "169.254.169.254"
}

pub fn header_map_from_pairs(pairs: &[(&str, &str)]) -> Result<HeaderMap, RemoteError> {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| RemoteError::Network("invalid header name".into()))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|_| RemoteError::Network("invalid header value".into()))?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_ipv4() {
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), false));
        assert!(is_blocked_ip(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            false
        ));
        assert!(is_blocked_ip(
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),
            false
        ));
    }

    #[test]
    fn blocks_loopback_unless_test_policy() {
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::LOCALHOST), false));
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::LOCALHOST), true));
        assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::LOCALHOST), false));
        assert!(!is_blocked_ip(IpAddr::V6(Ipv6Addr::LOCALHOST), true));
    }

    #[test]
    fn blocks_private_ipv6_and_mapped_ipv4() {
        assert!(is_blocked_ip(
            IpAddr::V6(Ipv6Addr::new(0xfd12, 0, 0, 0, 0, 0, 0, 1)),
            false
        ));
        assert!(is_blocked_ip(
            IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)),
            false
        ));
        let mapped = Ipv6Addr::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 10, 0, 0, 1]);
        assert!(is_blocked_ip(IpAddr::V6(mapped), false));
    }

    #[test]
    fn blocks_cloud_metadata() {
        assert!(is_blocked_ip(
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            false
        ));
        assert!(is_blocked_ip(
            IpAddr::V6(Ipv6Addr::new(0xfd00, 0xec2, 0, 0, 0, 0, 0, 0x254)),
            false
        ));
        assert!(blocked_hostname("metadata.google.internal"));
    }

    #[tokio::test]
    async fn rejects_localhost_hostname() {
        let err = validate_connector_url("https://localhost/mcp", false)
            .await
            .unwrap_err();
        assert!(matches!(err, RemoteError::Blocked(_)));
    }

    #[tokio::test]
    async fn rejects_url_credentials() {
        let err = validate_connector_url("https://user:pass@example.com/mcp", false)
            .await
            .unwrap_err();
        assert!(matches!(err, RemoteError::Blocked(_)));
        assert!(err.message().contains("credentials"));
    }

    #[tokio::test]
    async fn rejects_http_in_production() {
        let err = validate_connector_url("http://example.com/mcp", false)
            .await
            .unwrap_err();
        assert!(matches!(err, RemoteError::Blocked(_)));
    }

    #[tokio::test]
    async fn rejects_literal_private_ipv4() {
        let err = validate_connector_url("https://10.0.0.8/mcp", false)
            .await
            .unwrap_err();
        assert!(matches!(err, RemoteError::Blocked(_)));
    }

    #[tokio::test]
    async fn rejects_literal_private_ipv6() {
        let err = validate_connector_url("https://[fd12::1]/mcp", false)
            .await
            .unwrap_err();
        assert!(matches!(err, RemoteError::Blocked(_)));
    }

    #[tokio::test]
    async fn test_policy_allows_loopback_http() {
        let target = validate_connector_url("http://127.0.0.1:9/", true)
            .await
            .expect("loopback allowed in tests");
        assert_eq!(target.host, "127.0.0.1");
        assert!(!target.pinned_addrs.is_empty());
    }

    #[tokio::test]
    async fn rejects_redirects() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(302)
                    .insert_header("Location", "https://evil.example/"),
            )
            .mount(&mock)
            .await;
        let client = RemoteHttpClient::new(RemotePolicy::for_tests());
        let err = client
            .request(
                Method::GET,
                &format!("{}/go", mock.uri()),
                HeaderMap::new(),
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            RemoteError::Redirect | RemoteError::Network(_)
        ));
    }
}
