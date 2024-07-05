use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    config::{Config, ISettings, NVersion},
    node::*,
    ret_err, wrap_err,
};

use anyhow::Result;
use get_node::archive::{fetch_native, FetchConfig};
use tauri::Manager;
use tokio::time::Instant;

type CmdResult<T = ()> = Result<T, String>;

/// get current version
#[tauri::command]
pub fn current() -> CmdResult<Option<String>> {
    wrap_err!(get_current())
}

/// fetch node version list
#[tauri::command]
pub async fn version_list(fetch: Option<bool>) -> CmdResult<Option<Vec<NVersion>>> {
    wrap_err!(get_version_list(fetch).await)
}

/// read node installed version list
#[tauri::command]
pub async fn installed_list(fetch: Option<bool>) -> CmdResult<Option<Vec<String>>> {
    wrap_err!(get_installed_list(fetch).await)
}

/// read settings data
#[tauri::command]
pub async fn read_settings() -> CmdResult<ISettings> {
    Ok(Config::settings().data().clone())
}

/// install node
#[tauri::command]
pub async fn install_node(window: tauri::Window, version: Option<String>) -> CmdResult<String> {
    if version.is_none() {
        ret_err!("version should not be null");
    }

    let version: String = version.unwrap();
    let settings = Config::settings().latest().clone();
    let mirror = settings.mirror.unwrap();
    let directory = settings.directory.unwrap();

    let last_emit_time = Arc::new(Mutex::new(Instant::now()));

    let config = FetchConfig {
        dest: directory,
        mirror: mirror,
        version: version,
        no_proxy: settings.no_proxy,
        proxy: settings.proxy,
        cancel_signal: None,
        timeout: None,
        on_progress: Box::new({
            move |source: &str, transferred: usize, total: usize| {
                let mut last_emit_time = last_emit_time.lock().unwrap();
                let now = Instant::now();
                if now.duration_since(*last_emit_time) >= Duration::from_millis(300) {
                    *last_emit_time = now;
                    println!("Source: {}, Progress: {}/{}", source, transferred, total);
                    let source: String = source.to_string();
                    let _ = window.emit(
                        "on-node-progress",
                        ProgressData {
                            source: &source,
                            transferred,
                            total,
                        },
                    );
                }
            }
        }),
    };

    wrap_err!(fetch_native(config).await)
}

/// exit app
#[tauri::command]
pub fn exit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
    std::process::exit(0);
}
