use anyhow::{anyhow, Result};
use config::{Config as ConfigLoader, File};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ProxySettings {
    pub enable_http_proxy: bool,
    pub enable_https_proxy: bool,
    pub enable_ftp_proxy: bool,
    pub enable_all_proxy: bool,
    pub enable_proxy_rsync: bool,
    pub enable_no_proxy: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enable_http_proxy: true,
            enable_https_proxy: true,
            enable_ftp_proxy: true,
            enable_all_proxy: true,
            enable_proxy_rsync: true,
            enable_no_proxy: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub default_hosts_file: Option<String>,
    #[serde(default, deserialize_with = "deserialize_no_proxy")]
    pub no_proxy: Option<Vec<String>>,
    pub default_proxy: Option<String>,
    pub enable_wpad_discovery: Option<bool>,
    pub wpad_url: Option<String>,
    #[serde(default)]
    pub proxy_settings: ProxySettings,
}

#[derive(Debug, Clone)]
pub struct ConfigOptionDescriptor {
    pub key: &'static str,
    pub value_type: &'static str,
    pub description: &'static str,
    pub default: String,
    pub current: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NoProxyInput {
    List(Vec<String>),
    String(String),
}

fn deserialize_no_proxy<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<NoProxyInput>::deserialize(deserializer)?;
    Ok(value.map(|input| match input {
        NoProxyInput::List(items) => items,
        NoProxyInput::String(item) => item
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    }))
}

use crate::defaults;

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_hosts_file: Some("hosts".to_string()),
            no_proxy: None,
            default_proxy: None,
            enable_wpad_discovery: Some(true),
            wpad_url: Some(defaults::default_wpad_url()),
            proxy_settings: ProxySettings::default(),
        }
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    if let Some(xdg_config) = env::var_os("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg_config).join("proxyctl-rs");
        fs::create_dir_all(&path)?;
        return Ok(path);
    }

    if let Some(home_dir) = dirs::home_dir() {
        let path = home_dir.join(".config").join("proxyctl-rs");
        fs::create_dir_all(&path)?;
        return Ok(path);
    }

    if let Some(config_dir) = dirs::config_dir() {
        let path = config_dir.join("proxyctl-rs");
        fs::create_dir_all(&path)?;
        return Ok(path);
    }

    Err(anyhow!("Could not find config directory"))
}

pub fn get_data_dir() -> Result<PathBuf> {
    if let Some(xdg_data) = env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(xdg_data).join("proxyctl-rs");
        fs::create_dir_all(&path)?;
        return Ok(path);
    }

    if let Some(data_dir) = dirs::data_dir() {
        let path = data_dir.join("proxyctl-rs");
        fs::create_dir_all(&path)?;
        return Ok(path);
    }

    if let Some(home_dir) = dirs::home_dir() {
        let path = home_dir.join(".local").join("share").join("proxyctl-rs");
        fs::create_dir_all(&path)?;
        return Ok(path);
    }

    Err(anyhow!("Could not find data directory"))
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

pub fn get_default_proxy() -> Result<Option<String>> {
    let config = load_config()?;
    Ok(config.default_proxy.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }))
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

