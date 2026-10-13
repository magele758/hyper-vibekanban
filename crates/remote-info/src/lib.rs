use std::sync::OnceLock;

/// Runtime information about configured remote endpoints.
#[derive(Clone)]
pub struct RemoteInfo {
    api_base: OnceLock<String>,
    relay_api_base: OnceLock<String>,
}

impl Default for RemoteInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteInfo {
    pub fn new() -> Self {
        Self {
            api_base: OnceLock::new(),
            relay_api_base: OnceLock::new(),
        }
    }

    pub fn set_api_base(&self, api_base: String) -> Result<(), String> {
        self.api_base
            .set(api_base)
            .map_err(|_| "api_base already set".to_string())
    }

    pub fn get_api_base(&self) -> Option<String> {
        self.api_base.get().cloned()
    }

    pub fn set_relay_api_base(&self, relay_api_base: String) -> Result<(), String> {
        self.relay_api_base
            .set(relay_api_base)
            .map_err(|_| "relay_api_base already set".to_string())
    }

    pub fn get_relay_api_base(&self) -> Option<String> {
        self.relay_api_base.get().cloned()
    }
}

/// `join` is a long-running server mode: log in, register with Relay, do not
/// open a browser.
pub fn is_join_mode() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "join" || arg == "--join")
        || matches!(
            std::env::var("VK_JOIN").ok().as_deref(),
            Some("1") | Some("true")
        )
}

/// Relay origin the execution machine should dial.
///
/// An explicit value wins. In join mode, a missing value is derived from the
/// data-service API base. `https://*.ts.net` is forced onto port 18443 so the
/// TLS front door is used instead of plaintext 18082.
pub fn resolve_relay_api_base(
    explicit: Option<&str>,
    api_base: Option<&str>,
    join_mode: bool,
) -> Option<String> {
    let explicit = explicit.map(str::trim).filter(|value| !value.is_empty());
    let raw = if let Some(value) = explicit {
        value.to_string()
    } else if join_mode {
        derive_relay_api_base(api_base?)
    } else {
        return None;
    };
    let normalized = normalize_relay_api_base(&raw);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

/// `https://*.ts.net` with no port, :443, or :18082 becomes :18443.
pub fn normalize_relay_api_base(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/').to_string();
    let Some((https, rest)) = split_scheme(&trimmed) else {
        return trimmed;
    };
    if !https {
        return trimmed;
    }
    let (authority, path) = split_authority(rest);
    let hostport = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let (host, port) = split_host_port(hostport);
    if !is_ts_net_host(host) {
        return trimmed;
    }
    if port.is_none() || port == Some("443") || port == Some("18082") {
        return format!("https://{host}:18443{path}");
    }
    trimmed
}

pub fn derive_relay_api_base(api_base: &str) -> String {
    let trimmed = api_base.trim().trim_end_matches('/');
    let Some((https, rest)) = split_scheme(trimmed) else {
        return trimmed.to_string();
    };
    let (authority, _) = split_authority(rest);
    let hostport = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let (host, _) = split_host_port(hostport);
    if host.is_empty() {
        return trimmed.to_string();
    }
    let scheme = if https { "https" } else { "http" };
    let port = if https && is_ts_net_host(host) {
        "18443"
    } else {
        "18082"
    };
    normalize_relay_api_base(&format!("{scheme}://{host}:{port}"))
}

fn split_scheme(value: &str) -> Option<(bool, &str)> {
    if let Some(rest) = value.strip_prefix("https://") {
        Some((true, rest))
    } else if let Some(rest) = value.strip_prefix("http://") {
        Some((false, rest))
    } else {
        None
    }
}

fn split_authority(rest: &str) -> (&str, &str) {
    match rest.find(['/', '?', '#']) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    }
}

fn split_host_port(hostport: &str) -> (&str, Option<&str>) {
    if let Some((host, port)) = hostport.rsplit_once(':')
        && !host.is_empty()
        && !host.contains(']')
        && port.chars().all(|ch| ch.is_ascii_digit())
    {
        return (host, Some(port));
    }
    (hostport, None)
}

fn is_ts_net_host(host: &str) -> bool {
    host.trim_end_matches('.')
        .to_ascii_lowercase()
        .ends_with(".ts.net")
}

#[cfg(test)]
mod tests {
    use super::{
        RemoteInfo, derive_relay_api_base, normalize_relay_api_base, resolve_relay_api_base,
    };

    const TS_HOST: &str = "shinemo.taile7bb25.ts.net";

    #[test]
    fn stores_remote_endpoints() {
        let remote_info = RemoteInfo::new();

        assert_eq!(remote_info.get_api_base(), None);
        assert_eq!(remote_info.get_relay_api_base(), None);

        remote_info
            .set_api_base("https://api.example.com".to_string())
            .unwrap();
        remote_info
            .set_relay_api_base("https://relay.example.com".to_string())
            .unwrap();

        assert_eq!(
            remote_info.get_api_base().as_deref(),
            Some("https://api.example.com")
        );
        assert_eq!(
            remote_info.get_relay_api_base().as_deref(),
            Some("https://relay.example.com")
        );
    }

    #[test]
    fn https_ts_net_18082_uses_relay_tls_door() {
        assert_eq!(
            normalize_relay_api_base(&format!("https://{TS_HOST}:18082")),
            format!("https://{TS_HOST}:18443")
        );
        assert_eq!(
            normalize_relay_api_base(&format!("https://{TS_HOST}")),
            format!("https://{TS_HOST}:18443")
        );
        assert_eq!(
            normalize_relay_api_base(&format!("https://{TS_HOST}:443/")),
            format!("https://{TS_HOST}:18443")
        );
    }

    #[test]
    fn https_ts_net_18443_and_custom_ports_stay() {
        assert_eq!(
            normalize_relay_api_base(&format!("https://{TS_HOST}:18443/")),
            format!("https://{TS_HOST}:18443")
        );
        assert_eq!(
            normalize_relay_api_base(&format!("https://{TS_HOST}:18444")),
            format!("https://{TS_HOST}:18444")
        );
    }

    #[test]
    fn plaintext_and_non_tailscale_hosts_keep_their_port() {
        assert_eq!(
            normalize_relay_api_base(&format!("http://{TS_HOST}:18082")),
            format!("http://{TS_HOST}:18082")
        );
        assert_eq!(
            normalize_relay_api_base("https://relay.example.com:18082"),
            "https://relay.example.com:18082"
        );
    }

    #[test]
    fn derive_relay_base_from_data_service() {
        assert_eq!(
            derive_relay_api_base(&format!("https://{TS_HOST}:13444")),
            format!("https://{TS_HOST}:18443")
        );
        assert_eq!(
            derive_relay_api_base("http://127.0.0.1:13000"),
            "http://127.0.0.1:18082"
        );
    }

    #[test]
    fn join_mode_derives_relay_when_unset_and_rewrites_18082() {
        assert_eq!(
            resolve_relay_api_base(None, Some(&format!("https://{TS_HOST}:13444")), true),
            Some(format!("https://{TS_HOST}:18443"))
        );
        assert_eq!(
            resolve_relay_api_base(Some(&format!("https://{TS_HOST}:18082")), None, false),
            Some(format!("https://{TS_HOST}:18443"))
        );
        assert_eq!(
            resolve_relay_api_base(None, Some("http://127.0.0.1:13000"), false),
            None
        );
    }
}
