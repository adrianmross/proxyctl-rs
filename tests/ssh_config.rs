use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use proxyctl_rs::config;

fn proxy_line(proxy_host: &str) -> String {
    format!("ProxyCommand /usr/bin/nc -X connect -x {proxy_host} %h %p")
}

struct SshFixture {
    _lock: MutexGuard<'static, ()>,
    _temp_dir: tempfile::TempDir,
    ssh_config_path: PathBuf,
    hosts_path: PathBuf,
    backup_path: PathBuf,
    _home_guard: EnvGuard,
    _xdg_config_guard: EnvGuard,
    _xdg_data_guard: EnvGuard,
}

impl SshFixture {
    fn new(hosts: &str, ssh_config: &str) -> Self {
        let lock = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let home_dir = temp_dir.path().join("home");
        fs::create_dir_all(&home_dir).expect("create home dir");
        let ssh_dir = home_dir.join(".ssh");
        let config_root = home_dir.join(".config");
        let config_dir = config_root.join("proxyctl-rs");
        let data_root = home_dir.join(".local").join("share");
        let data_dir = data_root.join("proxyctl-rs");

        fs::create_dir_all(&ssh_dir).expect("create .ssh");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::create_dir_all(&data_dir).expect("create data dir");

        let ssh_config_path = ssh_dir.join("config");
        fs::write(&ssh_config_path, ssh_config).expect("write ssh config");

        let config_toml = "default_hosts_file = \"hosts.txt\"\n[proxy_settings]\nenable_http_proxy = true\nenable_https_proxy = true\nenable_ftp_proxy = true\nenable_no_proxy = true\n".to_string();
        fs::write(config_dir.join("config.toml"), config_toml).expect("write config.toml");

        let hosts_path = config_dir.join("hosts.txt");
        fs::write(&hosts_path, hosts).expect("write hosts file");

        let home_guard = EnvGuard::new("HOME", &home_dir);
        let xdg_config_guard = EnvGuard::new("XDG_CONFIG_HOME", &config_root);
        let xdg_data_guard = EnvGuard::new("XDG_DATA_HOME", &data_root);

        let backup_path = ssh_dir.join("config.proxyctl-rs.bak");

        Self {
            _lock: lock,
            _temp_dir: temp_dir,
            ssh_config_path: ssh_config_path.clone(),
            hosts_path,
            backup_path,
            _home_guard: home_guard,
            _xdg_config_guard: xdg_config_guard,
            _xdg_data_guard: xdg_data_guard,
        }
    }

    fn backup_path(&self) -> &Path {
        &self.backup_path
    }

    fn hosts_path(&self) -> &Path {
        &self.hosts_path
    }

    fn read_config(&self) -> String {
        fs::read_to_string(&self.ssh_config_path).expect("read ssh config")
    }
}

struct EnvGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvGuard {
    fn new(key: &'static str, value: &Path) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn ssh_add_adds_proxy_command_for_matching_hosts() {
    let proxy_host = "proxy.example.com:8080";
    let fixture = SshFixture::new(
        "host1.oracle.com\nHOST2.oracle.com\n",
        "Host host1.oracle.com\n    User alice\n\nHost unmatched\n    User bob\n",
    );

    config::add_ssh_hosts(fixture.hosts_path().to_string_lossy().as_ref(), proxy_host)
        .expect("add hosts");

    let updated = fixture.read_config();
    assert!(updated.contains(&proxy_line(proxy_host)));
    assert!(updated.contains("Host host1.oracle.com"));
    assert!(updated.contains("Host unmatched"));

    let backup = fs::read_to_string(fixture.backup_path()).expect("read backup");
    assert_eq!(
        backup,
        "Host host1.oracle.com\n    User alice\n\nHost unmatched\n    User bob\n"
    );
}

#[test]
fn ssh_remove_removes_proxy_command_but_preserves_other_hosts() {
    let proxy_host = "proxy.example.com:8080";
    let proxy_line_with_indent = format!("    {}\n", proxy_line(proxy_host));

    let initial = format!(
        "Host host1.oracle.com\n    User alice\n{proxy_line_with_indent}\nHost host2.oracle.com\n    User bob\n{proxy_line_with_indent}\nHost other\n    User carol\n"
    );

    let fixture = SshFixture::new("host1.oracle.com\nhost2.oracle.com\n", &initial);

    config::remove_ssh_hosts().expect("remove hosts");

    let updated = fixture.read_config();
    assert!(!updated.contains(&proxy_line(proxy_host)));
    assert!(updated.contains("Host other"));
}

#[test]
fn ssh_add_and_remove_are_idempotent() {
    let proxy_host = "proxy.example.com:8080";
    let fixture = SshFixture::new(
        "host1.oracle.com\n",
        "Host host1.oracle.com\n    User alice\n",
    );

    config::add_ssh_hosts(fixture.hosts_path().to_string_lossy().as_ref(), proxy_host)
        .expect("first add");
    let first_config = fixture.read_config();

    // ensure repeated add doesn't duplicate proxy line
    config::add_ssh_hosts(fixture.hosts_path().to_string_lossy().as_ref(), proxy_host)
        .expect("second add");
    let second_config = fixture.read_config();
    assert_eq!(first_config, second_config);

    // ensure remove eliminates proxy line
    config::remove_ssh_hosts().expect("first remove");
    let first_remove = fixture.read_config();
    assert!(!first_remove.contains(&proxy_line(proxy_host)));

    // re-add to confirm remove idempotence
    config::add_ssh_hosts(fixture.hosts_path().to_string_lossy().as_ref(), proxy_host)
        .expect("re-add");
    config::remove_ssh_hosts().expect("second remove");
    let second_remove = fixture.read_config();
    assert_eq!(first_remove, second_remove);
}
