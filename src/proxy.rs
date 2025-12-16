use crate::config;
use crate::db;
use crate::defaults;
use crate::detect;
use anyhow::{anyhow, Result};
use colored::Colorize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
    if proxy_settings.enable_all_proxy {
        set_env_vars(&ALL_PROXY_KEYS, proxy_url);
    }
    if proxy_settings.enable_proxy_rsync {
        set_env_vars(&PROXY_RSYNC_KEYS, proxy_url);
    }
    if let Some(ref no_proxy_str) = no_proxy_value {
        set_env_vars(&NO_PROXY_KEYS, no_proxy_str);
    }

    persist_proxy_settings(&proxy_settings, proxy_url, no_proxy_value.as_deref())?;

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
    if proxy_settings.enable_all_proxy {
        state.all_proxy = Some(proxy_url.to_string());
    }
    if proxy_settings.enable_proxy_rsync {
        state.proxy_rsync = Some(proxy_url.to_string());
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
    clear_env_vars(&ALL_PROXY_KEYS);
    clear_env_vars(&PROXY_RSYNC_KEYS);
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
        status_lines.push(render_status_line(
            "HTTP Proxy",
            state.http_proxy.as_deref(),
            &HTTP_PROXY_KEYS,
        ));
    }
    if proxy_settings.enable_https_proxy {
        status_lines.push(render_status_line(
            "HTTPS Proxy",
            state.https_proxy.as_deref(),
            &HTTPS_PROXY_KEYS,
        ));
    }
    if proxy_settings.enable_ftp_proxy {
        status_lines.push(render_status_line(
            "FTP Proxy",
            state.ftp_proxy.as_deref(),
            &FTP_PROXY_KEYS,
        ));
    }
    if proxy_settings.enable_all_proxy {
        status_lines.push(render_status_line(
            "All Proxy",
            state.all_proxy.as_deref(),
            &ALL_PROXY_KEYS,
        ));
    }
    if proxy_settings.enable_proxy_rsync {
        status_lines.push(render_status_line(
            "Proxy Rsync",
            state.proxy_rsync.as_deref(),
            &PROXY_RSYNC_KEYS,
        ));
    }
    if proxy_settings.enable_no_proxy {
        status_lines.push(render_status_line(
            "No Proxy",
            state.no_proxy.as_deref(),
            &NO_PROXY_KEYS,
        ));
    }

    Ok(status_lines.join("\n"))
}

fn render_status_line(label: &str, state_value: Option<&str>, keys: &[&str]) -> String {
    let env_value = get_env_value(keys);
    let value = state_value.or(env_value.as_deref());

    let status = match value {
        Some(v) if !v.is_empty() => v.green().bold().to_string(),
        _ => "Not set".red().bold().to_string(),
    };

    format!("{}: {}", label.bold(), status)
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
                return resolved_from_value(&value)
                    .map_err(|err| anyhow!("Failed to parse default proxy '{value}': {err}"));
            }

            Err(last_error
                .unwrap_or_else(|| anyhow!("No valid proxies discovered from WPAD response")))
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
const ALL_PROXY_KEYS: [&str; 2] = ["all_proxy", "ALL_PROXY"];
const PROXY_RSYNC_KEYS: [&str; 2] = ["proxy_rsync", "PROXY_RSYNC"];
const NO_PROXY_KEYS: [&str; 2] = ["no_proxy", "NO_PROXY"];
const MANAGED_START: &str = "### MANAGED BY PROXYCTL-RS START (DO NOT EDIT)";
const MANAGED_END: &str = "### MANAGED BY PROXYCTL-RS END (DO NOT EDIT)";

fn persist_proxy_settings(
    proxy_settings: &config::ProxySettings,
    proxy_url: &str,
    no_proxy: Option<&str>,
) -> Result<()> {
    let profiles = resolve_shell_profiles()?;
    if profiles.is_empty() {
        return Ok(());
    }

    let exports = gather_proxy_exports(proxy_settings, proxy_url, no_proxy);
    if exports.is_empty() {
        for profile in profiles {
            remove_managed_block(&profile)?;
        }
        return Ok(());
    }

    for profile in profiles {
        write_managed_block(&profile, &exports)?;
    }

    Ok(())
}

