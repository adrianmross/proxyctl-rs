use crate::config;
use crate::db;
use crate::defaults;
use crate::detect;
use anyhow::{anyhow, Result};
use std::env;
use std::fs;

pub async fn set_proxy(proxy_url: &str) -> Result<()> {
    let proxy_settings = config::get_proxy_settings()?;

    let no_proxy_value = if proxy_settings.enable_no_proxy {
        let value = if let Some(custom_no_proxy) = config::get_custom_no_proxy()? {
            custom_no_proxy.join(",")
        } else {
            defaults::default_no_proxy()
        };
        Some(value)
    } else {
        None
    };

    if proxy_settings.enable_http_proxy {
        set_env_vars(&HTTP_PROXY_KEYS, proxy_url);
    }
    if proxy_settings.enable_https_proxy {
        set_env_vars(&HTTPS_PROXY_KEYS, proxy_url);
    }
    if proxy_settings.enable_ftp_proxy {
        set_env_vars(&FTP_PROXY_KEYS, proxy_url);
    }
    if let Some(ref no_proxy_str) = no_proxy_value {
        set_env_vars(&NO_PROXY_KEYS, no_proxy_str);
    }

    persist_proxy_settings(proxy_url, no_proxy_value.as_deref())?;

    let mut state = db::EnvState::default();
    if proxy_settings.enable_http_proxy {
        state.http_proxy = Some(proxy_url.to_string());
    }
    if proxy_settings.enable_https_proxy {
        state.https_proxy = Some(proxy_url.to_string());
    }
    if proxy_settings.enable_ftp_proxy {
        state.ftp_proxy = Some(proxy_url.to_string());
    }
    if let Some(no_proxy_str) = no_proxy_value {
        state.no_proxy = Some(no_proxy_str);
    }
    save_env_state(&state).await?;

    Ok(())
}

pub async fn disable_proxy() -> Result<()> {
    clear_env_vars(&HTTP_PROXY_KEYS);
    clear_env_vars(&HTTPS_PROXY_KEYS);
    clear_env_vars(&FTP_PROXY_KEYS);
    clear_env_vars(&NO_PROXY_KEYS);

    remove_persisted_settings()?;
    save_env_state(&db::EnvState::default()).await?;

    Ok(())
}

pub async fn get_status() -> Result<String> {
    let proxy_settings = config::get_proxy_settings()?;
    let state = load_env_state()
        .await
        .unwrap_or_else(|_| db::EnvState::default());

    let mut status_lines = Vec::new();

    if proxy_settings.enable_http_proxy {
        let env_value = env::var("http_proxy").ok();
        let value = state.http_proxy.as_deref().or(env_value.as_deref());
        status_lines.push(format!("HTTP Proxy: {}", value.unwrap_or("Not set")));
    }
    if proxy_settings.enable_https_proxy {
        let env_value = env::var("https_proxy").ok();
        let value = state.https_proxy.as_deref().or(env_value.as_deref());
        status_lines.push(format!("HTTPS Proxy: {}", value.unwrap_or("Not set")));
    }
    if proxy_settings.enable_ftp_proxy {
        let env_value = env::var("ftp_proxy").ok();
        let value = state.ftp_proxy.as_deref().or(env_value.as_deref());
        status_lines.push(format!("FTP Proxy: {}", value.unwrap_or("Not set")));
    }
    if proxy_settings.enable_no_proxy {
        let env_value = env::var("no_proxy").ok();
        let value = state.no_proxy.as_deref().or(env_value.as_deref());
        status_lines.push(format!("No Proxy: {}", value.unwrap_or("Not set")));
    }

    Ok(status_lines.join("\n"))
}

#[derive(Debug, Clone)]
pub struct ResolvedProxy {
    pub proxy_url: String,
    pub proxy_host: String,
}

pub async fn resolve_proxy(proxy: Option<&str>) -> Result<ResolvedProxy> {
    if let Some(value) = proxy {
        return resolved_from_value(value);
    }

    if let Some(env_proxy) = proxy_from_env() {
        return Ok(env_proxy);
    }

    let default_proxy = config::get_default_proxy()?;
    let mut last_error: Option<anyhow::Error> = None;

    match detect::detect_proxy_candidates().await {
        Ok(candidates) => {
            for candidate in candidates {
                match resolved_from_value(&candidate) {
                    Ok(resolved) => return Ok(resolved),
                    Err(err) => last_error = Some(err),
                }
            }

            if let Some(value) = default_proxy {
                return resolved_from_value(&value).map_err(|err| {
                    anyhow!("Failed to parse default proxy '{value}': {err}")
                });
            }

            Err(last_error.unwrap_or_else(|| {
                anyhow!("No valid proxies discovered from WPAD response")
            }))
        }
        Err(err) => {
            if let Some(value) = default_proxy {
                return resolved_from_value(&value).map_err(|parse_err| {
                    anyhow!("Failed to parse default proxy '{value}': {parse_err}")
                });
            }
            Err(err)
        }
    }
}

const HTTP_PROXY_KEYS: [&str; 2] = ["http_proxy", "HTTP_PROXY"];
const HTTPS_PROXY_KEYS: [&str; 2] = ["https_proxy", "HTTPS_PROXY"];
const FTP_PROXY_KEYS: [&str; 2] = ["ftp_proxy", "FTP_PROXY"];
const NO_PROXY_KEYS: [&str; 2] = ["no_proxy", "NO_PROXY"];

