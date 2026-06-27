#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[macro_use]
extern crate rust_i18n;

rust_i18n::i18n!("locales", fallback = "en");

mod app;
mod clients;
mod config;
mod features;
mod i18n;
mod mcp;
mod services;
mod store;
mod sync;
mod telemetry;
mod telemetry_contract;
mod ui;

fn main() -> iced::Result {
  if features::roster::auth::forward_or_claim() {
    std::process::exit(0);
  }
  app::run()
}
