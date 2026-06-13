use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, Default)]
pub struct ProxyValue {
    pub is_specified: bool,
    pub protocol: String,
    pub login: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub is_http: bool,
    pub is_socks5: bool,
    pub is_auth: bool,
}

static PROXY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?:([a-z\d]+)://)?(?:(.+):(.+)@)?([^:]+):(\d+)$").expect("valid proxy regex")
});

impl ProxyValue {
    pub fn parse_from_string(url: &str) -> Self {
        let mut value = Self::default();
        let url = url.trim();
        if url.is_empty() {
            return value;
        }

        let Some(caps) = PROXY_RE.captures(url) else {
            return value;
        };

        value.is_specified = true;

        let mut protocol = caps
            .get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        if protocol.is_empty() || protocol == "https" {
            protocol = "http".to_string();
        }

        value.protocol = protocol.clone();
        value.login = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        value.password = caps
            .get(3)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        value.host = caps.get(4).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        value.port = caps
            .get(5)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);

        value.is_http = protocol == "http";
        value.is_socks5 = protocol == "socks5";
        value.is_auth = !value.login.is_empty() && !value.password.is_empty();

        value
    }

    pub fn format_to_http_env(&self) -> String {
        if !self.is_specified {
            return String::new();
        }

        let mut result = String::from("http://");
        if self.is_auth {
            result.push_str(&self.login);
            result.push(':');
            result.push_str(&self.password);
            result.push('@');
        }
        result.push_str(&self.host);
        result.push(':');
        result.push_str(&self.port.to_string());
        result
    }

    pub fn format_to_chrome_proxy(&self) -> String {
        if !self.is_specified {
            return String::new();
        }
        format!("{}://{}:{}", self.protocol, self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::ProxyValue;

    #[test]
    fn parses_http_proxy() {
        let proxy = ProxyValue::parse_from_string("http://127.0.0.1:8080");
        assert!(proxy.is_specified);
        assert!(proxy.is_http);
        assert_eq!(proxy.host, "127.0.0.1");
        assert_eq!(proxy.port, 8080);
    }

    #[test]
    fn parses_socks5_with_auth() {
        let proxy = ProxyValue::parse_from_string("socks5://user:pass@127.0.0.1:1080");
        assert!(proxy.is_specified);
        assert!(proxy.is_socks5);
        assert!(proxy.is_auth);
    }
}
