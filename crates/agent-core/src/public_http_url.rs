use std::net::IpAddr;

use crate::ToolError;

/// Returns true when the address must not be reached from the browser sandbox.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
                || is_cgnat_ipv4(v4)
                || v4.octets() == [169, 254, 169, 254]
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || is_unique_local_ipv6(v6)
                || is_link_local_ipv6(v6)
        }
    }
}

fn is_cgnat_ipv4(v4: std::net::Ipv4Addr) -> bool {
    let [a, b, _, _] = v4.octets();
    a == 100 && (64..=127).contains(&b)
}

fn is_unique_local_ipv6(v6: std::net::Ipv6Addr) -> bool {
    let o = v6.octets();
    o[0] == 0xfc || o[0] == 0xfd
}

fn is_link_local_ipv6(v6: std::net::Ipv6Addr) -> bool {
    let o = v6.octets();
    o[0] == 0xfe && (o[1] & 0xc0) == 0x80
}

fn blocked_hostname(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "169.254.169.254"
        || host == "metadata.google.internal"
        || host == "metadata.goog"
}

/// Validate scheme/host and resolve DNS so hostnames cannot bypass IP literal checks.
pub async fn validate_public_http_url(raw_url: &str) -> Result<(), ToolError> {
    if raw_url.len() > crate::approval::MAX_BROWSER_URL_CHARS {
        return Err(ToolError::MalformedArguments("url is too long".into()));
    }
    let parsed = url::Url::parse(raw_url).map_err(|_| {
        ToolError::MalformedArguments("url is not a valid http(s) URL".into())
    })?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(ToolError::MalformedArguments(
            "url must be http or https".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| ToolError::MalformedArguments("url is missing a host".into()))?;
    if blocked_hostname(host) {
        return Err(ToolError::MalformedArguments(
            "url targets a blocked host".into(),
        ));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_blocked_ip(ip) {
            return Err(ToolError::MalformedArguments(
                "url targets a private or link-local address".into(),
            ));
        }
        return Ok(());
    }

    let port = parsed.port_or_known_default().unwrap_or(443);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| {
            ToolError::MalformedArguments("url host could not be resolved".into())
        })?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(addr.ip()) {
            return Err(ToolError::MalformedArguments(
                "url resolves to a private or link-local address".into(),
            ));
        }
    }
    if !any {
        return Err(ToolError::MalformedArguments(
            "url host could not be resolved".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn cgnat_ipv4_is_blocked() {
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));
    }

    #[test]
    fn metadata_ipv4_is_blocked() {
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
    }

    #[test]
    fn unique_local_ipv6_is_blocked() {
        assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::new(0xfd12, 0, 0, 0, 0, 0, 0, 1))));
    }

    #[tokio::test]
    async fn rejects_literal_loopback() {
        let err = validate_public_http_url("http://127.0.0.1/")
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::MalformedArguments(_)));
    }
}
