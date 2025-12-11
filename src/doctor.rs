use crate::{config, db};
use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

struct DoctorSummary {
    lines: Vec<String>,
    healthy: bool,
}

pub async fn run() -> Result<()> {
    let summary = evaluate().await?;

    for line in &summary.lines {
        println!("{line}");
    }

    if summary.healthy {
        Ok(())
    } else {
        Err(anyhow!("doctor checks failed"))
    }
}

async fn evaluate() -> Result<DoctorSummary> {
    let mut lines = Vec::new();
    let mut healthy = true;

    match check_config() {
        Ok(message) => lines.push(format!("Config: OK - {message}")),
        Err(err) => {
            lines.push(format!("Config: ERR - {err}"));
            healthy = false;
        }
    }

    match check_database().await {
        Ok(message) => lines.push(format!("Database: OK - {message}")),
        Err(err) => {
            lines.push(format!("Database: ERR - {err}"));
            healthy = false;
        }
    }

    if healthy {
        lines.push("Doctor summary: all checks passed".to_string());
    } else {
        lines.push("Doctor summary: issues detected".to_string());
    }

    Ok(DoctorSummary { lines, healthy })
}

fn check_config() -> Result<String> {
    let config_dir = config::get_config_dir().context("finding config directory")?;
    let config_file = config_dir.join("config.toml");

    config::load_config()
        .with_context(|| format!("loading configuration from {}", config_file.display()))?;

    let hosts_path = config::get_hosts_file_path().context("resolving hosts file path")?;
    if !hosts_path.exists() {
        return Err(anyhow!("expected hosts file at {}", hosts_path.display()));
    }

    Ok(format!(
        "configuration file at {} parsed successfully",
        config_file.display()
    ))
}

async fn check_database() -> Result<String> {
    let db_path = db::get_db_path();
    db::init_db(&db_path)
        .await
        .with_context(|| format!("initializing database at {db_path}"))?;

    db::load_env_state(&db_path)
        .await
        .with_context(|| format!("querying env_state table at {db_path}"))?;

    let file_path = PathBuf::from(&db_path);
    Ok(format!("database reachable at {}", file_path.display()))
}