pub fn describe_config_options() -> Result<Vec<ConfigOptionDescriptor>> {
    let default_config = AppConfig::default();
    let current_config = load_config()?;

    let mut options = Vec::new();

    options.push(ConfigOptionDescriptor {
        key: "default_hosts_file",
        value_type: "string",
        description: "File name used for proxy host entries within the config directory",
        default: clone_or_none(default_config.default_hosts_file.as_ref()),
        current: clone_or_none(current_config.default_hosts_file.as_ref()),
    });

    options.push(ConfigOptionDescriptor {
        key: "no_proxy",
        value_type: "list<string>",
        description: "Additional hosts appended to the NO_PROXY environment variable",
        default: join_list(default_config.no_proxy.as_ref()),
        current: join_list(current_config.no_proxy.as_ref()),
    });

    options.push(ConfigOptionDescriptor {
        key: "default_proxy",
        value_type: "string",
        description: "Fallback proxy URL used when detection is disabled or unavailable",
        default: clone_or_none(default_config.default_proxy.as_ref()),
        current: clone_or_none(current_config.default_proxy.as_ref()),
    });

    let default_wpad = default_config
        .enable_wpad_discovery
        .unwrap_or(true)
        .to_string();
    let current_wpad = current_config
        .enable_wpad_discovery
        .unwrap_or(default_config.enable_wpad_discovery.unwrap_or(true))
        .to_string();

    options.push(ConfigOptionDescriptor {
        key: "enable_wpad_discovery",
        value_type: "bool",
        description: "Enable Web Proxy Auto-Discovery (WPAD) when no proxy URL is provided",
        default: default_wpad,
        current: current_wpad,
    });

    options.push(ConfigOptionDescriptor {
        key: "wpad_url",
        value_type: "string",
        description: "Override the WPAD URL used when discovery is enabled",
        default: clone_or_none(default_config.wpad_url.as_ref()),
        current: clone_or_none(current_config.wpad_url.as_ref()),
    });

    options.push(ConfigOptionDescriptor {
        key: "proxy_settings.enable_http_proxy",
        value_type: "bool",
        description: "Control whether HTTP proxy environment variables are managed",
        default: default_config.proxy_settings.enable_http_proxy.to_string(),
        current: current_config.proxy_settings.enable_http_proxy.to_string(),
    });

    options.push(ConfigOptionDescriptor {
        key: "proxy_settings.enable_https_proxy",
        value_type: "bool",
        description: "Control whether HTTPS proxy environment variables are managed",
        default: default_config.proxy_settings.enable_https_proxy.to_string(),
        current: current_config.proxy_settings.enable_https_proxy.to_string(),
    });

    options.push(ConfigOptionDescriptor {
        key: "proxy_settings.enable_ftp_proxy",
        value_type: "bool",
        description: "Control whether FTP proxy environment variables are managed",
        default: default_config.proxy_settings.enable_ftp_proxy.to_string(),
        current: current_config.proxy_settings.enable_ftp_proxy.to_string(),
    });

    options.push(ConfigOptionDescriptor {
        key: "proxy_settings.enable_all_proxy",
        value_type: "bool",
        description: "Control whether ALL_PROXY environment variables are managed",
        default: default_config.proxy_settings.enable_all_proxy.to_string(),
        current: current_config.proxy_settings.enable_all_proxy.to_string(),
    });

    options.push(ConfigOptionDescriptor {
        key: "proxy_settings.enable_proxy_rsync",
        value_type: "bool",
        description: "Control whether PROXY_RSYNC environment variables are managed",
        default: default_config.proxy_settings.enable_proxy_rsync.to_string(),
        current: current_config.proxy_settings.enable_proxy_rsync.to_string(),
    });

    options.push(ConfigOptionDescriptor {
        key: "proxy_settings.enable_no_proxy",
        value_type: "bool",
        description: "Control whether the NO_PROXY environment variable is managed",
        default: default_config.proxy_settings.enable_no_proxy.to_string(),
        current: current_config.proxy_settings.enable_no_proxy.to_string(),
    });

    Ok(options)
}

fn clone_or_none(value: Option<&String>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "None".to_string())
}

fn join_list(value: Option<&Vec<String>>) -> String {
    match value {
        Some(items) if !items.is_empty() => items.join(", "),
        _ => "None".to_string(),
    }
}

