use std::env;
use std::path::PathBuf;

pub mod sled_access;
mod indexing;
pub mod transaction_store;
pub mod adressbook_store;
pub mod xpubpos_store;

/// Default path (*nix)
#[cfg(all(
unix,
not(target_os = "macos"),
not(target_os = "ios"),
not(target_os = "android")
))]
pub fn default_path() -> PathBuf {
    let mut config_dir = env::home_dir().expect("Expect path to home dir");
    config_dir.push(".emerald");
    config_dir.push("state");
    config_dir
}

/// Default path (Mac OS X)
#[cfg(target_os = "macos")]
pub fn default_path() -> PathBuf {
    let mut config_dir = env::home_dir().expect("Expect path to home dir");
    config_dir.push("Library");
    config_dir.push("Emerald");
    config_dir.push("state");
    config_dir
}

/// Default path (Windows OS)
#[cfg(target_os = "windows")]
pub fn default_path() -> PathBuf {
    let app_data_var = env::var("APPDATA").expect("Expect 'APPDATA' environment variable");
    let mut config_dir = PathBuf::from(app_data_var);
    config_dir.push(".emerald");
    config_dir.push("state");
    config_dir
}