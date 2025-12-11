use proxyctl_rs::{config, doctor};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

struct EnvGuard {
    entries: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn set<I, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (&'static str, V)>,
        V: Into<String>,
    {
        let entries = vars
            .into_iter()
            .map(|(key, value)| {
                let previous = std::env::var(key).ok();
                std::env::set_var(key, value.into());
                (key, previous)
            })
            .collect();
        Self { entries }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, previous) in self.entries.drain(..) {
            if let Some(value) = previous {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

struct TestEnv {
    _dir: TempDir,
    _env_guard: EnvGuard,
    _lock: MutexGuard<'static, ()>,
}

impl TestEnv {
    fn new() -> Self {
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let dir = tempfile::tempdir().expect("temporary env root");
        let config_dir = dir.path().join("config");
        let data_dir = dir.path().join("data");
        let home_dir = dir.path().join("home");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&home_dir).expect("home dir");

        let env_guard = EnvGuard::set([
            ("XDG_CONFIG_HOME", config_dir.to_string_lossy().into_owned()),
            ("XDG_DATA_HOME", data_dir.to_string_lossy().into_owned()),
            ("HOME", home_dir.to_string_lossy().into_owned()),
            ("SHELL", "/bin/false".to_string()),
        ]);

        Self {
            _dir: dir,
            _env_guard: env_guard,
            _lock: lock,
        }
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test]
async fn test_doctor_reports_success() {
    let _env = TestEnv::new();
    config::initialize_config().unwrap();

    doctor::run().await.unwrap();
}

#[tokio::test]
async fn test_doctor_reports_missing_hosts() {
    let _env = TestEnv::new();
    config::initialize_config().unwrap();

    let hosts_path = config::get_hosts_file_path().unwrap();
    std::fs::remove_file(&hosts_path).unwrap();

    let result = doctor::run().await;
    assert!(result.is_err());
}
