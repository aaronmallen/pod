// i18n anti-regression guard: detects user-facing string literals that bypass `t!(...)`.
//
// This is the forward-direction counterpart to `tests/i18n_parity.rs` (which only checks that every
// `t!("key")` referenced in `src/` exists in the locale files). Nothing else catches literals that
// never got wrapped in `t!()`, so untranslated pockets were invisible. This scan closes that gap and
// emits a `file:line` worklist of everything still hard-coded.
//
// DORMANT BY DESIGN. Both tests are `#[ignore]`d so a plain `cargo test` / `mise run test` stays green
// during the migration. Run the detector (and read the worklist) with:
//
//   cargo test --test i18n_no_hardcoded -- --ignored --nocapture
//
// or run a single one:
//
//   cargo test --test i18n_no_hardcoded -- --ignored no_hardcoded_user_facing_strings
//
// The migration's final verification task flips these from `#[ignore]` to live CI gates.
//
// WHAT IT SCANS (user-facing UI contexts holding a *direct* string literal, not a `t!(...)` call):
//   * function calls   — `text("...")`, `set_title("...")`, `placeholder("...")`
//   * struct fields    — `label:`, `title:`, `description:`, `kicker:` (+ sibling copy fields below)
//   * MCP descriptions — the 2nd positional arg of `McpTool::new(name, description, ...)` (LLM-facing,
//                        localized per the spec; explicitly NOT exempt)
// A second test flags duplicated month / weekday name literal arrays so the date-array work is covered.
//
// ALLOWLIST (legitimate permanent exceptions, kept as easily-extended data + predicates):
//   * i18n keys        — a dotted lowercase literal IS already a key reference (e.g. a field that stores
//                        `"settings.telemetry.stream_usage_title"` and resolves it at render).
//   * URLs / endpoints — `http...`, `://`, leading `/`, `www.`, `mailto:`.
//   * SQL fragments    — literals starting with a SQL keyword (covers store/repo/* and asset_filter SQL,
//                        which lives in query strings, not UI contexts).
//   * tracing / log    — lines containing `tracing::`, `log::`, `target:`, or a `info!`/`warn!`/... macro.
//   * doc / line comments — any line whose trimmed start is `//`.
//   * unicode glyphs   — literals with no alphabetic character once `\u{...}`/escapes are stripped
//                        (e.g. `"\u{2192}"`, `"\u{2022}"`, `" / "`).
//   * test modules     — everything guarded by `#[cfg(test)]` (mirrors tests/i18n_parity.rs:141, but
//                        brace-matched per item so mid-file test fns/blocks in src/app.rs are handled,
//                        not just a single truncate point).
//   * code identifiers — ESI notification type IDs, enum-variant strings, and `variant_name()` maps are
//                        structurally excluded: they live in `=> "..."` match arms, never in UI contexts.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
};

// Struct-field names that carry user-facing copy. Easily extended.
const FIELD_CONTEXTS: [&str; 9] = [
  "desc",
  "description",
  "heading",
  "headline",
  "kicker",
  "label",
  "subtitle",
  "title",
  "tooltip",
];

// Function calls whose (first) argument is user-facing copy. Easily extended.
const FN_CONTEXTS: [&str; 3] = ["placeholder", "set_title", "text"];

// Helpers whose SECOND positional argument is user-facing copy (the first is a handle/channel), e.g.
// the splash seeding `step(tx, "Seeding ...")`. Easily extended.
const SECOND_ARG_CONTEXTS: [&str; 1] = ["step"];

// Month and weekday names (full + 3-letter abbreviation) for the date-array detector.
const DATE_LITERALS: [&str; 38] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
  "Sunday",
  "Mon",
  "Tue",
  "Wed",
  "Thu",
  "Fri",
  "Sat",
  "Sun",
  "May",
];

// Leading keywords that mark a literal as a SQL fragment rather than UI copy.
const SQL_PREFIXES: [&str; 18] = [
  "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "WITH ", "CREATE ", "ALTER ", "DROP ", "PRAGMA ", "BEGIN ", "COMMIT",
  "REPLACE ", "VALUES", "FROM ", "WHERE ", "JOIN ", "ORDER BY", "GROUP BY",
];

// Substrings that mark a whole line as logging/tracing (or a doc comment) rather than UI.
const LINE_EXEMPT_MARKERS: [&str; 8] = [
  "tracing::",
  "log::",
  "target:",
  "trace!",
  "debug!",
  "info!",
  "warn!",
  "error!",
];

#[derive(Clone, Debug)]
struct Violation {
  context: String,
  file: String,
  line: usize,
  literal: String,
}

fn src_dir() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

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

