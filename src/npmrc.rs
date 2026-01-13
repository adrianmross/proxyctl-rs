use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config;

pub fn is_enabled() -> Result<bool> {
    Ok(config::get_npmrc_settings()?.enabled)
}

pub fn activate_proxy_profile() -> Result<()> {
    let settings = config::get_npmrc_settings()?;
    if !settings.enabled {
        return Ok(());
    }

    apply_profile(&settings, settings.proxy_profile.as_deref())
}

pub fn restore_default_profile() -> Result<()> {
    let settings = config::get_npmrc_settings()?;
    if !settings.enabled {
        return Ok(());
    }

    if let Some(profile) = settings.default_profile.as_deref() {
        apply_profile(&settings, Some(profile))
    } else {
        remove_active_link(&settings)
    }
}

fn apply_profile(settings: &config::NpmrcSettings, profile_name: Option<&str>) -> Result<()> {
    let profile = profile_name
        .and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .ok_or_else(|| anyhow!("npmrc profile name is not configured"))?;

    let directory = settings.directory.as_deref().unwrap_or("~/.npmrcs");
    let active_path = settings.active_path.as_deref().unwrap_or("~/.npmrc");

    let directory = expand_path(directory)?;
    let active_path = expand_path(active_path)?;

    ensure_parent_directory(&active_path)?;

    let target = directory.join(&profile);
    if !target.exists() {
        return Err(anyhow!(
            "npmrc profile '{}' not found at {}",
            profile,
            target.display()
        ));
    }

    if is_same_link(&active_path, &target)? {
        return Ok(());
    }

    if let Ok(metadata) = fs::symlink_metadata(&active_path) {
        if metadata.file_type().is_dir() {
            return Err(anyhow!(
                "Active npmrc path '{}' is a directory",
                active_path.display()
            ));
        }
        fs::remove_file(&active_path).with_context(|| {
            format!(
                "Failed to remove existing npmrc symlink at {}",
                active_path.display()
            )
        })?;
    } else if active_path.exists() {
        fs::remove_file(&active_path).with_context(|| {
            format!(
                "Failed to remove existing npmrc file at {}",
                active_path.display()
            )
        })?;
    }

    create_symlink(&target, &active_path).with_context(|| {
        format!(
            "Failed to link {} to {}",
            active_path.display(),
            target.display()
        )
    })
}

fn remove_active_link(settings: &config::NpmrcSettings) -> Result<()> {
    let active_path = settings.active_path.as_deref().unwrap_or("~/.npmrc");
    let active_path = expand_path(active_path)?;

    if let Ok(metadata) = fs::symlink_metadata(&active_path) {
        if metadata.file_type().is_dir() {
            return Err(anyhow!(
                "Active npmrc path '{}' is a directory",
                active_path.display()
            ));
        }
        fs::remove_file(&active_path).with_context(|| {
            format!(
                "Failed to remove existing npmrc link at {}",
                active_path.display()
            )
        })?;
    } else if active_path.exists() {
        fs::remove_file(&active_path).with_context(|| {
            format!(
                "Failed to remove existing npmrc file at {}",
                active_path.display()
            )
        })?;
    }

    Ok(())
}

fn ensure_parent_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory for {}", path.display()))?;
    }
    Ok(())
}

fn expand_path(value: &str) -> Result<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Path value cannot be empty"));
    }

    if trimmed == "~" {
        return dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"));
    }

    if trimmed.starts_with("~/") {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        let rest = trimmed.trim_start_matches("~/");
        return Ok(home.join(rest));
    }

    Ok(PathBuf::from(trimmed))
}

fn is_same_link(link: &Path, target: &Path) -> Result<bool> {
    match fs::symlink_metadata(link) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                if let Ok(existing) = fs::read_link(link) {
                    return Ok(existing == target);
                }
            }
            Ok(false)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, NpmrcSettings};
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct Fixture {
        _lock: std::sync::MutexGuard<'static, ()>,
        _temp: TempDir,
        npmrc_dir: PathBuf,
        active_path: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let lock = env_lock().lock().unwrap_or_else(|e| e.into_inner());
            let temp = tempfile::tempdir().expect("tempdir");
            let home_dir = temp.path().join("home");
            let config_dir = home_dir.join(".config").join("proxyctl-rs");
            let data_dir = home_dir.join(".local").join("share").join("proxyctl-rs");
            let npmrc_dir = home_dir.join("npmrcs");
            let active_path = home_dir.join(".npmrc");
            fs::create_dir_all(&config_dir).expect("config dir");
            fs::create_dir_all(&data_dir).expect("data dir");
            fs::create_dir_all(&npmrc_dir).expect("npmrc dir");

            std::env::set_var("HOME", &home_dir);
            std::env::set_var("XDG_CONFIG_HOME", home_dir.join(".config"));
            std::env::set_var("XDG_DATA_HOME", home_dir.join(".local").join("share"));

            let mut config = AppConfig::default();
            config.npmrc = NpmrcSettings {
                enabled: true,
                directory: Some(npmrc_dir.to_string_lossy().into_owned()),
                active_path: Some(active_path.to_string_lossy().into_owned()),
                default_profile: Some("default".to_string()),
                proxy_profile: Some("proxy".to_string()),
            };
            config::save_config(&config).expect("save config");

            fs::write(npmrc_dir.join("default"), "registry=https://default").expect("default");
            fs::write(npmrc_dir.join("proxy"), "registry=https://proxy").expect("proxy");

            Self {
                _lock: lock,
                _temp: temp,
                npmrc_dir,
                active_path,
            }
        }
    }

    #[test]
    fn activate_and_restore_profiles() {
        let fixture = Fixture::new();

        activate_proxy_profile().expect("activate proxy profile");
        let link_target = fs::read_link(&fixture.active_path).expect("read link");
        assert_eq!(link_target, fixture.npmrc_dir.join("proxy"));

        restore_default_profile().expect("restore default profile");
        let link_target = fs::read_link(&fixture.active_path).expect("read link default");
        assert_eq!(link_target, fixture.npmrc_dir.join("default"));
    }
}
