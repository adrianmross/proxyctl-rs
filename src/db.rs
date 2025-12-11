use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use turso::Builder;

use crate::config;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct EnvState {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub ftp_proxy: Option<String>,
    pub no_proxy: Option<String>,
}

async fn migrate_db_if_needed() -> Result<()> {
    let old_path = match config::get_config_dir() {
        Ok(dir) => dir.join("env_state.db"),
        Err(_) => return Ok(()),
    };

    let new_path = match db_file_path() {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };

    if old_path.exists() && !new_path.exists() {
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&old_path, &new_path)?;
    }
    Ok(())
}

pub async fn init_db(db_path: &str) -> Result<()> {
    migrate_db_if_needed().await?;
    let db = Builder::new_local(db_path).build().await?;
    let conn = db.connect()?;
    conn.execute(
        r#"CREATE TABLE IF NOT EXISTS env_state (
            key TEXT PRIMARY KEY,
            value TEXT
        )"#,
        (),
    )
    .await?;
    Ok(())
}

pub async fn save_env_state(db_path: &str, state: &EnvState) -> Result<()> {
    let db = Builder::new_local(db_path).build().await?;
    let conn = db.connect()?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS env_state (key TEXT PRIMARY KEY, value TEXT)",
        (),
    )
    .await?;
    // Clear existing
    conn.execute("DELETE FROM env_state", ()).await?;
    // Insert new
    if let Some(ref v) = state.http_proxy {
        conn.execute(
            "INSERT INTO env_state (key, value) VALUES (?1, ?2)",
            ("http_proxy", v.as_str()),
        )
        .await?;
    }
    if let Some(ref v) = state.https_proxy {
        conn.execute(
            "INSERT INTO env_state (key, value) VALUES (?1, ?2)",
            ("https_proxy", v.as_str()),
        )
        .await?;
    }
    if let Some(ref v) = state.ftp_proxy {
        conn.execute(
            "INSERT INTO env_state (key, value) VALUES (?1, ?2)",
            ("ftp_proxy", v.as_str()),
        )
        .await?;
    }
    if let Some(ref v) = state.no_proxy {
        conn.execute(
            "INSERT INTO env_state (key, value) VALUES (?1, ?2)",
            ("no_proxy", v.as_str()),
        )
        .await?;
    }
    Ok(())
}

pub async fn load_env_state(db_path: &str) -> Result<EnvState> {
    let db = Builder::new_local(db_path).build().await?;
    let conn = db.connect()?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS env_state (key TEXT PRIMARY KEY, value TEXT)",
        (),
    )
    .await?;
    let mut stmt = conn.prepare("SELECT key, value FROM env_state").await?;
    let mut rows = stmt.query(()).await?;
    let mut state = EnvState::default();
    while let Some(row) = rows.next().await? {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        match key.as_str() {
            "http_proxy" => state.http_proxy = Some(value),
            "https_proxy" => state.https_proxy = Some(value),
            "ftp_proxy" => state.ftp_proxy = Some(value),
            "no_proxy" => state.no_proxy = Some(value),
            _ => {}
        }
    }
    Ok(state)
}

fn db_file_path() -> Result<PathBuf> {
    Ok(config::get_data_dir()?.join("env_state.db"))
}

pub fn get_db_path() -> String {
    db_file_path()
        .unwrap_or_else(|_| PathBuf::from("env_state.db"))
        .to_string_lossy()
        .to_string()
}
