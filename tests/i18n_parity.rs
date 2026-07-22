// i18n locale-parity CI check (gest task xsokswzk).
//
// Run locally with `mise run lint:i18n` or `cargo test --test i18n_parity`; it also runs in CI as part
// of the `test` job (`cargo nextest run` / `mise run test` pick up every integration test under `tests/`).
//
// This is a standalone integration-test crate: it reads `assets/locales/*.toml` straight off disk and parses them
// with the `toml` crate, so it does NOT link the `pod` binary crate (which exposes no lib). The check enforces
// i18n drift policy across the nine locales (en is the fallback baseline):
//
//   * missing keys   — an en key absent from a non-en locale -> reported, but only RED once that locale has
//                      authored at least one feature fragment. A locale with zero fragments yet (the current
//                      en-only state) is "not translated yet" and is skipped, so CI is not red just because
//                      machine translation has not run.
//   * orphan keys    — a key in a non-en locale that does not exist in en -> always RED (stale key).
//   * placeholder    — a translated value whose `%{...}` token set differs from en -> always RED.
//   * en-us == en    — en-us must mirror en exactly, enforced once en-us has fragments.
//   * referenced key — every `t!("literal")` and `tr_static("literal")` key referenced in `src/` must exist
//                      in the en baseline -> RED.
//
// Each locale is a single consolidated `<locale>.toml` file with every feature's strings merged in.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
};

const BASELINE_LOCALE: &str = "en";
const LOCALES: [&str; 9] = ["en", "en-us", "de", "es", "fr", "ja", "ko", "ru", "zh"];

type KeyMap = BTreeMap<String, String>;

fn locales_dir() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/locales")
}

fn src_dir() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

// Parse the locale code out of a `<locale>.toml` filename (one consolidated file per locale).
fn locale_of(file_name: &str) -> Option<String> {
  let stem = file_name.strip_suffix(".toml")?;
  Some(stem.to_string())
}

// Flatten a parsed TOML table into dot-separated leaf keys -> string values.
fn flatten(prefix: &str, value: &toml::Value, out: &mut KeyMap) {
  match value {
    toml::Value::Table(table) => {
      for (key, child) in table {
        let next = if prefix.is_empty() {
          key.clone()
        } else {
          format!("{prefix}.{key}")
        };
        flatten(&next, child, out);
      }
    }
    toml::Value::String(s) => {
      out.insert(prefix.to_string(), s.clone());
    }
    other => {
      out.insert(prefix.to_string(), other.to_string());
    }
  }
}

// Load a locale's consolidated `<locale>.toml` into one flat key map (mirrors rust-i18n's per-locale merge).
fn load_locale(locale: &str) -> KeyMap {
  let mut merged = KeyMap::new();
  let dir = locales_dir();
  let entries = fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));

  for entry in entries {
    let entry = entry.expect("dir entry");
    let name = entry.file_name().to_string_lossy().into_owned();
    if locale_of(&name).as_deref() != Some(locale) {
      continue;
    }
    let raw = fs::read_to_string(entry.path()).unwrap_or_else(|e| panic!("read {name}: {e}"));
    let parsed: toml::Value = toml::from_str(&raw).unwrap_or_else(|e| panic!("parse {name}: {e}"));
    flatten("", &parsed, &mut merged);
  }

  merged
}

// Collect the `%{name}` placeholder tokens present in a value.
fn placeholders(value: &str) -> BTreeSet<String> {
  let mut tokens = BTreeSet::new();
  let bytes = value.as_bytes();
  let mut i = 0;
  while i + 1 < bytes.len() {
    if bytes[i] == b'%'
      && bytes[i + 1] == b'{'
      && let Some(end) = value[i + 2..].find('}')
    {
      tokens.insert(value[i + 2..i + 2 + end].trim().to_string());
      i += 2 + end + 1;
      continue;
    }
    i += 1;
  }
  tokens
}

// Recursively collect `*.rs` files under `src/`.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
  let entries = match fs::read_dir(dir) {
    Ok(e) => e,
    Err(_) => return,
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      rust_files(&path, out);
    } else if path.extension().is_some_and(|ext| ext == "rs") {
      out.push(path);
    }
  }
}

fn is_ident_byte(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

// Collect the leading string-literal argument of every standalone `<needle>` call site in `src` into `keys`.
// The needle is a call/macro prefix like `t!(` or `tr_static(`. The standalone-ident guard rejects the tail
// of a longer ident (`format!`, `other_tr_static(`) but accepts scope-qualified forms such as
// `super::i18n::tr_static(`, where the preceding `:` is not an ident byte.
fn collect_literal_keys(src: &str, needle: &str, keys: &mut BTreeSet<String>) {
  let bytes = src.as_bytes();
  let mut search = 0;
  while let Some(rel) = src[search..].find(needle) {
    let start = search + rel;
    let open = start + needle.len();
    let standalone = start == 0 || !is_ident_byte(bytes[start - 1]);
    if standalone {
      // Skip whitespace, then require a string literal start to count it as a static key.
      let mut j = open;
      while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\n' || bytes[j] == b'\t') {
        j += 1;
      }
      if j < bytes.len()
        && bytes[j] == b'"'
        && let Some(end) = src[j + 1..].find('"')
      {
        keys.insert(src[j + 1..j + 1 + end].to_string());
      }
    }
    search = open;
  }
}

