use proxyctl_rs::{config, defaults, proxy};
use tempfile::TempDir;

struct EnvGuard {
    originals: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn set<I, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (&'static str, V)>,
        V: Into<String>,
    {
        let originals = vars
            .into_iter()
            .map(|(key, value)| {
                let original = std::env::var(key).ok();
                std::env::set_var(key, value.into());
                (key, original)
            })
            .collect();
        Self { originals }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, original) in self.originals.drain(..) {
            if let Some(value) = original {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

struct ConfigDirGuard {
    _dir: TempDir,
    _env_guard: EnvGuard,
}

impl ConfigDirGuard {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp config dir");
        let config_dir = dir.path().join("config");
        let data_dir = dir.path().join("data");
        let home_dir = dir.path().join("home");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&home_dir).expect("home dir");

        let env_guard = EnvGuard::set([
            ("XDG_CONFIG_HOME", config_dir.to_string_lossy().into_owned()),
            ("XDG_DATA_HOME", data_dir.to_string_lossy().into_owned()),
            ("HOME", home_dir.to_string_lossy().into_owned()),
            ("SHELL", "/bin/false".to_string()),
        ]);
        Self {
            _dir: dir,
            _env_guard: env_guard,
        }
    }
}

#[tokio::test]
async fn test_proxy_status() {
    let _config_guard = ConfigDirGuard::new();
    // Test that status returns expected format
    let status = proxy::get_status().await.unwrap();
    assert!(status.contains("HTTP Proxy:"));
    assert!(status.contains("HTTPS Proxy:"));
    assert!(status.contains("No Proxy:"));
}

#[tokio::test]
async fn test_status_reflects_disable_without_vars() {
    let _config_guard = ConfigDirGuard::new();
    let _guard = EnvGuard::set([
        ("http_proxy", "http://proxy.example.com:8080"),
        ("https_proxy", "http://proxy.example.com:8080"),
        ("no_proxy", "localhost"),
    ]);

    proxy::disable_proxy().await.unwrap();
    let status = proxy::get_status().await.unwrap();

    assert!(status.contains("HTTP Proxy: Not set"));
    assert!(status.contains("HTTPS Proxy: Not set"));
    assert!(status.contains("No Proxy: Not set"));
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
    let config = config::AppConfig {
        no_proxy: Some(vec!["custom.domain".to_string()]),
        enable_wpad_discovery: Some(false),
        wpad_url: Some("http://custom-wpad.example.com/wpad.dat".to_string()),
        proxy_settings: config::ProxySettings {
            enable_ftp_proxy: false,
            ..config::ProxySettings::default()
        },
        ..config::AppConfig::default()
    };

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
    let _xdg_guard = EnvGuard::set(
        "XDG_CONFIG_HOME",
        fake_home.join(".config").to_string_lossy(),
    );
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
