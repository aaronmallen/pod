//! Pure PII scrubbing for crash content and buffered log lines.
//!
//! Crash traces and log lines routinely carry character ids, URLs, filesystem
//! paths, and names. Nothing in this module performs I/O: every function maps a
//! `&str` / JSON value to a scrubbed owned value, so it can be unit-tested
//! adversarially against real log lines and reused at both ingest time (the
//! `context_log` ring buffer) and panic time (crash message / location /
//! backtrace).
//!
//! The boundary enforced here is the spec's never-collected list (see spec
//! `mmmzstpq` §5.4/§5.5): paths are stripped to the app root, home directories
//! collapse to `<HOME>`, cargo-registry frames collapse to a crate path,
//! id-keyword values and any long digit run are redacted, URLs keep only
//! scheme + host, and oversized payloads are truncated rather than rejected.

const MESSAGE_CAP_BYTES: usize = 2 * 1024;

const FRAME_CAP_BYTES: usize = 1024;

const LOG_LINE_CAP_BYTES: usize = 1024;

const MAX_LOG_LINES: usize = 20;

/// Placeholder substituted for a redacted id value.
const ID_TOKEN: &str = "<id>";

/// Placeholder substituted for a surviving home-directory prefix.
const HOME_TOKEN: &str = "<HOME>";

/// Id keywords whose immediately-following numeric value is redacted to
/// [`ID_TOKEN`]. EVE ids are large, but small ids (e.g. `race_id`) must be
/// caught too, so this keyword rule does not depend on digit count.
const ID_KEYWORDS: &[&str] = &[
  "character_id",
  "corporation_id",
  "alliance_id",
  "owner_id",
  "station_id",
  "structure_id",
  "location_id",
  "mail_id",
  "plan_id",
  "type_id",
  "race_id",
];

/// `context_log` target allow-list. A buffered line survives only if its
/// `target` field is exactly one of these benign targets; everything else —
/// including the default module-path targets that carry PII such as
/// `pod::features::roster::auth`, `pod::lifecycle`, and `pod::sync::*` — is dropped.
const ALLOWED_TARGETS: &[&str] = &[
  "pod::nav",
  "pod::ui",
  "pod::graphics",
  "pod::updater",
  "pod::telemetry",
  "pod::images",
  "pod::sde",
];

/// `context_log` field allow-list. Every other field on a surviving line is
/// dropped by default (notably `name`, `query`, `hostname`, any `*_id`,
/// `subject`, `item`).
const ALLOWED_FIELDS: &[&str] = &["level", "target", "timestamp", "message", "msg"];

/// Scrub a free-text crash message: apply the full §5.4 path/id/URL ruleset,
/// then truncate to [`MESSAGE_CAP_BYTES`].
pub fn scrub_message(input: &str) -> String {
  truncate_bytes(&scrub_text(input), MESSAGE_CAP_BYTES)
}

/// Scrub a crash `location` (e.g. `src/features/wallet.rs:412`). Treated as
/// text (path-stripped, home-collapsed, digit-guarded) and capped like a frame.
pub fn scrub_location(input: &str) -> String {
  truncate_bytes(&scrub_text(input), FRAME_CAP_BYTES)
}

/// Scrub a single backtrace frame: collapse cargo-registry / sysroot frames to
/// a crate path, apply the §5.4 ruleset, then truncate to [`FRAME_CAP_BYTES`].
pub fn scrub_frame(input: &str) -> String {
  truncate_bytes(&scrub_text(&collapse_registry_frame(input)), FRAME_CAP_BYTES)
}

/// Scrub a whole backtrace, frame by frame.
pub fn scrub_backtrace(frames: &[String]) -> Vec<String> {
  frames.iter().map(|f| scrub_frame(f)).collect()
}

