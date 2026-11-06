pub mod agent_sessions;
pub mod berth;
mod child;
mod commands;
pub mod connectors;
#[cfg(feature = "desktop")]
mod desktop;
pub mod files;
pub mod remote;
mod review;
pub mod rpc;
pub mod settings;
pub mod sink;
pub mod stratum;
#[cfg(feature = "desktop")]
mod tray;

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    desktop::run();
}
