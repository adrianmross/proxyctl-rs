#[test]
fn describe_config_options_includes_defaults() {
    let config = proxyctl_rs::config::AppConfig::default();
    assert_eq!(config.default_hosts_file, Some("hosts".to_string()));
    assert!(config.proxy_settings.enable_http_proxy);
}