/// Scrub the JSON-structured `context_log` ring buffer.
///
/// Each input element is a JSON object emitted by the `fmt().json()` tracing
/// layer. A line is kept only if its `target` is allow-listed; surviving lines
/// are reduced to the field allow-list, their message is scrubbed through the
/// §5.4 ruleset, each retained line is truncated to [`LOG_LINE_CAP_BYTES`], and
/// the buffer is capped at [`MAX_LOG_LINES`] lines (newest-wins).
///
/// Returns the re-serialized JSON strings that get buffered.
pub fn scrub_context_log(lines: &[String]) -> Vec<String> {
  let mut kept: Vec<String> = lines.iter().filter_map(|raw| scrub_log_line(raw)).collect();

  // Newest-wins: keep the last MAX_LOG_LINES.
  if kept.len() > MAX_LOG_LINES {
    kept.drain(0..kept.len() - MAX_LOG_LINES);
  }
  kept
}

fn scrub_log_line(raw: &str) -> Option<String> {
  let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
  let object = value.as_object()?;

  let target = object.get("target").and_then(|t| t.as_str()).unwrap_or_default();
  if !ALLOWED_TARGETS.contains(&target) {
    return None;
  }

  let mut scrubbed = serde_json::Map::new();
  for (key, val) in object {
    if let Some(field) = scrub_log_field(key, val) {
      scrubbed.insert(key.clone(), field);
    }
  }

  let serialized = serde_json::to_string(&serde_json::Value::Object(scrubbed)).unwrap_or_default();
  Some(truncate_bytes(&serialized, LOG_LINE_CAP_BYTES))
}

fn scrub_log_field(key: &str, val: &serde_json::Value) -> Option<serde_json::Value> {
  if !ALLOWED_FIELDS.contains(&key) {
    return None;
  }
  if (key == "message" || key == "msg")
    && let Some(text) = val.as_str()
  {
    Some(serde_json::Value::String(scrub_text(text)))
  } else {
    Some(val.clone())
  }
}

/// Apply the §5.4 text ruleset (everything except the per-target truncation).
///
/// Order matters: paths/home tokens are normalized first (so a redacted digit
/// run cannot mask a `/pod/` boundary), then URLs are reduced to scheme+host,
/// then id-keyword values are redacted, then any remaining long digit run is
/// blanket-redacted.
fn scrub_text(input: &str) -> String {
  let stripped = strip_app_root(input);
  let homed = replace_home_tokens(&stripped);
  let urled = reduce_urls(&homed);
  let id_keyed = redact_id_keywords(&urled);
  redact_long_digit_runs(&id_keyed)
}

/// §5.4.1 — strip any absolute build-root prefix down to `src/`: drop
/// everything up to and including the last `/pod/`, so an absolute build path
/// collapses to a repo-relative one. Handles both `/` and `\` separators.
fn strip_app_root(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut i = 0;

  while i < bytes.len() {
    // Find the start of a path-like token (no whitespace), scan it, and if it
    // contains a `/pod/` (or `\pod\`) boundary, emit only the tail.
    if bytes[i].is_ascii_whitespace() {
      out.push(bytes[i] as char);
      i += 1;
      continue;
    }
    let start = i;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    let token = &input[start..i];
    out.push_str(&strip_pod_prefix(token));
  }
  out
}

/// Collapse a single whitespace-free token to its tail after the last `/pod/`
/// or `\pod\` boundary, leaving non-path tokens untouched.
fn strip_pod_prefix(token: &str) -> String {
  for marker in ["/dev.aaronmallen.pod/", "\\dev.aaronmallen.pod\\", "/pod/", "\\pod\\"] {
    if let Some(pos) = token.rfind(marker) {
      // Keep everything after the marker, normalizing to forward slashes only
      // for the surviving repo-relative portion is unnecessary; preserve as-is.
      return token[pos + marker.len()..].to_owned();
    }
  }
  token.to_owned()
}

/// §5.4.2 — replace any surviving home-directory prefix with `<HOME>/`.
fn replace_home_tokens(input: &str) -> String {
  let mut out = input.to_owned();

  // Unix: /Users/<name>/ and /home/<name>/
  for root in ["/Users/", "/home/"] {
    out = replace_home_unix(&out, root);
  }
  // Windows: C:\Users\<name>\ (any drive letter).
  out = replace_home_windows(&out);
  out
}

