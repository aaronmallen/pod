#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod clients;
mod config;
mod features;
mod mcp;
mod notifications;
mod services;
mod store;
mod sync;
mod telemetry;
mod telemetry_contract;
mod ui;
mod window_state;

fn main() -> iced::Result {
  if features::auth::forward_or_claim() {
    std::process::exit(0);
  }
  app::run()
}
