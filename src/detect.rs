use crate::config;
use anyhow::Result;
use regex::Regex;
use reqwest::Client;

pub async fn detect_best_proxy() -> Result<String> {
    let (enabled, url) = config::get_wpad_config()?;

    if !enabled {
        return Err(anyhow::anyhow!(
            "WPAD proxy discovery is disabled in configuration"
        ));
    }

    let client = Client::new();

    // Fetch WPAD file
    let response = client
        .get(&url)
        .header("noproxy", "*")
        .send()
        .await?
        .text()
        .await?;

    // Parse proxies from response
    let re = Regex::new(r#"proxies\s*=\s*"([^"]+)""#)?;
    if let Some(caps) = re.captures(&response) {
        let proxies_str = &caps[1];
        // For now, return the first proxy
        // In a real implementation, you might want to test connectivity
        let proxies: Vec<&str> = proxies_str.split(';').collect();
        if let Some(first_proxy) = proxies.first() {
            Ok(first_proxy.trim().to_string())
        } else {
            Err(anyhow::anyhow!("No proxies found in WPAD response"))
        }
    } else {
        Err(anyhow::anyhow!(
            "Could not parse proxies from WPAD response"
        ))
    }
}
