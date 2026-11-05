mod commands;
pub mod connectors;
mod desktop;
pub mod files;
pub mod pacer;
pub mod remote;
pub mod rpc;
pub mod settings;
pub mod sink;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    desktop::run();
}
