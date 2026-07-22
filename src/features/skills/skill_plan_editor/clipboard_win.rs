//! Windows-specific clipboard reader for EVE skill plan text.
//!
//! EVE's in-game browser on Windows publishes copied skill plans as CF_HTML only — it does not
//! emit a CF_UNICODETEXT companion — so the standard cross-platform clipboard path returns nothing.
//! This module tries CF_UNICODETEXT first (succeeds when the plan was copied from an external tool
//! that does the right thing), then falls back to CF_HTML (EVE's actual format, stripped to plain
//! text via `import_export::html_fragment_to_text`), then to CF_TEXT (ANSI, last resort).

use clipboard_win::{formats, get_clipboard};

use super::import_export;

pub fn read_plan_text() -> Option<String> {
  if let Some(text) = read_unicode() {
    tracing::debug!(target: "pod::skills::import", source = "CF_UNICODETEXT", "clipboard fallback hit");
    return Some(text);
  }
  if let Some(text) = read_html() {
    tracing::debug!(target: "pod::skills::import", source = "CF_HTML", "clipboard fallback hit");
    return Some(text);
  }
  let ansi = read_ansi();
  if ansi.is_some() {
    tracing::debug!(target: "pod::skills::import", source = "CF_TEXT", "clipboard fallback hit");
  } else {
    tracing::debug!(target: "pod::skills::import", "clipboard fallback found no text format");
  }
  ansi
}

fn non_empty(text: String) -> Option<String> {
  (!text.trim().is_empty()).then_some(text)
}

fn read_ansi() -> Option<String> {
  let bytes: Vec<u8> = get_clipboard(formats::RawData(formats::CF_TEXT)).ok()?;
  // CF_TEXT is an ANSI (codepage) string; from_utf8_lossy replaces non-ASCII bytes with U+FFFD.
  // That is safe here because skill plan names and EFT headers are ASCII-only.
  non_empty(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_html() -> Option<String> {
  let html = formats::Html::new()?;
  let raw: String = get_clipboard(html).ok()?;
  non_empty(import_export::html_fragment_to_text(&raw))
}

fn read_unicode() -> Option<String> {
  let text: String = get_clipboard(formats::Unicode).ok()?;
  non_empty(text)
}
