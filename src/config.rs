use anyhow::{anyhow, Result};
use config::{Config as ConfigLoader, File};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
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
    #[serde(default, deserialize_with = "deserialize_no_proxy")]
    pub no_proxy: Option<Vec<String>>,
    pub enable_wpad_discovery: Option<bool>,
    pub wpad_url: Option<String>,
    #[serde(default)]
    pub proxy_settings: ProxySettings,
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
            enable_wpad_discovery: Some(true),
            wpad_url: Some(defaults::default_wpad_url()),
            proxy_settings: ProxySettings::default(),
        }
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
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

pub fn add_ssh_hosts(hosts_file: &str, proxy_host: &str) -> Result<()> {
    let ssh_config_path = get_ssh_config_path()?;
    ensure_parent_dir(&ssh_config_path)?;

    let hosts = read_hosts_from_file(hosts_file)?;
    if hosts.is_empty() {
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

    let host_set: HashSet<String> = hosts.iter().map(|h| h.to_ascii_lowercase()).collect();

    let expected_proxy = format!("ProxyCommand /usr/bin/nc -X connect -x {proxy_host} %h %p");
    let mut changed = false;
    let mut index = 0;

    while index < lines.len() {
        if is_host_line(&lines[index]) {
            let block_hosts = host_patterns_from_line(&lines[index]);
            let matches_host = block_hosts
                .iter()
                .any(|pattern| host_set.contains(&pattern.to_ascii_lowercase()));

            let block_end = find_block_end(&lines, index + 1);

            if matches_host {
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
    let ssh_config_path = get_ssh_config_path()?;
    if !ssh_config_path.exists() {
        return Ok(());
    }

    let hosts_file = get_hosts_file_path()?;
    let hosts = read_hosts_from_file(&hosts_file)?;
    if hosts.is_empty() {
        return Ok(());
    }

    create_backup(&ssh_config_path)?;

    let config = fs::read_to_string(&ssh_config_path)?;
    let had_trailing_newline = config.ends_with('\n');
    let mut lines: Vec<String> = collect_lines(config);

    let host_set: HashSet<String> = hosts.iter().map(|h| h.to_ascii_lowercase()).collect();

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
                    let trimmed = line.trim_start().to_ascii_lowercase();
                    if trimmed.starts_with("proxycommand ") && trimmed.contains("/usr/bin/nc -X") {
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

fn read_hosts_from_file<P: AsRef<Path>>(hosts_file: P) -> Result<Vec<String>> {
    let path = hosts_file.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    let hosts = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect();
    Ok(hosts)
}

fn create_backup(ssh_config_path: &Path) -> Result<()> {
    if !ssh_config_path.exists() {
        return Ok(());
    }

    if let Some(parent) = ssh_config_path.parent() {
        fs::create_dir_all(parent)?;
        let backup_path = parent.join("config.proxyctl-rs.bak");
        if backup_path.exists() {
            fs::remove_file(&backup_path)?;
        }
        fs::copy(ssh_config_path, backup_path)?;
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
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".ssh").join("config"))
}
