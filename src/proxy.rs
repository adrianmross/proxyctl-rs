use crate::config;
use crate::defaults;
use anyhow::Result;
use std::env;
use std::fs;

pub fn set_proxy(proxy_url: &str) -> Result<()> {
    let proxy_settings = config::get_proxy_settings()?;

    // Set environment variables based on config
    if proxy_settings.enable_http_proxy {
        unsafe { env::set_var("http_proxy", proxy_url) };
    }
    if proxy_settings.enable_https_proxy {
        unsafe { env::set_var("https_proxy", proxy_url) };
    }
    if proxy_settings.enable_ftp_proxy {
        unsafe { env::set_var("ftp_proxy", proxy_url) };
    }

    // Set no_proxy
    if proxy_settings.enable_no_proxy {
        let no_proxy_str = if let Some(custom_no_proxy) = config::get_custom_no_proxy()? {
            // Use custom no_proxy instead of defaults
            custom_no_proxy.join(",")
        } else {
            // Use defaults
            defaults::default_no_proxy()
        };
        unsafe { env::set_var("no_proxy", &no_proxy_str) };

        // Persist to shell profile
        persist_proxy_settings(proxy_url, &no_proxy_str)?;
    } else {
        // Still persist other settings if enabled
        persist_proxy_settings(proxy_url, "")?;
    }

    // Set environment variables after persisting
    if proxy_settings.enable_http_proxy {
        unsafe { env::set_var("http_proxy", proxy_url) };
    }
    if proxy_settings.enable_https_proxy {
        unsafe { env::set_var("https_proxy", proxy_url) };
    }
    if proxy_settings.enable_ftp_proxy {
        unsafe { env::set_var("ftp_proxy", proxy_url) };
    }
    if proxy_settings.enable_no_proxy {
        let no_proxy_str = if let Some(custom_no_proxy) = config::get_custom_no_proxy()? {
            custom_no_proxy.join(",")
        } else {
            defaults::default_no_proxy()
        };
        unsafe { env::set_var("no_proxy", &no_proxy_str) };
    }

    Ok(())
}

pub fn disable_proxy() -> Result<()> {
    let proxy_settings = config::get_proxy_settings()?;

    // Remove environment variables based on config
    if proxy_settings.enable_http_proxy {
        unsafe { env::remove_var("http_proxy") };
    }
    if proxy_settings.enable_https_proxy {
        unsafe { env::remove_var("https_proxy") };
    }
    if proxy_settings.enable_ftp_proxy {
        unsafe { env::remove_var("ftp_proxy") };
    }
    if proxy_settings.enable_no_proxy {
        unsafe { env::remove_var("no_proxy") };
    }

    // Remove from shell profile
    remove_persisted_settings()?;

    Ok(())
}

pub fn get_status() -> Result<String> {
    let proxy_settings = config::get_proxy_settings()?;

    let mut status_lines = Vec::new();

    if proxy_settings.enable_http_proxy {
        let http_proxy = env::var("http_proxy").unwrap_or_else(|_| "Not set".to_string());
        status_lines.push(format!("HTTP Proxy: {http_proxy}"));
    }
    if proxy_settings.enable_https_proxy {
        let https_proxy = env::var("https_proxy").unwrap_or_else(|_| "Not set".to_string());
        status_lines.push(format!("HTTPS Proxy: {https_proxy}"));
    }
    if proxy_settings.enable_ftp_proxy {
        let ftp_proxy = env::var("ftp_proxy").unwrap_or_else(|_| "Not set".to_string());
        status_lines.push(format!("FTP Proxy: {ftp_proxy}"));
    }
    if proxy_settings.enable_no_proxy {
        let no_proxy = env::var("no_proxy").unwrap_or_else(|_| "Not set".to_string());
        status_lines.push(format!("No Proxy: {no_proxy}"));
    }

    Ok(status_lines.join("\n"))
}

fn persist_proxy_settings(proxy_url: &str, no_proxy: &str) -> Result<()> {
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
        if proxy_settings.enable_no_proxy && !no_proxy.is_empty() {
            lines.push(format!("export no_proxy=\"{no_proxy}\""));
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