fn ssh_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn add_ssh_hosts(hosts_file: &str, proxy_host: &str) -> Result<()> {
    let _lock = ssh_lock().lock().unwrap_or_else(|e| e.into_inner());
    let ssh_config_path = get_ssh_config_path()?;
    ensure_parent_dir(&ssh_config_path)?;

    let host_entries = read_hosts_from_file(hosts_file)?;
    if host_entries.is_empty() {
        return Ok(());
    }

    create_backup(&ssh_config_path)?;

    let config = if ssh_config_path.exists() {
        fs::read_to_string(&ssh_config_path)?
    } else {
        String::new()
    };
    let had_trailing_newline = config.ends_with('\n');
    let mut lines: Vec<String> = collect_lines(config);

    let default_proxy_host = proxy_host.to_string();
    let mut host_proxy_map: HashMap<String, String> = HashMap::new();
    for entry in &host_entries {
        let proxy_value = entry
            .proxy
            .clone()
            .unwrap_or_else(|| default_proxy_host.clone());
        host_proxy_map.insert(entry.pattern.to_ascii_lowercase(), proxy_value);
    }
    let mut changed = false;
    let mut index = 0;

    while index < lines.len() {
        if is_host_line(&lines[index]) {
            let block_hosts = host_patterns_from_line(&lines[index]);
            let block_end = find_block_end(&lines, index + 1);

            let mut matched_proxies: Vec<&String> = Vec::new();
            for pattern in &block_hosts {
                let key = pattern.to_ascii_lowercase();
                if let Some(proxy_value) = host_proxy_map.get(&key) {
                    matched_proxies.push(proxy_value);
                }
            }

            if !matched_proxies.is_empty() {
                let first_proxy = matched_proxies[0];
                if matched_proxies.iter().any(|value| *value != first_proxy) {
                    return Err(anyhow!(
                        "Host block '{}' matches multiple proxy assignments; split hosts with differing proxies",
                        lines[index].trim()
                    ));
                }

                let expected_proxy =
                    format!("ProxyCommand /usr/bin/nc -X connect -x {first_proxy} %h %p");
                let proxy_line_idx = (index + 1..block_end).find(|&i| {
                    lines[i]
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("proxycommand ")
                });

                let indent = determine_block_indent(&lines, index + 1, block_end);
                let formatted_proxy = format!("{indent}{expected_proxy}");

                match proxy_line_idx {
                    Some(i) => {
                        if lines[i].trim() != expected_proxy || lines[i] != formatted_proxy {
                            lines[i] = formatted_proxy;
                            changed = true;
                        }
                    }
                    None => {
                        lines.insert(index + 1, formatted_proxy);
                        changed = true;
                    }
                }
            }

            index = find_block_end(&lines, index + 1);
            continue;
        }

        index += 1;
    }

    if changed {
        let mut new_content = lines.join("\n");
        if had_trailing_newline || new_content.is_empty() {
            new_content.push('\n');
        }
        fs::write(&ssh_config_path, new_content)?;
    }

    Ok(())
}

