use crate::config;
use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::Client;

// PAC entries typically follow the pattern "PROXY host:port" or variations
// such as "HTTPS host:port". We capture the target component while skipping
// trailing directives like DIRECT. Case-insensitive to support mixed casing.
const PROXY_TARGET_REGEX: &str = r#"(?i)\b(?:PROXY|HTTPS?|SOCKS[45]?)\s+([^;\s"]+)"#;

pub async fn detect_best_proxy() -> Result<String> {
    let (enabled, url) = config::get_wpad_config()?;

    if !enabled {
        return Err(anyhow!("WPAD proxy discovery is disabled in configuration"));
    }

    let client = Client::new();

    let response = client
        .get(&url)
        .header("noproxy", "*")
        .send()
        .await?
        .text()
        .await?;

    detect_proxy_candidates_from_response(&response)
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Could not parse proxies from WPAD response"))
}

pub async fn detect_proxy_candidates() -> Result<Vec<String>> {
    let (enabled, url) = config::get_wpad_config()?;

    if !enabled {
        return Err(anyhow!("WPAD proxy discovery is disabled in configuration"));
    }

    let client = Client::new();
    let response = client
        .get(&url)
        .header("noproxy", "*")
        .send()
        .await?
        .text()
        .await?;

    let proxies = detect_proxy_candidates_from_response(&response);

    if proxies.is_empty() {
        Err(anyhow!("Could not parse proxies from WPAD response"))
    } else {
        Ok(proxies)
    }
}

fn detect_proxy_candidates_from_response(response: &str) -> Vec<String> {
    let re = Regex::new(PROXY_TARGET_REGEX).expect("invalid proxy token regex");
    re.captures_iter(response)
        .filter_map(|caps| caps.get(1))
        .map(|target| target.as_str().trim().trim_matches(';').trim_matches('"'))
        .map(|target| target.trim_end_matches('/').to_string())
        .collect()
}

#[cfg(test)]
mod detect_tests {
    use super::detect_proxy_candidates_from_response;

    #[test]
    fn parses_proxies_from_variable_assignment() {
        let body = r#"
            var proxies = "PROXY proxy-us.example.com:8080; PROXY proxy-backup.example.com:8080; DIRECT";
            return proxies;
        "#;

        let proxies = detect_proxy_candidates_from_response(body);
        assert_eq!(proxies.len(), 2);
        assert_eq!(proxies[0], "proxy-us.example.com:8080");
        assert_eq!(proxies[1], "proxy-backup.example.com:8080");
    }

    #[test]
    fn parses_proxies_from_return_statement() {
        let body = r#"
            function FindProxyForURL(url, host) {
                return "PROXY proxy-eu.example.net:3128; DIRECT";
            }
        "#;

        let proxies = detect_proxy_candidates_from_response(body);
        assert_eq!(proxies, vec!["proxy-eu.example.net:3128".to_string()]);
    }

    #[test]
    fn ignores_direct_entries() {
        let body = r#"
            return "DIRECT";
        "#;

        let proxies = detect_proxy_candidates_from_response(body);
        assert!(proxies.is_empty());
    }
}