// Extract every static `t!("literal", ...)` and `tr_static("literal")` key referenced in the source tree.
fn referenced_keys() -> BTreeSet<String> {
  let mut files = Vec::new();
  rust_files(&src_dir(), &mut files);

  let mut keys = BTreeSet::new();
  for file in files {
    let mut src = match fs::read_to_string(&file) {
      Ok(s) => s,
      Err(_) => continue,
    };

    // Ignore references inside the file's `#[cfg(test)]` module: test code legitimately probes
    // absent keys (the fallback-to-key behavior), which are not real UI references.
    if let Some(cut) = src.find("#[cfg(test)]") {
      src.truncate(cut);
    }

    collect_literal_keys(&src, "t!(", &mut keys);
    collect_literal_keys(&src, "tr_static(", &mut keys);
  }
  keys
}

#[test]
fn every_declared_locale_has_a_file() {
  let dir = locales_dir();
  let mut missing = Vec::new();

  for locale in LOCALES {
    if !dir.join(format!("{locale}.toml")).exists() {
      missing.push(locale);
    }
  }

  assert!(
    missing.is_empty(),
    "locales missing a consolidated `<locale>.toml` file: {missing:?}"
  );
}

#[test]
fn referenced_keys_exist_in_the_en_baseline() {
  let baseline = load_locale(BASELINE_LOCALE);
  let referenced = referenced_keys();

  let mut unknown: Vec<String> = referenced
    .iter()
    .filter(|k| !baseline.contains_key(*k))
    .cloned()
    .collect();
  unknown.sort();

  assert!(
    unknown.is_empty(),
    "`t!(\"...\")` / `tr_static(\"...\")` call sites reference keys absent from the en baseline (assets/locales/en.toml):\n  {}",
    unknown.join("\n  ")
  );
}

#[test]
fn non_en_locales_have_no_orphan_keys() {
  let baseline = load_locale(BASELINE_LOCALE);
  let mut report = Vec::new();

  for locale in LOCALES {
    if locale == BASELINE_LOCALE {
      continue;
    }
    let keys = load_locale(locale);
    let mut orphans: Vec<&String> = keys.keys().filter(|k| !baseline.contains_key(*k)).collect();
    orphans.sort();
    for key in orphans {
      report.push(format!("{locale}: {key}"));
    }
  }

  assert!(
    report.is_empty(),
    "non-en locales contain orphan keys not present in en (stale, remove or add to en):\n  {}",
    report.join("\n  ")
  );
}

#[test]
fn translated_values_preserve_en_placeholders() {
  let baseline = load_locale(BASELINE_LOCALE);
  let mut report = Vec::new();

  for locale in LOCALES {
    if locale == BASELINE_LOCALE {
      continue;
    }
    let keys = load_locale(locale);
    for (key, value) in &keys {
      let Some(en_value) = baseline.get(key) else {
        continue; // orphan keys are reported by the orphan test
      };
      let expected = placeholders(en_value);
      let actual = placeholders(value);
      if expected != actual {
        report.push(format!("{locale}: {key} (en={expected:?}, {locale}={actual:?})"));
      }
    }
  }

  assert!(
    report.is_empty(),
    "translated values dropped or altered %{{...}} placeholders present in en:\n  {}",
    report.join("\n  ")
  );
}

#[test]
fn translated_locales_cover_every_en_key() {
  let baseline = load_locale(BASELINE_LOCALE);
  let mut report = Vec::new();
  let mut skipped = Vec::new();

  for locale in LOCALES {
    if locale == BASELINE_LOCALE {
      continue;
    }
    let keys = load_locale(locale);

    // A locale with zero authored fragments is "not translated yet" -> skip, do not fail CI.
    if keys.is_empty() {
      skipped.push(locale);
      continue;
    }

    let mut missing: Vec<&String> = baseline.keys().filter(|k| !keys.contains_key(*k)).collect();
    missing.sort();
    for key in missing {
      report.push(format!("{locale}: {key}"));
    }
  }

  if !skipped.is_empty() {
    eprintln!("i18n: locales not translated yet (skipped, no fragments authored): {skipped:?}");
  }

  assert!(
    report.is_empty(),
    "translated locales are missing keys present in the en baseline:\n  {}",
    report.join("\n  ")
  );
}

#[test]
fn en_us_mirrors_en_exactly_once_authored() {
  let baseline = load_locale(BASELINE_LOCALE);
  let en_us = load_locale("en-us");

  // en-us begins empty (a copy of en is authored at the translation capstone); only enforce once it exists.
  if en_us.is_empty() {
    eprintln!("i18n: en-us has no fragments yet (skipped exact-mirror check)");
    return;
  }

  let mut missing: Vec<&String> = baseline.keys().filter(|k| !en_us.contains_key(*k)).collect();
  let mut extra: Vec<&String> = en_us.keys().filter(|k| !baseline.contains_key(*k)).collect();
  missing.sort();
  extra.sort();

  assert!(
    missing.is_empty() && extra.is_empty(),
    "en-us must mirror en exactly. missing from en-us: {missing:?}; not in en: {extra:?}"
  );
}
