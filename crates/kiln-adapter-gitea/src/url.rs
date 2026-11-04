//! Reviewed HTTPS origin admission shared by the Gitea status and job shells.

use crate::domain;

/// One admitted absolute HTTPS origin without user information, query or fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HttpsOrigin {
    /// Normalized origin, including an optional reviewed subpath, without a trailing slash.
    pub(crate) value: String,
    /// DNS or IP authority in `host:port` form.
    pub(crate) destination: String,
    pub(crate) port: u16,
}

pub(crate) fn admit_https_origin(
    value: &str,
    require_api_v1: bool,
) -> kiln_provider_api::PortResult<HttpsOrigin> {
    let invalid = || {
        domain(
            "gitea_origin",
            "origin",
            "Gitea origin must be an absolute HTTPS origin without user information, query or fragment",
        )
    };
    if value.is_empty()
        || value.len() > 2_048
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || value.contains('@')
        || value.contains('?')
        || value.contains('#')
        || value.contains('\\')
    {
        return Err(invalid());
    }
    let rest = value.strip_prefix("https://").ok_or_else(invalid)?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => (rest, ""),
    };
    if authority.is_empty() {
        return Err(invalid());
    }
    let (host, port) = split_authority(authority).ok_or_else(invalid)?;
    if !valid_host(&host) {
        return Err(invalid());
    }
    let path = path.trim_end_matches('/');
    if !path.is_empty()
        && path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid());
    }
    let normalized_path = if path.is_empty() {
        String::new()
    } else {
        format!("/{path}")
    };
    if require_api_v1 && !normalized_path.ends_with("/api/v1") {
        return Err(domain(
            "gitea_api_base",
            "status.api_base",
            "Gitea API base must end with /api/v1",
        ));
    }
    let destination = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let normalized_authority = if host.contains(':') {
        if port == 443 {
            format!("[{host}]")
        } else {
            format!("[{host}]:{port}")
        }
    } else if port == 443 {
        host
    } else {
        format!("{host}:{port}")
    };
    Ok(HttpsOrigin {
        value: format!("https://{normalized_authority}{normalized_path}"),
        destination,
        port,
    })
}

fn split_authority(authority: &str) -> Option<(String, u16)> {
    if authority.starts_with('[') {
        let end = authority.find(']')?;
        let host = authority.get(1..end)?.to_ascii_lowercase();
        let remainder = authority.get(end + 1..)?;
        let port = if remainder.is_empty() {
            443
        } else {
            remainder.strip_prefix(':')?.parse::<u16>().ok()?
        };
        if port == 0 {
            return None;
        }
        return Some((host, port));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() => (host, port.parse::<u16>().ok()?),
        Some(_) => return None,
        None => (authority, 443),
    };
    if port == 0 || host.is_empty() {
        return None;
    }
    Some((host.to_ascii_lowercase(), port))
}

fn valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.contains(':') {
        // Bracketed IPv6 literal. The standard parser is authoritative.
        return host.parse::<std::net::Ipv6Addr>().is_ok();
    }
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}
