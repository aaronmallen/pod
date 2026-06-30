#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
// The i18n idiom `t!(...).into_owned()` materializes an owned `String` for UI sinks across the
// localized call sites; clippy's `unnecessary_to_owned` over-flags it against the `Cow` that `t!`
// returns, so it is allowed crate-wide rather than peppering per-call-site attributes.
#![allow(clippy::unnecessary_to_owned)]

#[macro_use]
extern crate rust_i18n;

rust_i18n::i18n!("locales", fallback = "en");

mod app;
mod clients;
mod config;
mod features;
mod services;
mod store;
mod sync;
mod ui;

fn main() -> iced::Result {
  if features::roster::auth::forward_or_claim() {
    std::process::exit(0);
  }
  app::run()
}
