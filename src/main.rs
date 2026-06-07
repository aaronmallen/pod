#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod clients;
mod config;
mod features;
#[allow(dead_code)]
mod services;
mod store;
#[allow(dead_code)]
mod sync;
mod ui;
#[allow(dead_code)]
mod window_state;

fn main() -> iced::Result {
  if features::auth::forward_or_claim() {
    std::process::exit(0);
  }
  app::run()
}