fn persist_proxy_settings(proxy_url: &str, no_proxy: Option<&str>) -> Result<()> {
    // Try to detect shell and update profile
    let shell_profile = detect_shell_profile()?;
    if let Some(profile) = shell_profile {
        let proxy_settings = config::get_proxy_settings()?;
        let content = fs::read_to_string(&profile)?;
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        // Remove existing proxy settings
        lines.retain(|line| {
            !line.contains("export http_proxy=")
                && !line.contains("export https_proxy=")
                && !line.contains("export ftp_proxy=")
                && !line.contains("export no_proxy=")
        });

        // Add new settings based on config
        if proxy_settings.enable_http_proxy {
            lines.push(format!("export http_proxy=\"{proxy_url}\""));
        }
        if proxy_settings.enable_https_proxy {
            lines.push(format!("export https_proxy=\"{proxy_url}\""));
        }
        if proxy_settings.enable_ftp_proxy {
            lines.push(format!("export ftp_proxy=\"{proxy_url}\""));
        }
        if proxy_settings.enable_no_proxy {
            if let Some(value) = no_proxy {
                if !value.is_empty() {
                    lines.push(format!("export no_proxy=\"{value}\""));
                }
            }
        }

        fs::write(&profile, lines.join("\n"))?;
    }

    Ok(())
}

fn remove_persisted_settings() -> Result<()> {
    let shell_profile = detect_shell_profile()?;
    if let Some(profile) = shell_profile {
        let content = fs::read_to_string(&profile)?;
        let lines: Vec<String> = content
            .lines()
            .filter(|line| {
                !line.contains("export http_proxy=")
                    && !line.contains("export https_proxy=")
                    && !line.contains("export ftp_proxy=")
                    && !line.contains("export no_proxy=")
            })
            .map(|s| s.to_string())
            .collect();

        fs::write(&profile, lines.join("\n"))?;
    }

    Ok(())
}

fn set_env_vars(keys: &[&str], value: &str) {
    for key in keys {
        env::set_var(key, value);
    }
}

fn clear_env_vars(keys: &[&str]) {
    for key in keys {
        env::remove_var(key);
    }
}

async fn save_env_state(state: &db::EnvState) -> Result<()> {
    let db_path = db::get_db_path();
    db::save_env_state(&db_path, state).await
}

async fn load_env_state() -> Result<db::EnvState> {
    let db_path = db::get_db_path();
    db::load_env_state(&db_path).await
}

fn detect_shell_profile() -> Result<Option<String>> {
    // Detect shell and return profile path
    let shell = env::var("SHELL").unwrap_or_default();
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    let profile = if shell.contains("zsh") {
        home.join(".zshrc")
    } else if shell.contains("bash") {
        // Prefer .bashrc for interactive, but check .bash_profile for login
        let bashrc = home.join(".bashrc");
        if bashrc.exists() {
            bashrc
        } else {
            home.join(".bash_profile")
        }
    } else {
        return Ok(None); // Unsupported shell
    };

    Ok(Some(profile.to_string_lossy().to_string()))
}

fn resolved_from_value(value: &str) -> Result<ResolvedProxy> {
    let host = extract_proxy_host(value)
        .ok_or_else(|| anyhow!("unable to determine proxy host from '{value}'"))?;
    Ok(ResolvedProxy {
        proxy_url: value.to_string(),
        proxy_host: host,
    })
}

fn proxy_from_env() -> Option<ResolvedProxy> {
    const VARS: [&str; 4] = ["https_proxy", "HTTPS_PROXY", "http_proxy", "HTTP_PROXY"];
    for key in VARS {
        if let Ok(value) = env::var(key) {
            if let Some(host) = extract_proxy_host(&value) {
                return Some(ResolvedProxy {
                    proxy_url: value,
                    proxy_host: host,
                });
            }
        }
    }
    None
}

fn extract_proxy_host(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let try_parse = |input: &str| -> Option<String> {
        if let Ok(url) = reqwest::Url::parse(input) {
            if let Some(host) = url.host_str() {
                if let Some(port) = url.port().or_else(|| url.port_or_known_default()) {
                    return Some(format!("{host}:{port}"));
                }
            }
        }
        None
    };

    if let Some(host) = try_parse(trimmed) {
        return Some(host);
    }

    if let Some(host) = try_parse(&format!("http://{trimmed}")) {
        return Some(host);
    }

    let mut candidate = trimmed;
    if let Some(stripped) = candidate.strip_prefix("PROXY ") {
        candidate = stripped.trim();
    }
    if let Some(stripped) = candidate.strip_prefix("proxy ") {
        candidate = stripped.trim();
    }

    if let Some(token) = candidate.split_whitespace().next() {
        candidate = token.trim();
    }

    candidate = candidate.trim_end_matches(';').trim().trim_end_matches('/');
    if candidate.is_empty() {
        return None;
    }

    if let Some(host) = try_parse(&format!("http://{candidate}")) {
        return Some(host);
    }

    if let Some((host_part, port_part)) = split_host_port(candidate) {
        return Some(format!("{host_part}:{port_part}"));
    }

    None
}

fn split_host_port(input: &str) -> Option<(String, String)> {
    let input = input.trim();
    if input.starts_with('[') {
        if let Some(idx) = input.find("]: ") {
            let host = &input[..idx + 1];
            let port = &input[idx + 2..];
            return Some((host.trim().to_string(), port.trim().to_string()));
        }
        if let Some(idx) = input.rfind("]:") {
            let host = &input[..=idx];
            let port = &input[idx + 2..];
            return Some((host.trim().to_string(), port.trim().to_string()));
        }
    }

    if let Some((host, port)) = input.rsplit_once(':') {
        let host = host.trim();
        let port = port.trim();
        if !host.is_empty() && !port.is_empty() {
            return Some((host.to_string(), port.to_string()));
        }
    }

    None
}
