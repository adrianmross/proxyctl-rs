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
fn test_missing_proxy_settings_defaults() {
    let config: config::AppConfig = toml::from_str(
        r#"
default_hosts_file = "hosts"

[proxy_settings]
"#,
    )
    .unwrap();

    assert!(config.proxy_settings.enable_http_proxy);
    assert!(config.proxy_settings.enable_https_proxy);
    assert!(config.proxy_settings.enable_ftp_proxy);
    assert!(config.proxy_settings.enable_no_proxy);
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

#[test]
fn test_wpad_url_override_from_config() {
    use std::env;
    use std::fs;

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn set<S: AsRef<str>>(key: &'static str, value: S) -> Self {
            let original = env::var(key).ok();
            env::set_var(key, value.as_ref());
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref value) = self.original {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    let temp_dir = tempfile::tempdir().unwrap();
    let fake_home = temp_dir.path().join("home");
    fs::create_dir_all(&fake_home).unwrap();

    let mut potential_config_dirs = vec![fake_home.join(".config")];
    potential_config_dirs.push(fake_home.join("Library").join("Application Support"));
    for dir in &potential_config_dirs {
        let _ = fs::create_dir_all(dir);
    }

    let _home_guard = EnvGuard::set("HOME", fake_home.to_string_lossy());
    let _xdg_guard = EnvGuard::set("XDG_CONFIG_HOME", fake_home.join(".config").to_string_lossy());
    let _default_guard = EnvGuard::set("DEFAULT_WPAD_URL", "http://default.local/wpad.dat");

    let config_dir = config::get_config_dir().unwrap();
    fs::write(
        config_dir.join("config.toml"),
        r#"wpad_url = "http://override.example.com/wpad.dat"

[proxy_settings]
enable_http_proxy = true
enable_https_proxy = true
enable_ftp_proxy = true
enable_no_proxy = true
"#,
    )
    .unwrap();

    let (_, url) = config::get_wpad_config().unwrap();
    assert_eq!(url, "http://override.example.com/wpad.dat");
}

#[tokio::test]
async fn test_detect_proxy_placeholder() {
    // Placeholder for proxy detection test
    // In a real implementation, this would test the detect module
    // with mocked HTTP responses
}