/// Replace `<root><name>/` (e.g. `/Users/aaron/`) with `<HOME>/`, for every
/// occurrence, where `<name>` is the single path segment after `root`.
fn replace_home_unix(input: &str, root: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let mut rest = input;

  while let Some(pos) = rest.find(root) {
    out.push_str(&rest[..pos]);
    let after = &rest[pos + root.len()..];
    // The home directory is the next segment up to the following separator.
    if let Some(slash) = after.find('/') {
      out.push_str(HOME_TOKEN);
      out.push('/');
      rest = &after[slash + 1..];
    } else {
      // No trailing slash: collapse the whole `root<name>` to `<HOME>/`.
      out.push_str(HOME_TOKEN);
      out.push('/');
      rest = "";
    }
  }
  out.push_str(rest);
  out
}

/// Replace `<drive>:\Users\<name>\` with `<HOME>\`, for every occurrence.
fn replace_home_windows(input: &str) -> String {
  let needle = ":\\Users\\";
  let mut out = String::with_capacity(input.len());
  let mut rest = input;

  while let Some(pos) = rest.find(needle) {
    // Step back one char for the drive letter, if present.
    let prefix_end = pos;
    let drive_start = rest[..prefix_end]
      .char_indices()
      .next_back()
      .filter(|(_, c)| c.is_ascii_alphabetic())
      .map_or(prefix_end, |(idx, _)| idx);
    out.push_str(&rest[..drive_start]);

    let after = &rest[pos + needle.len()..];
    if let Some(slash) = after.find('\\') {
      out.push_str(HOME_TOKEN);
      out.push('\\');
      rest = &after[slash + 1..];
    } else {
      out.push_str(HOME_TOKEN);
      out.push('\\');
      rest = "";
    }
  }
  out.push_str(rest);
  out
}

/// §5.4.3 — collapse a cargo-registry / rustc-sysroot frame to a crate path:
/// `.../registry/src/index.crates.io-.../iced_runtime-0.13/src/program.rs` →
/// `iced_runtime::program`. Non-registry frames are returned untouched.
fn collapse_registry_frame(input: &str) -> String {
  // Normalize separators for detection only.
  let normalized = input.replace('\\', "/");
  let markers = ["/registry/src/", "/.cargo/registry/src/", "/rustlib/src/rust/library/"];

  let Some(rest) = markers
    .iter()
    .find_map(|m| normalized.find(m).map(|pos| &normalized[pos + m.len()..]))
  else {
    return input.to_owned();
  };

  // The first segment after the marker is `index.crates.io-<hash>` (cargo) or a
  // crate dir directly (sysroot). Skip a leading index segment if present.
  let segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
  if segments.is_empty() {
    return input.to_owned();
  }
  let mut start = 0;
  if segments[0].starts_with("index.crates.io") {
    start = 1;
  }
  let path_segments = &segments[start..];
  if path_segments.is_empty() {
    return input.to_owned();
  }

  // First segment is the crate dir `crate_name-x.y.z`; strip the version.
  let crate_name = path_segments[0]
    .rsplit_once('-')
    .map_or(path_segments[0], |(name, _ver)| name);

  // Remaining segments form the module path; drop a leading `src`, any trailing
  // `:line:col`, and the `.rs` extension on each segment.
  let modules: Vec<String> = path_segments[1..]
    .iter()
    .map(|s| {
      let no_loc = s.split(':').next().unwrap_or(s);
      no_loc.trim_end_matches(".rs").to_owned()
    })
    // `src` and `mod` are structural, not module-name, segments.
    .filter(|s| s != "src" && s != "mod")
    .collect();

  let mut joined = crate_name.to_owned();
  if !modules.is_empty() {
    joined.push_str("::");
    joined.push_str(&modules.join("::"));
  }
  joined
}

