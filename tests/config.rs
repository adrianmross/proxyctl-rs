use proxyctl_rs::config;

#[test]
fn describe_config_options_includes_defaults() {
    let options = config::describe_config_options().unwrap();
    let keys: Vec<&str> = options.iter().map(|option| option.key).collect();
    assert!(keys.contains(&"default_hosts_file"));
    assert!(keys.contains(&"proxy_settings.enable_http_proxy"));
}
