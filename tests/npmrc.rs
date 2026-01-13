use proxyctl_rs::{config, npmrc};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct TestEnv {
    _lock: std::sync::MutexGuard<'static, ()>,
    _temp: TempDir,
    npmrc_dir: PathBuf,
    active_path: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let lock = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().expect("tempdir");
        let home_dir = temp.path().join("home");
        let config_dir = home_dir.join(".config").join("proxyctl-rs");
        let data_dir = home_dir.join(".local").join("share").join("proxyctl-rs");
        let npmrc_dir = home_dir.join(".npmrcs");
        let active_path = home_dir.join(".npmrc");

        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&npmrc_dir).expect("npmrc dir");

        std::env::set_var("HOME", &home_dir);
        std::env::set_var("XDG_CONFIG_HOME", home_dir.join(".config"));
        std::env::set_var("XDG_DATA_HOME", home_dir.join(".local").join("share"));

        let mut config = config::AppConfig::default();
        config.npmrc.enabled = true;
        config.npmrc.directory = Some(npmrc_dir.to_string_lossy().into_owned());
        config.npmrc.active_path = Some(active_path.to_string_lossy().into_owned());
        config.npmrc.default_profile = Some("default".to_string());
        config.npmrc.proxy_profile = Some("proxy".to_string());
        config::save_config(&config).expect("save config");

        std::fs::write(npmrc_dir.join("default"), "registry=https://default").expect("default");
        std::fs::write(npmrc_dir.join("proxy"), "registry=https://proxy").expect("proxy");

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
    let env = TestEnv::new();

    npmrc::activate_proxy_profile().expect("activate proxy profile");
    let link_target = std::fs::read_link(&env.active_path).expect("read link");
    assert_eq!(link_target, env.npmrc_dir.join("proxy"));

    npmrc::restore_default_profile().expect("restore default profile");
    let link_target = std::fs::read_link(&env.active_path).expect("read link default");
    assert_eq!(link_target, env.npmrc_dir.join("default"));
}
