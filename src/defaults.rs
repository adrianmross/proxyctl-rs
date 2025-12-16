use std::env;

/// Get default domains to exclude from proxy (comma-separated)
/// Loads from DEFAULT_NO_PROXY environment variable if set, otherwise uses generic defaults
pub fn default_no_proxy() -> String {
    env::var("DEFAULT_NO_PROXY").unwrap_or_else(|_| "localhost,127.0.0.1".to_string())
}

/// Get default WPAD URL for proxy discovery
/// Loads from DEFAULT_WPAD_URL environment variable if set, otherwise uses generic default
pub fn default_wpad_url() -> String {
    env::var("DEFAULT_WPAD_URL").unwrap_or_else(|_| "http://wpad.local/wpad.dat".to_string())
}