// End offset (inclusive) of the item a `#[cfg(test)]` attribute guards, starting the scan just after
// the attribute. Skips string/char/comment content so braces inside test code never throw off depth.
// Returns the index of the closing `}` for a braced item, the `;` for a statement item, or EOF.
fn item_end(bytes: &[u8], from: usize) -> usize {
  let mut i = from;
  let mut depth: usize = 0;
  let mut started = false;
  while i < bytes.len() {
    match bytes[i] {
      b'/' if bytes.get(i + 1) == Some(&b'/') => {
        while i < bytes.len() && bytes[i] != b'\n' {
          i += 1;
        }
        continue;
      }
      b'/' if bytes.get(i + 1) == Some(&b'*') => {
        i += 2;
        while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
          i += 1;
        }
        i += 2;
        continue;
      }
      b'r' => {
        // Raw string `r#"..."#` only when `r` is a standalone token followed by `#`*`"`.
        let standalone = i == 0 || !is_ident_byte(bytes[i - 1]);
        let mut j = i + 1;
        let mut hashes = 0;
        while j < bytes.len() && bytes[j] == b'#' {
          hashes += 1;
          j += 1;
        }
        if standalone && j < bytes.len() && bytes[j] == b'"' {
          i = j + 1;
          let close = format!("\"{}", "#".repeat(hashes));
          let cb = close.as_bytes();
          while i < bytes.len() && !bytes[i..].starts_with(cb) {
            i += 1;
          }
          i += cb.len();
          continue;
        }
        i += 1;
      }
      b'"' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'"' {
          i += if bytes[i] == b'\\' { 2 } else { 1 };
        }
        i += 1;
      }
      b'\'' => {
        // Char literal `'x'` / `'\n'` vs lifetime `'a`: only treat as a literal when a closing quote
        // sits within the next few bytes.
        let is_char =
          (bytes.get(i + 1) == Some(&b'\\') && bytes.get(i + 3) == Some(&b'\'')) || bytes.get(i + 2) == Some(&b'\'');
        if is_char {
          i += if bytes.get(i + 1) == Some(&b'\\') { 4 } else { 3 };
        } else {
          i += 1;
        }
      }
      b'{' => {
        depth += 1;
        started = true;
        i += 1;
      }
      b'}' => {
        depth = depth.saturating_sub(1);
        if started && depth == 0 {
          return i;
        }
        i += 1;
      }
      b';' if depth == 0 && !started => return i,
      _ => i += 1,
    }
  }
  bytes.len().saturating_sub(1)
}

// Replace every `#[cfg(test)]`-guarded item with spaces, preserving length and newlines so byte
// offsets and line numbers stay aligned with the original source.
fn blank_test_regions(src: &str) -> String {
  let mut out = src.as_bytes().to_vec();
  let attr = "#[cfg(test)]";
  let mut search = 0;
  while let Some(rel) = src[search..].find(attr) {
    let start = search + rel;
    let end = item_end(src.as_bytes(), start + attr.len());
    for b in out.iter_mut().take(end + 1).skip(start) {
      if *b != b'\n' {
        *b = b' ';
      }
    }
    search = end + 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t') {
    i += 1;
  }
  i
}

