use proxyctl_rs::{config, defaults, proxy};

#[test]
fn test_proxy_status() {
    // Test that status returns expected format
    let status = proxy::get_status().unwrap();
    assert!(status.contains("HTTP Proxy:"));
    assert!(status.contains("HTTPS Proxy:"));
    assert!(status.contains("No Proxy:"));
}

#[test]
fn test_default_constants() {
    // Test that default constants are properly defined
    assert!(!defaults::default_no_proxy().is_empty());
    assert!(defaults::default_no_proxy().contains("localhost"));
    assert!(!defaults::default_wpad_url().is_empty());
    assert!(defaults::default_wpad_url().contains("wpad"));
}

#[test]
fn test_config_struct_defaults() {
    // Test the config struct defaults directly
    let config = config::AppConfig::default();
    assert_eq!(config.default_hosts_file, Some("hosts".to_string()));
    assert!(config.no_proxy.is_none());
    assert_eq!(config.enable_wpad_discovery, Some(true));
    assert_eq!(
        config.wpad_url,
        Some("http://wpad.local/wpad.dat".to_string())
    );
    assert!(config.proxy_settings.enable_http_proxy);
    assert!(config.proxy_settings.enable_https_proxy);
    assert!(config.proxy_settings.enable_no_proxy);
}

#[test]
fn test_proxy_settings_struct() {
    // Test proxy settings struct
    let settings = config::ProxySettings::default();
    assert!(settings.enable_http_proxy);
    assert!(settings.enable_https_proxy);
    assert!(settings.enable_ftp_proxy);
    assert!(settings.enable_no_proxy);
}

#[test]
fn test_combined_no_proxy_logic() {
    // Test that default no_proxy combines with overrides correctly
    let default_no_proxy = defaults::default_no_proxy();
    let overrides = vec!["test.com".to_string(), "example.org".to_string()];
    let mut combined = vec![default_no_proxy.to_string()];
    combined.extend(overrides.clone());
    let result = combined.join(",");

    assert!(result.contains("localhost"));
    assert!(result.contains("127.0.0.1"));
    assert!(result.contains("test.com"));
    assert!(result.contains("example.org"));
}

#[test]
fn test_config_serialization() {
    // Test that config can be serialized and deserialized
    let mut config = config::AppConfig::default();
    config.no_proxy = Some(vec!["custom.domain".to_string()]);
    config.enable_wpad_discovery = Some(false);
    config.wpad_url = Some("http://custom-wpad.example.com/wpad.dat".to_string());
    config.proxy_settings.enable_ftp_proxy = false;

    let toml = toml::to_string(&config).unwrap();
    assert!(toml.contains("custom.domain"));
    assert!(toml.contains("enable_wpad_discovery = false"));
    assert!(toml.contains("http://custom-wpad.example.com/wpad.dat"));
    assert!(toml.contains("enable_ftp_proxy = false"));
}

#[test]
fn test_no_proxy_parses_comma_string() {
    let config: config::AppConfig = toml::from_str(
        r#"
default_hosts_file = "hosts"
no_proxy = "example.com,foo.bar"

[proxy_settings]
enable_http_proxy = true
enable_https_proxy = true
enable_ftp_proxy = true
enable_no_proxy = true
"#,
    )
    .unwrap();

    assert_eq!(
        config.no_proxy,
        Some(vec!["example.com".to_string(), "foo.bar".to_string()])
    );
}

#[tokio::test]
async fn test_detect_proxy_placeholder() {
    // Placeholder for proxy detection test
    // In a real implementation, this would test the detect module
    // with mocked HTTP responses
}
