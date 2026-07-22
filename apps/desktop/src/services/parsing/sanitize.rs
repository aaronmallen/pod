#[cfg(any(windows, test))]
const HTML_END_FRAGMENT: &str = "<!--EndFragment-->";
#[cfg(any(windows, test))]
const HTML_START_FRAGMENT: &str = "<!--StartFragment-->";
const LINE_BREAK_TAGS: [&str; 6] = ["br", "div", "li", "p", "tr", "ul"];

#[cfg(any(windows, test))]
pub fn html_fragment_to_text(html: &str) -> String {
  sanitize(extract_html_fragment(html))
}

pub fn sanitize(raw: &str) -> String {
  normalize_whitespace(&decode_entities(&strip_html_tags(raw)))
}

fn decode_entities(input: &str) -> String {
  input
    .replace("&nbsp;", " ")
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&quot;", "\"")
    .replace("&#39;", "'")
    .replace("&amp;", "&")
}

#[cfg(any(windows, test))]
fn extract_html_fragment(html: &str) -> &str {
  let start = html
    .find(HTML_START_FRAGMENT)
    .map(|index| index + HTML_START_FRAGMENT.len());
  let end = html.find(HTML_END_FRAGMENT);
  match (start, end) {
    (Some(start), Some(end)) if start <= end => &html[start..end],
    (Some(start), _) => &html[start..],
    _ => html,
  }
}

fn normalize_whitespace(input: &str) -> String {
  input
    .chars()
    .filter(|&c| c != '\u{FEFF}' && c != '\0')
    .map(|c| {
      if c.is_whitespace() && !matches!(c, ' ' | '\n' | '\r' | '\t') {
        ' '
      } else {
        c
      }
    })
    .collect()
}

fn strip_html_tags(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let mut tag = String::new();
  let mut in_tag = false;
  for c in input.chars() {
    match c {
      '<' => {
        in_tag = true;
        tag.clear();
      }
      '>' if in_tag => {
        in_tag = false;
        let trimmed = tag.trim();
        let is_closing = trimmed.starts_with('/');
        let name = trimmed
          .trim_start_matches('/')
          .split(|c: char| c.is_whitespace() || c == '/')
          .next()
          .unwrap_or("")
          .to_ascii_lowercase();
        if name == "br" || (is_closing && LINE_BREAK_TAGS.contains(&name.as_str())) {
          out.push('\n');
        }
      }
      _ if in_tag => tag.push(c),
      _ => out.push(c),
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  mod sanitize {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_strips_a_leading_bom_and_nul() {
      assert_eq!(sanitize("\u{FEFF}\0Gunnery V"), "Gunnery V");
    }

    #[test]
    fn it_normalizes_nbsp_and_unicode_spaces_to_ascii_space() {
      assert_eq!(sanitize("Gunnery\u{00A0}V"), "Gunnery V");
      assert_eq!(sanitize("Gunnery\u{2003}V"), "Gunnery V");
    }

    #[test]
    fn it_preserves_newlines_carriage_returns_and_tabs() {
      assert_eq!(sanitize("Gunnery V\r\nMissiles\tIV"), "Gunnery V\r\nMissiles\tIV");
    }

    #[test]
    fn it_strips_residual_html_tags_and_decodes_entities() {
      assert_eq!(sanitize("<b>Gunnery</b> &amp; Missiles V"), "Gunnery & Missiles V");
    }

    #[test]
    fn it_leaves_clean_plain_text_unchanged() {
      assert_eq!(
        sanitize("Gunnery V\nSmall Hybrid Turret 4"),
        "Gunnery V\nSmall Hybrid Turret 4"
      );
    }
  }

  mod html_fragment_to_text {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_extracts_only_the_marked_fragment() {
      let cf_html = concat!(
        "Version:0.9\r\n",
        "StartHTML:00000097\r\n",
        "EndHTML:00000200\r\n",
        "StartFragment:00000131\r\n",
        "EndFragment:00000164\r\n",
        "<html><body><!--StartFragment-->Gunnery V<!--EndFragment--></body></html>",
      );

      assert_eq!(html_fragment_to_text(cf_html), "Gunnery V");
    }

    #[test]
    fn it_turns_block_tags_into_newlines_and_strips_markup() {
      let cf_html = concat!(
        "<!--StartFragment-->",
        "<table><tr><td>Gunnery V</td></tr><tr><td>Small Hybrid Turret IV</td></tr></table>",
        "<!--EndFragment-->",
      );

      assert_eq!(html_fragment_to_text(cf_html), "Gunnery V\nSmall Hybrid Turret IV\n");
    }

    #[test]
    fn it_decodes_entities_and_normalizes_nbsp() {
      let cf_html = "<!--StartFragment--><p>Drones&nbsp;&amp;\u{00A0}Rigging V</p><!--EndFragment-->";

      assert_eq!(html_fragment_to_text(cf_html), "Drones & Rigging V\n");
    }

    #[test]
    fn it_falls_back_to_the_whole_input_without_fragment_markers() {
      assert_eq!(html_fragment_to_text("Gunnery V"), "Gunnery V");
    }
  }
}