/// §5.4.5 — reduce any `http://` / `https://` URL to scheme + host, dropping
/// the path, query, and fragment.
fn reduce_urls(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let mut rest = input;

  loop {
    let next = ["https://", "http://"]
      .iter()
      .filter_map(|scheme| rest.find(scheme).map(|pos| (pos, *scheme)))
      .min_by_key(|(pos, _)| *pos);

    let Some((pos, scheme)) = next else {
      out.push_str(rest);
      break;
    };

    out.push_str(&rest[..pos]);
    let after = &rest[pos + scheme.len()..];
    // Host runs until the first path/query/fragment/whitespace/quote delimiter.
    let host_len = after
      .find(|c: char| matches!(c, '/' | '?' | '#') || c.is_whitespace() || c == '"' || c == '\'' || c == ')')
      .unwrap_or(after.len());
    out.push_str(scheme);
    out.push_str(&after[..host_len]);

    // Discard the URL's path/query/fragment: skip everything from the host's
    // end until a true URL terminator (whitespace / quote / paren / comma), so
    // the dropped path can never be re-emitted by a later iteration.
    let tail = &after[host_len..];
    let stop = tail
      .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | ','))
      .unwrap_or(tail.len());
    rest = &tail[stop..];
  }
  out
}

/// §5.4.4 — replace the numeric value immediately following an id keyword with
/// [`ID_TOKEN`]. Matches `character_id=123`, `character_id: 123`,
/// `character_id 123`, and `"character_id":123` shapes; the value may include
/// digits and an optional sign.
fn redact_id_keywords(input: &str) -> String {
  let mut out = input.to_owned();
  for keyword in ID_KEYWORDS {
    out = redact_keyword(&out, keyword);
  }
  out
}

/// Redact the numeric value following every occurrence of `keyword`.
fn redact_keyword(input: &str, keyword: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut i = 0;

  while i < input.len() {
    if let Some((value_start, value_end)) = keyword_value_span(input, bytes, keyword, i) {
      let kw_end = i + keyword.len();
      out.push_str(keyword);
      out.push_str(&input[kw_end..value_start]);
      out.push_str(ID_TOKEN);
      i = value_end;
    } else {
      let ch_len = utf8_char_len(bytes[i]);
      out.push_str(&input[i..i + ch_len]);
      i += ch_len;
    }
  }
  out
}

fn keyword_value_span(input: &str, bytes: &[u8], keyword: &str, i: usize) -> Option<(usize, usize)> {
  if !input[i..].starts_with(keyword) || !is_keyword_boundary(bytes, i) {
    return None;
  }

  let value_start = skip_value_separators(bytes, i + keyword.len());
  let mut j = value_start;
  if j < bytes.len() && (bytes[j] == b'-' || bytes[j] == b'+') {
    j += 1;
  }
  let digit_start = j;
  while j < bytes.len() && bytes[j].is_ascii_digit() {
    j += 1;
  }

  (j > digit_start).then_some((value_start, j))
}

fn skip_value_separators(bytes: &[u8], from: usize) -> usize {
  let mut j = from;
  while j < bytes.len() && matches!(bytes[j], b'=' | b':' | b' ' | b'\t' | b'"' | b'\'') {
    j += 1;
  }
  j
}

/// True when a keyword occurrence is a whole word (not a suffix of a longer
/// identifier such as `xcharacter_id`).
fn is_keyword_boundary(bytes: &[u8], start: usize) -> bool {
  if start == 0 {
    return true;
  }
  let prev = bytes[start - 1];
  // A preceding identifier char means this is part of a larger word.
  !(prev.is_ascii_alphanumeric() || prev == b'_')
}

/// §5.4.6 — redact any run of 7 or more consecutive digits anywhere.
fn redact_long_digit_runs(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut i = 0;

  while i < bytes.len() {
    if bytes[i].is_ascii_digit() {
      let start = i;
      while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
      }
      if i - start >= 7 {
        out.push_str(ID_TOKEN);
      } else {
        out.push_str(&input[start..i]);
      }
    } else {
      let ch_len = utf8_char_len(bytes[i]);
      out.push_str(&input[i..i + ch_len]);
      i += ch_len;
    }
  }
  out
}