fn remove_persisted_settings() -> Result<()> {
    for profile in resolve_shell_profiles()? {
        remove_managed_block(&profile)?;
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

fn get_env_value(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(value) = env::var(key) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

async fn save_env_state(state: &db::EnvState) -> Result<()> {
    let db_path = db::get_db_path();
    db::save_env_state(&db_path, state).await
}

async fn load_env_state() -> Result<db::EnvState> {
    let db_path = db::get_db_path();
    db::load_env_state(&db_path).await
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
    const VARS: [&[&str]; 5] = [
        &HTTPS_PROXY_KEYS,
        &HTTP_PROXY_KEYS,
        &ALL_PROXY_KEYS,
        &FTP_PROXY_KEYS,
        &PROXY_RSYNC_KEYS,
    ];
    for keys in VARS {
        if let Some(value) = get_env_value(keys) {
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

fn gather_proxy_exports(
    proxy_settings: &config::ProxySettings,
    proxy_url: &str,
    no_proxy: Option<&str>,
) -> Vec<String> {
    let mut exports = Vec::new();

    if proxy_settings.enable_http_proxy {
        add_export_lines(&mut exports, &HTTP_PROXY_KEYS, proxy_url);
    }
    if proxy_settings.enable_https_proxy {
        add_export_lines(&mut exports, &HTTPS_PROXY_KEYS, proxy_url);
    }
    if proxy_settings.enable_ftp_proxy {
        add_export_lines(&mut exports, &FTP_PROXY_KEYS, proxy_url);
    }
    if proxy_settings.enable_all_proxy {
        add_export_lines(&mut exports, &ALL_PROXY_KEYS, proxy_url);
    }
    if proxy_settings.enable_proxy_rsync {
        add_export_lines(&mut exports, &PROXY_RSYNC_KEYS, proxy_url);
    }
    if proxy_settings.enable_no_proxy {
        if let Some(value) = no_proxy {
            if !value.is_empty() {
                add_export_lines(&mut exports, &NO_PROXY_KEYS, value);
            }
        }
    }

    exports
}

fn add_export_lines(target: &mut Vec<String>, keys: &[&str], value: &str) {
    for key in keys {
        target.push(format!("export {key}=\"{value}\""));
    }
}

fn resolve_shell_profiles() -> Result<Vec<PathBuf>> {
    let integration = config::get_shell_integration()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

    let config::ShellIntegration {
        detect_shell,
        default_shell,
        shells,
        profile_paths,
    } = integration;

    let mut profiles = Vec::new();
    let mut seen = HashSet::new();

    for path in profile_paths {
        let expanded = expand_profile_path(&path, &home);
        push_unique_path(&mut profiles, &mut seen, expanded);
    }

    let mut shell_names: HashSet<String> = HashSet::new();

    if detect_shell {
        if let Ok(shell_value) = env::var("SHELL") {
            if let Some(name) = Path::new(shell_value.trim())
                .file_name()
                .and_then(|value| value.to_str())
            {
                if !name.is_empty() {
                    shell_names.insert(name.to_ascii_lowercase());
                }
            }
        }
    }

    for shell in shells {
        if !shell.is_empty() {
            shell_names.insert(shell.to_ascii_lowercase());
        }
    }

    if let Some(default_shell) = default_shell {
        if !default_shell.is_empty() {
            shell_names.insert(default_shell.to_ascii_lowercase());
        }
    }

    for shell in shell_names {
        for profile in shell_profiles_for(&shell, &home) {
            push_unique_path(&mut profiles, &mut seen, profile);
        }
    }

    Ok(profiles)
}

fn shell_profiles_for(shell: &str, home: &Path) -> Vec<PathBuf> {
    match shell {
        "zsh" => vec![select_profile(&[".zshenv", ".zprofile", ".zshrc"], home)],
        "bash" => vec![select_profile(&[".bash_profile", ".bashrc"], home)],
        _ => Vec::new(),
    }
    .into_iter()
    .flatten()
    .collect()
}

fn select_profile(candidates: &[&str], home: &Path) -> Option<PathBuf> {
    for candidate in candidates {
        let path = home.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }
    candidates.first().map(|candidate| home.join(candidate))
}

fn expand_profile_path(value: &str, home: &Path) -> PathBuf {
    let trimmed = value.trim();
    if trimmed.starts_with("~/") {
        return home.join(trimmed.trim_start_matches("~/"));
    }

    let path = PathBuf::from(trimmed);
    if path.is_relative() {
        home.join(path)
    } else {
        path
    }
}

fn push_unique_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        paths.push(path);
    }
}

fn write_managed_block(profile: &Path, exports: &[String]) -> Result<()> {
    ensure_parent_directory(profile)?;
    let existing = if profile.exists() {
        fs::read_to_string(profile)?
    } else {
        String::new()
    };

    let (mut base, _) = strip_managed_block(&existing);

    if !base.is_empty() && !base.ends_with('\n') {
        base.push('\n');
    }
    if !base.is_empty() && !base.ends_with("\n\n") {
        base.push('\n');
    }

    let mut block_lines = Vec::with_capacity(exports.len() + 2);
    block_lines.push(MANAGED_START.to_string());
    block_lines.extend(exports.iter().cloned());
    block_lines.push(MANAGED_END.to_string());
    let block = block_lines.join("\n");

    base.push_str(&block);
    base.push('\n');

    fs::write(profile, base)?;
    Ok(())
}

fn remove_managed_block(profile: &Path) -> Result<()> {
    if !profile.exists() {
        return Ok(());
    }

    let existing = fs::read_to_string(profile)?;
    let (updated, changed) = strip_managed_block(&existing);
    if changed {
        fs::write(profile, updated)?;
    }

    Ok(())
}

fn strip_managed_block(content: &str) -> (String, bool) {
    let mut current = content.to_string();
    let mut changed = false;

    loop {
        let Some(start_idx) = current.find(MANAGED_START) else {
            break;
        };

        let Some(rel_end) = current[start_idx..].find(MANAGED_END) else {
            break;
        };

        let end_idx = start_idx + rel_end + MANAGED_END.len();

        let mut remove_start = start_idx;
        while remove_start > 0 && matches!(current.as_bytes()[remove_start - 1], b'\n') {
            remove_start -= 1;
        }

        let mut remove_end = end_idx;
        while remove_end < current.len() && matches!(current.as_bytes()[remove_end], b'\n') {
            remove_end += 1;
        }

        current.replace_range(remove_start..remove_end, "");
        changed = true;
    }

    (current, changed)
}

fn ensure_parent_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
