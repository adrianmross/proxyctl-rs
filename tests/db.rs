use proxyctl_rs::db;
use tempfile::TempDir;

#[tokio::test]
async fn test_init_db() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db").to_string_lossy().to_string();
    db::init_db(&db_path).await.unwrap();
    // Just check it doesn't error
}

#[tokio::test]
async fn test_save_and_load_env_state() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db").to_string_lossy().to_string();
    db::init_db(&db_path).await.unwrap();

    let state = db::EnvState {
        http_proxy: Some("http://example.com:8080".to_string()),
        https_proxy: Some("http://example.com:8080".to_string()),
        ftp_proxy: None,
        no_proxy: Some("localhost".to_string()),
    };

    db::save_env_state(&db_path, &state).await.unwrap();
    let loaded = db::load_env_state(&db_path).await.unwrap();

    assert_eq!(loaded, state);
}

#[tokio::test]
async fn test_load_empty_db() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db").to_string_lossy().to_string();
    db::init_db(&db_path).await.unwrap();

    let loaded = db::load_env_state(&db_path).await.unwrap();
    let expected = db::EnvState::default();
    assert_eq!(loaded, expected);
}