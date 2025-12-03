use anyhow::Result;
use config::{Config as ConfigLoader, File};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxySettings {
    pub enable_http_proxy: bool,
    pub enable_https_proxy: bool,
    pub enable_ftp_proxy: bool,
    pub enable_no_proxy: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enable_http_proxy: true,
            enable_https_proxy: true,
            enable_ftp_proxy: true,
            enable_no_proxy: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub default_hosts_file: Option<String>,
    pub no_proxy: Option<Vec<String>>,
    pub enable_wpad_discovery: Option<bool>,
    pub wpad_url: Option<String>,
    pub proxy_settings: ProxySettings,
}

use crate::defaults;

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_hosts_file: Some("hosts".to_string()),
            no_proxy: None,
            enable_wpad_discovery: Some(true),
            wpad_url: Some(defaults::default_wpad_url()),
            proxy_settings: ProxySettings::default(),
        }
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
    let app_config_dir = config_dir.join("proxyctl-rs");
    fs::create_dir_all(&app_config_dir)?;
    Ok(app_config_dir)
}

pub fn load_config() -> Result<AppConfig> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join("config.toml");

    let loader = ConfigLoader::builder()
        .add_source(File::from(config_file).required(false))
        .build()?;

    let config: AppConfig = loader.try_deserialize()?;
    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join("config.toml");

    let toml = toml::to_string(config)?;
    fs::write(config_file, toml)?;
    Ok(())
}

pub fn get_hosts_file_path() -> Result<PathBuf> {
    let config = load_config()?;
    let config_dir = get_config_dir()?;
    let hosts_file = config
        .default_hosts_file
        .unwrap_or_else(|| "hosts.txt".to_string());
    Ok(config_dir.join(hosts_file))
}

pub fn get_custom_no_proxy() -> Result<Option<Vec<String>>> {
    let config = load_config()?;
    Ok(config.no_proxy)
}

pub fn get_proxy_settings() -> Result<ProxySettings> {
    match load_config() {
        Ok(config) => Ok(config.proxy_settings),
        Err(_) => Ok(ProxySettings::default()),
    }
}

pub fn get_wpad_config() -> Result<(bool, String)> {
    let config = load_config()?;
    let enabled = config.enable_wpad_discovery.unwrap_or(true);
    let url = config.wpad_url.unwrap_or_else(defaults::default_wpad_url);
    Ok((enabled, url))
}

pub fn initialize_config() -> Result<()> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join("config.toml");

    // Create default config if it doesn't exist
    if !config_file.exists() {
        let default_config = AppConfig::default();
        save_config(&default_config)?;
    }

    // Create default hosts file if it doesn't exist
    let hosts_path = get_hosts_file_path()?;
    if !hosts_path.exists() {
        // Try to copy from default_hosts.example.txt in current dir
        let example_file = std::env::current_dir()?.join("default_hosts.example.txt");
        if example_file.exists() {
            fs::copy(&example_file, &hosts_path)?;
        } else {
            // Create empty file
            fs::write(&hosts_path, "# Add proxy hosts here, one per line\n")?;
        }
    }

    Ok(())
}

pub fn add_ssh_hosts(hosts_file: &str) -> Result<()> {
    let ssh_config_path = get_ssh_config_path()?;
    let hosts_content = fs::read_to_string(hosts_file)?;

    // Read existing config
    let mut config = if ssh_config_path.exists() {
        fs::read_to_string(&ssh_config_path)?
    } else {
        String::new()
    };

    // Add proxy settings for each host
    for line in hosts_content.lines() {
        let host = line.trim();
        if !host.is_empty() && !host.starts_with('#') {
            let proxy_config = format!(
                "\nHost {}\n    ProxyCommand nc -x {} %h %p\n",
                host,
                get_proxy_host()
            );
            if !config.contains(&format!("Host {}", host)) {
                config.push_str(&proxy_config);
            }
        }
    }

    fs::write(&ssh_config_path, config)?;
    Ok(())
}

pub fn remove_ssh_hosts() -> Result<()> {
    let ssh_config_path = get_ssh_config_path()?;
    if !ssh_config_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&ssh_config_path)?;
    let mut in_proxy_block = false;
    let mut lines_to_keep = Vec::new();

    for line in content.lines() {
        if line.starts_with("Host ") && line.contains("ProxyCommand") {
            in_proxy_block = true;
            continue;
        }
        if in_proxy_block && line.trim().is_empty() {
            in_proxy_block = false;
            continue;
        }
        if !in_proxy_block {
            lines_to_keep.push(line.to_string());
        }
    }

    fs::write(&ssh_config_path, lines_to_keep.join("\n"))?;
    Ok(())
}

fn get_ssh_config_path() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".ssh").join("config"))
}

fn get_proxy_host() -> &'static str {
    // This should be configurable, but for now use a default
    "proxy.example.com:8080"
}