// Read a string literal (plain or raw) starting at `i`; returns its inner content and the index just
// past the closing quote. Returns `None` when `i` is not the start of a string literal.
fn read_literal(src: &str, i: usize) -> Option<(String, usize)> {
  let bytes = src.as_bytes();
  if i >= bytes.len() {
    return None;
  }
  if bytes[i] == b'r' {
    let mut j = i + 1;
    let mut hashes = 0;
    while j < bytes.len() && bytes[j] == b'#' {
      hashes += 1;
      j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'"' {
      return None;
    }
    let content_start = j + 1;
    let closing = format!("\"{}", "#".repeat(hashes));
    let rel = src[content_start..].find(&closing)?;
    return Some((
      src[content_start..content_start + rel].to_string(),
      content_start + rel + closing.len(),
    ));
  }
  if bytes[i] != b'"' {
    return None;
  }
  let mut j = i + 1;
  while j < bytes.len() {
    match bytes[j] {
      b'\\' => j += 2,
      b'"' => return Some((src[i + 1..j].to_string(), j + 1)),
      _ => j += 1,
    }
  }
  None
}

// Index of the first comma at paren/bracket/brace depth 0 starting at `i`, skipping string content;
// `None` if a closing delimiter is reached first (single-argument call).
fn top_level_comma(src: &str, mut i: usize) -> Option<usize> {
  let bytes = src.as_bytes();
  let mut depth: i32 = 0;
  while i < bytes.len() {
    match bytes[i] {
      b'"' => {
        if let Some((_, end)) = read_literal(src, i) {
          i = end;
          continue;
        }
        i += 1;
      }
      b'(' | b'[' | b'{' => {
        depth += 1;
        i += 1;
      }
      b')' | b']' | b'}' => {
        if depth == 0 {
          return None;
        }
        depth -= 1;
        i += 1;
      }
      b',' if depth == 0 => return Some(i),
      _ => i += 1,
    }
  }
  None
}

// Given the byte just after a context delimiter (`:` or `(`), return the direct string literal that
// is the value, unwrapping a leading `Some(`. Returns `None` for `t!(...)`, dynamic expressions, etc.
fn literal_value(src: &str, i: usize) -> Option<(String, usize)> {
  let bytes = src.as_bytes();
  let mut at = skip_ws(bytes, i);
  while src[at..].starts_with("Some(") {
    at = skip_ws(bytes, at + "Some(".len());
  }
  read_literal(src, at).map(|(content, _)| (content, at))
}

fn line_of(src: &str, offset: usize) -> usize {
  src[..offset.min(src.len())].bytes().filter(|&b| b == b'\n').count() + 1
}

fn line_text(src: &str, offset: usize) -> &str {
  let offset = offset.min(src.len());
  let start = src[..offset].rfind('\n').map_or(0, |p| p + 1);
  let end = src[offset..].find('\n').map_or(src.len(), |p| offset + p);
  &src[start..end]
}

fn looks_like_i18n_key(content: &str) -> bool {
  if !content.contains('.') {
    return false;
  }
  content.split('.').all(|seg| {
    let bytes = seg.as_bytes();
    !bytes.is_empty()
      && bytes[0].is_ascii_lowercase()
      && bytes
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
  })
}

fn looks_like_url(content: &str) -> bool {
  let t = content.trim_start();
  t.contains("://") || t.starts_with("http") || t.starts_with("www.") || t.starts_with('/') || t.starts_with("mailto:")
}

fn looks_like_sql(content: &str) -> bool {
  let upper = content.trim_start().to_ascii_uppercase();
  SQL_PREFIXES.iter().any(|p| upper.starts_with(p))
}

// A literal with no alphabetic character once `\u{...}` and other escapes are removed: an arrow,
// bullet, separator, or pure punctuation/number that carries no translatable words.
fn is_glyph_only(content: &str) -> bool {
  let mut chars = content.chars().peekable();
  let mut has_alpha = false;
  while let Some(c) = chars.next() {
    if c == '\\' {
      match chars.peek() {
        Some('u') => {
          for n in chars.by_ref() {
            if n == '}' {
              break;
            }
          }
        }
        Some(_) => {
          chars.next();
        }
        None => {}
      }
      continue;
    }
    if c.is_alphabetic() {
      has_alpha = true;
    }
  }
  !has_alpha
}

fn line_is_exempt(line: &str) -> bool {
  if line.trim_start().starts_with("//") {
    return true;
  }
  LINE_EXEMPT_MARKERS.iter().any(|m| line.contains(m))
}

fn is_allowed(content: &str, line: &str) -> bool {
  content.is_empty()
    || is_glyph_only(content)
    || looks_like_i18n_key(content)
    || looks_like_url(content)
    || looks_like_sql(content)
    || line_is_exempt(line)
}

fn scan_field_contexts(file: &str, src: &str, out: &mut Vec<Violation>) {
  let bytes = src.as_bytes();
  for field in FIELD_CONTEXTS {
    for (p, _) in src.match_indices(field) {
      if p != 0 && bytes[p - 1].is_ascii_alphanumeric() {
        continue;
      }
      let end = p + field.len();
      if bytes.get(end) != Some(&b':') || bytes.get(end + 1) == Some(&b':') {
        continue;
      }
      if let Some((content, off)) = literal_value(src, end + 1)
        && !is_allowed(&content, line_text(src, off))
      {
        out.push(Violation {
          context: format!("{field}:"),
          file: file.to_string(),
          line: line_of(src, off),
          literal: content,
        });
      }
    }
  }
}

fn scan_fn_contexts(file: &str, src: &str, out: &mut Vec<Violation>) {
  let bytes = src.as_bytes();
  for name in FN_CONTEXTS {
    for (p, _) in src.match_indices(name) {
      if p != 0 && is_ident_byte(bytes[p - 1]) {
        continue;
      }
      let end = p + name.len();
      if bytes.get(end) != Some(&b'(') {
        continue;
      }
      if let Some((content, off)) = literal_value(src, end + 1)
        && !is_allowed(&content, line_text(src, off))
      {
        out.push(Violation {
          context: format!("{name}("),
          file: file.to_string(),
          line: line_of(src, off),
          literal: content,
        });
      }
    }
  }
}

fn scan_second_arg_contexts(file: &str, src: &str, out: &mut Vec<Violation>) {
  let bytes = src.as_bytes();
  for name in SECOND_ARG_CONTEXTS {
    for (p, _) in src.match_indices(name) {
      if p != 0 && is_ident_byte(bytes[p - 1]) {
        continue;
      }
      let end = p + name.len();
      if bytes.get(end) != Some(&b'(') {
        continue;
      }
      if let Some(comma) = top_level_comma(src, end + 1)
        && let Some((content, off)) = literal_value(src, comma + 1)
        && !is_allowed(&content, line_text(src, off))
      {
        out.push(Violation {
          context: format!("{name}(_,"),
          file: file.to_string(),
          line: line_of(src, off),
          literal: content,
        });
      }
    }
  }
}

fn scan_mcp_descriptions(file: &str, src: &str, out: &mut Vec<Violation>) {
  let bytes = src.as_bytes();
  let needle = "McpTool::new(";
  for (p, _) in src.match_indices(needle) {
    let mut i = skip_ws(bytes, p + needle.len());
    let Some((_, end1)) = read_literal(src, i) else {
      continue;
    };
    i = skip_ws(bytes, end1);
    if bytes.get(i) != Some(&b',') {
      continue;
    }
    i = skip_ws(bytes, i + 1);
    // MCP tool descriptions are LLM-facing and intentionally NOT exempt; only skip an empty or
    // glyph-only literal to avoid meaningless noise.
    if let Some((content, off)) = read_literal(src, i).map(|(c, _)| (c, i))
      && !content.is_empty()
      && !is_glyph_only(&content)
    {
      out.push(Violation {
        context: "McpTool desc".to_string(),
        file: file.to_string(),
        line: line_of(src, off),
        literal: content,
      });
    }
  }
}

fn scan_date_literals(file: &str, src: &str, out: &mut Vec<Violation>) {
  let mut seen: BTreeSet<(usize, &str)> = BTreeSet::new();
  for name in DATE_LITERALS {
    let quoted = format!("\"{name}\"");
    for (p, _) in src.match_indices(&quoted) {
      let line = line_text(src, p);
      if line.trim_start().starts_with("//") {
        continue;
      }
      let line_no = line_of(src, p);
      if seen.insert((line_no, name)) {
        out.push(Violation {
          context: "date array".to_string(),
          file: file.to_string(),
          line: line_no,
          literal: name.to_string(),
        });
      }
    }
  }
}

fn collect(scan: impl Fn(&str, &str, &mut Vec<Violation>)) -> Vec<Violation> {
  let mut files = Vec::new();
  rust_files(&src_dir(), &mut files);
  files.sort();

  let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut out = Vec::new();
  for path in files {
    let Ok(raw) = fs::read_to_string(&path) else {
      continue;
    };
    let rel = path.strip_prefix(manifest).unwrap_or(&path).display().to_string();
    let blanked = blank_test_regions(&raw);
    scan(&rel, &blanked, &mut out);
  }
  out
}

fn render(violations: &[Violation]) -> String {
  let mut by_file: BTreeMap<&str, Vec<&Violation>> = BTreeMap::new();
  for v in violations {
    by_file.entry(v.file.as_str()).or_default().push(v);
  }
  let mut out = String::new();
  for (file, mut list) in by_file {
    list.sort_by(|a, b| a.line.cmp(&b.line).then(a.context.cmp(&b.context)));
    out.push_str(file);
    out.push('\n');
    for v in list {
      let literal = if v.literal.chars().count() > 80 {
        format!("{}…", v.literal.chars().take(79).collect::<String>())
      } else {
        v.literal.clone()
      };
      out.push_str(&format!("  {:>6}  {:<14}  \"{}\"\n", v.line, v.context, literal));
    }
    out.push('\n');
  }
  out
}

#[test]
#[ignore = "dormant migration worklist; run with: cargo test --test i18n_no_hardcoded -- --ignored"]
fn no_hardcoded_user_facing_strings() {
  let mut violations = collect(|file, src, out| {
    scan_field_contexts(file, src, out);
    scan_fn_contexts(file, src, out);
    scan_second_arg_contexts(file, src, out);
    scan_mcp_descriptions(file, src, out);
  });
  violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

  assert!(
    violations.is_empty(),
    "found {} user-facing string literal(s) outside `t!(...)`. Wrap each in `t!(\"feature.section.key\")` \
     and add the key to the locales. Worklist:\n\n{}",
    violations.len(),
    render(&violations)
  );
}

#[test]
#[ignore = "dormant migration worklist; run with: cargo test --test i18n_no_hardcoded -- --ignored"]
fn no_hardcoded_month_or_weekday_arrays() {
  let violations = collect(scan_date_literals);

  assert!(
    violations.is_empty(),
    "found {} hard-coded month/weekday name literal(s). Consolidate these duplicated date arrays into \
     one shared `t!()`-backed helper. Worklist:\n\n{}",
    violations.len(),
    render(&violations)
  );
}