/// Width, in bytes, of a UTF-8 code point given its lead byte.
fn utf8_char_len(lead: u8) -> usize {
  match lead {
    0x00..=0x7F => 1,
    0xC0..=0xDF => 2,
    0xE0..=0xEF => 3,
    _ => 4,
  }
}

/// Truncate a string to at most `cap` bytes, never splitting a UTF-8 boundary.
fn truncate_bytes(input: &str, cap: usize) -> String {
  if input.len() <= cap {
    return input.to_owned();
  }
  let mut end = cap;
  while end > 0 && !input.is_char_boundary(end) {
    end -= 1;
  }
  input[..end].to_owned()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn strips_absolute_build_root_to_src() {
    let input = "/Users/aaron/src/github.com/aaronmallen/pod/src/features/wallet.rs:412";
    assert_eq!(scrub_location(input), "src/features/wallet.rs:412");
  }

  #[test]
  fn collapses_home_token_unix_and_mac() {
    assert_eq!(scrub_text("at /Users/aaron/.cargo/config"), "at <HOME>/.cargo/config");
    assert_eq!(scrub_text("at /home/aaron/.config/pod"), "at <HOME>/.config/pod");
  }

  #[test]
  fn collapses_home_token_windows() {
    let scrubbed = scrub_text("C:\\Users\\Aaron\\AppData\\pod.log");
    assert!(scrubbed.contains("<HOME>\\AppData"), "got {scrubbed}");
    assert!(!scrubbed.contains("Aaron"), "windows home leaked: {scrubbed}");
  }

  #[test]
  fn collapses_cargo_registry_frame_to_crate_path() {
    let frame =
      "/Users/aaron/.cargo/registry/src/index.crates.io-6f17d22bba15001f/iced_runtime-0.13/src/program.rs:118";
    assert_eq!(scrub_frame(frame), "iced_runtime::program");
  }

  #[test]
  fn reduces_url_to_scheme_and_host() {
    let input = "GET https://esi.evetech.net/v5/characters/2117209623/wallet/?datasource=tranquility";
    let scrubbed = scrub_text(input);
    assert!(scrubbed.contains("https://esi.evetech.net"), "got {scrubbed}");
    assert!(!scrubbed.contains("/v5/"), "url path leaked: {scrubbed}");
    assert!(!scrubbed.contains("datasource"), "url query leaked: {scrubbed}");
  }

  #[test]
  fn redacts_id_keyword_values_including_small_ids() {
    assert_eq!(scrub_text("character_id=90000001"), "character_id=<id>");
    assert_eq!(scrub_text("race_id: 8"), "race_id: <id>");
    assert_eq!(scrub_text("\"owner_id\":98000123"), "\"owner_id\":<id>");
  }

  #[test]
  fn id_keyword_requires_word_boundary() {
    assert_eq!(scrub_text("xrace_id 8"), "xrace_id 8");
  }

  #[test]
  fn id_keyword_without_numeric_value_is_untouched() {
    assert_eq!(scrub_text("type_id=unknown"), "type_id=unknown");
    assert_eq!(scrub_text("race_id: none"), "race_id: none");
  }

  #[test]
  fn redacts_any_long_digit_run() {
    assert_eq!(scrub_text("dropped frame for 2117209623"), "dropped frame for <id>");
    assert_eq!(scrub_text("retry 3 of 10"), "retry 3 of 10");
  }

  #[test]
  fn truncates_message_to_2kib() {
    let big = "x".repeat(4096);
    assert_eq!(scrub_message(&big).len(), MESSAGE_CAP_BYTES);
  }

  #[test]
  fn truncates_frame_to_1kib() {
    let big = "y".repeat(4096);
    assert_eq!(scrub_frame(&big).len(), FRAME_CAP_BYTES);
  }

  #[test]
  fn keeps_only_allowlisted_targets() {
    let lines = vec![
      "{\"level\":\"INFO\",\"target\":\"pod::nav\",\"message\":\"navigated\"}".to_owned(),
      "{\"level\":\"INFO\",\"target\":\"pod::features::roster::auth\",\"message\":\"x\"}".to_owned(),
      "{\"level\":\"INFO\",\"target\":\"pod::lifecycle\",\"message\":\"x\"}".to_owned(),
    ];
    let kept = scrub_context_log(&lines);
    assert_eq!(kept.len(), 1);
    assert!(kept[0].contains("pod::nav"));
  }

  #[test]
  fn drops_non_allowlisted_fields_from_kept_line() {
    let input = "{\"level\":\"INFO\",\"target\":\"pod::ui\",\"message\":\"ok\",\"name\":\"Pilot\",\"query\":\"jita\",\"hostname\":\"laptop\",\"character_id\":90000001}";
    let kept = scrub_context_log(&[input.to_owned()]);
    assert_eq!(kept.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&kept[0]).unwrap();
    let obj = parsed.as_object().unwrap();
    for banned in ["name", "query", "hostname", "character_id"] {
      assert!(!obj.contains_key(banned), "field {banned} survived: {}", kept[0]);
    }
    assert!(obj.contains_key("level") && obj.contains_key("target") && obj.contains_key("message"));
  }

  #[test]
  fn scrubs_message_of_a_kept_line() {
    let input = "{\"level\":\"INFO\",\"target\":\"pod::sde\",\"message\":\"loaded https://example.com/secret type_id=587 for 2117209623\"}";
    let kept = scrub_context_log(&[input.to_owned()]);
    let parsed: serde_json::Value = serde_json::from_str(&kept[0]).unwrap();
    let msg = parsed["message"].as_str().unwrap();
    assert!(!msg.contains("/secret"), "url path leaked: {msg}");
    assert!(msg.contains("type_id=<id>"), "id keyword survived: {msg}");
    assert!(!msg.contains("2117209623"), "long digit run survived: {msg}");
  }

  #[test]
  fn caps_context_log_at_twenty_lines() {
    let mut lines = Vec::new();
    for i in 0..50 {
      lines.push(format!(
        "{{\"level\":\"INFO\",\"target\":\"pod::ui\",\"message\":\"line {i}\"}}"
      ));
    }
    let kept = scrub_context_log(&lines);
    assert_eq!(kept.len(), MAX_LOG_LINES);
    assert!(kept.last().unwrap().contains("line 49"));
  }

  #[test]
  fn truncates_each_context_log_line_to_1kib() {
    let huge = "z".repeat(4096);
    let input = format!("{{\"level\":\"INFO\",\"target\":\"pod::ui\",\"message\":\"{huge}\"}}");
    let kept = scrub_context_log(&[input]);
    assert!(kept[0].len() <= LOG_LINE_CAP_BYTES);
  }

  #[test]
  fn invalid_json_lines_are_dropped() {
    let kept = scrub_context_log(&[
      "not json".to_owned(),
      "{\"target\":\"pod::ui\",\"message\":\"ok\"}".to_owned(),
    ]);
    assert_eq!(kept.len(), 1);
  }

  #[test]
  fn adversarial_character_name_at_auth_is_dropped() {
    let line = "{\"timestamp\":\"2026-06-25T00:00:00Z\",\"level\":\"INFO\",\"target\":\"pod::features::roster::auth\",\"character_id\":90000001,\"name\":\"Aaron Mallen\",\"message\":\"character signed in\"}";
    let kept = scrub_context_log(&[line.to_owned()]);
    assert!(kept.is_empty(), "auth line survived: {kept:?}");
    assert!(!ALLOWED_TARGETS.contains(&"pod::auth"));
    assert!(!ALLOWED_TARGETS.contains(&"pod::features::roster::auth"));
    let joined = kept.join("");
    assert!(!joined.contains("Aaron"), "character_name leaked");
  }

  #[test]
  fn adversarial_hostname_at_lifecycle_is_dropped() {
    let line = "{\"timestamp\":\"2026-06-25T00:00:00Z\",\"level\":\"WARN\",\"target\":\"pod::lifecycle\",\"hostname\":\"aarons-macbook-pro.local\",\"message\":\"the share is open elsewhere; opening read-only\"}";
    let kept = scrub_context_log(&[line.to_owned()]);
    assert!(kept.is_empty(), "lifecycle hostname line survived: {kept:?}");
    assert!(!kept.join("").contains("macbook"), "hostname leaked");
  }

  #[test]
  fn adversarial_search_query_is_dropped() {
    let line = "{\"timestamp\":\"2026-06-25T00:00:00Z\",\"level\":\"WARN\",\"target\":\"pod::entity_search\",\"query\":\"CCP Falcon\",\"message\":\"entity search failed\"}";
    let kept = scrub_context_log(&[line.to_owned()]);
    assert!(kept.is_empty(), "entity_search query line survived: {kept:?}");
    assert!(!kept.join("").contains("Falcon"), "query leaked");
  }

  #[test]
  fn adversarial_full_esi_url_in_message_reduced_to_host() {
    let msg = "called `Result::unwrap()` on an `Err` value: request to https://esi.evetech.net/latest/characters/2117209623/assets/?page=3&token=secret failed";
    let scrubbed = scrub_message(msg);
    assert!(scrubbed.contains("https://esi.evetech.net"), "host lost: {scrubbed}");
    assert!(!scrubbed.contains("/latest/"), "url path leaked: {scrubbed}");
    assert!(!scrubbed.contains("token=secret"), "url query leaked: {scrubbed}");
    assert!(!scrubbed.contains("2117209623"), "id in url leaked: {scrubbed}");
  }

  #[test]
  fn adversarial_sync_ids_in_message_redacted() {
    let msg = "sync failed owner_id=98000123 structure_id=1035466617946 station_id=60003760 location_id=1000000016 mail_id=438492011";
    let scrubbed = scrub_message(msg);
    for raw in ["98000123", "1035466617946", "60003760", "1000000016", "438492011"] {
      assert!(!scrubbed.contains(raw), "sync id {raw} leaked: {scrubbed}");
    }
    assert_eq!(
      scrubbed,
      "sync failed owner_id=<id> structure_id=<id> station_id=<id> location_id=<id> mail_id=<id>"
    );
  }

  #[test]
  fn adversarial_sync_ids_smuggled_into_allowlisted_message() {
    let line = "{\"timestamp\":\"2026-06-25T00:00:00Z\",\"level\":\"DEBUG\",\"target\":\"pod::sde\",\"message\":\"resolved station_id=60003760 owner_id 98000123\"}";
    let kept = scrub_context_log(&[line.to_owned()]);
    assert_eq!(kept.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&kept[0]).unwrap();
    let msg = parsed["message"].as_str().unwrap();
    assert!(msg.contains("station_id=<id>"), "station_id survived: {msg}");
    assert!(msg.contains("owner_id <id>"), "owner_id survived: {msg}");
    assert!(!msg.contains("60003760") && !msg.contains("98000123"));
  }

  #[test]
  fn adversarial_backtrace_strips_paths_and_collapses_registry() {
    let frames = vec![
      "/Users/aaron/src/github.com/aaronmallen/pod/src/sync/engine.rs:204".to_owned(),
      "/Users/aaron/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.38/src/runtime/task/mod.rs:42"
        .to_owned(),
    ];
    let scrubbed = scrub_backtrace(&frames);
    assert_eq!(scrubbed[0], "src/sync/engine.rs:204");
    assert_eq!(scrubbed[1], "tokio::runtime::task");
    assert!(!scrubbed.join("").contains("aaron"), "home leaked in backtrace");
  }
}
