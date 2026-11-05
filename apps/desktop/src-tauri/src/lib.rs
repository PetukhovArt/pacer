mod commands;
pub mod connectors;
#[cfg(feature = "desktop")]
mod desktop;
pub mod files;
pub mod pacer;
pub mod remote;
pub mod rpc;
pub mod settings;
pub mod sink;
#[cfg(feature = "desktop")]
mod tray;

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    desktop::run();
}