pub fn remove_ssh_hosts() -> Result<()> {
    let _lock = ssh_lock().lock().unwrap_or_else(|e| e.into_inner());
    let ssh_config_path = get_ssh_config_path()?;
    if !ssh_config_path.exists() {
        return Ok(());
    }

    let hosts_file = get_hosts_file_path()?;
    let host_entries = read_hosts_from_file(&hosts_file)?;
    if host_entries.is_empty() {
        return Ok(());
    }

    create_backup(&ssh_config_path)?;

    let config = fs::read_to_string(&ssh_config_path)?;
    let had_trailing_newline = config.ends_with('\n');
    let mut lines: Vec<String> = collect_lines(config);

    let host_set: HashSet<String> = host_entries
        .iter()
        .map(|entry| entry.pattern.to_ascii_lowercase())
        .collect();

    let mut changed = false;
    let mut index = 0;

    while index < lines.len() {
        if is_host_line(&lines[index]) {
            let block_hosts = host_patterns_from_line(&lines[index]);
            let matches_host = block_hosts
                .iter()
                .any(|pattern| host_set.contains(&pattern.to_ascii_lowercase()));

            let mut block_end = find_block_end(&lines, index + 1);

            if matches_host {
                let mut removal_indices: Vec<usize> = Vec::new();
                for (offset, line) in lines.iter().take(block_end).skip(index + 1).enumerate() {
                    let trimmed_lower = line.trim_start().to_ascii_lowercase();
                    if trimmed_lower.starts_with("proxycommand ")
                        && trimmed_lower.contains("/usr/bin/nc -x")
                    {
                        removal_indices.push(index + 1 + offset);
                    }
                }

                if !removal_indices.is_empty() {
                    for &idx in removal_indices.iter().rev() {
                        lines.remove(idx);
                        block_end -= 1;
                    }
                    // Clean up multiple blank lines after removal
                    while index + 1 < block_end
                        && lines[index + 1].trim().is_empty()
                        && (index + 2 == block_end
                            || lines[index + 2]
                                .trim_start()
                                .to_ascii_lowercase()
                                .starts_with("host "))
                    {
                        lines.remove(index + 1);
                        block_end -= 1;
                    }
                    changed = true;
                }
            }

            index = block_end;
            continue;
        }

        index += 1;
    }

    if changed {
        let mut new_content = lines.join("\n");
        if had_trailing_newline && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        fs::write(&ssh_config_path, new_content)?;
    }

    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct HostEntry {
    pattern: String,
    proxy: Option<String>,
}

fn read_hosts_from_file<P: AsRef<Path>>(hosts_file: P) -> Result<Vec<HostEntry>> {
    let path = hosts_file.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    let mut entries = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let entry = parse_host_line(trimmed).map_err(|err| {
            anyhow!(
                "Failed to parse hosts file {}:{}: {}",
                path.display(),
                idx + 1,
                err
            )
        })?;
        entries.push(entry);
    }

    Ok(entries)
}

fn parse_host_line(line: &str) -> Result<HostEntry> {
    let mut parts = line.split_whitespace();
    let pattern = parts
        .next()
        .ok_or_else(|| anyhow!("missing host pattern"))?
        .to_string();

    let mut proxy: Option<String> = None;

    for part in parts {
        if part.starts_with('#') {
            break;
        }

        let value = if let Some(rest) = part.strip_prefix("proxy=") {
            rest
        } else if proxy.is_none() {
            part
        } else {
            return Err(anyhow!("unexpected token '{part}'"));
        };

        if value.is_empty() {
            return Err(anyhow!("empty proxy value for host '{pattern}'"));
        }

        proxy = Some(value.to_string());
    }

    Ok(HostEntry { pattern, proxy })
}

fn create_backup(ssh_config_path: &Path) -> Result<()> {
    if !ssh_config_path.exists() {
        return Ok(());
    }

    if let Some(parent) = ssh_config_path.parent() {
        fs::create_dir_all(parent)?;
        let backup_path = parent.join("config.proxyctl-rs.bak");
        let contents = fs::read(ssh_config_path)?;
        fs::write(&backup_path, contents)?;
    }

    Ok(())
}

fn is_host_line(line: &str) -> bool {
    line.trim_start().to_ascii_lowercase().starts_with("host ")
}

fn host_patterns_from_line(line: &str) -> Vec<String> {
    line.split_whitespace()
        .skip(1)
        .map(|s| s.to_string())
        .collect()
}

fn find_block_end(lines: &[String], mut index: usize) -> usize {
    while index < lines.len() {
        if is_host_line(&lines[index]) {
            break;
        }
        index += 1;
    }
    index
}

fn determine_block_indent(lines: &[String], start: usize, end: usize) -> String {
    for line in lines.iter().take(end).skip(start) {
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        if indent.is_empty() {
            continue;
        }
        return indent;
    }
    "    ".to_string()
}

fn collect_lines(content: String) -> Vec<String> {
    if content.is_empty() {
        Vec::new()
    } else {
        content.lines().map(|line| line.to_string()).collect()
    }
}

fn get_ssh_config_path() -> Result<std::path::PathBuf> {
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".ssh").join("config"));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".ssh").join("config"))
}
