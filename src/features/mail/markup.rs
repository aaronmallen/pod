//! Pure builder for the EVE mail wire markup the send pipeline carries verbatim.
//!
//! EVE mail bodies are an HTML-like markup. The composer wraps selections in
//! `<b>`/`<i>` emphasis tags and embeds entity/URL links as
//! `<a href="...">text</a>`. Entity links use the in-game `showinfo:` scheme,
//! whose first path segment is a *type-id* (what kind of thing to show) and
//! whose second segment is the *item-id* (which specific thing):
//! `showinfo:<type-id>//<item-id>`.
//!
//! This module is the single tested place that emits those strings. It is a
//! leaf module (no DB, no UI, no async) so the compose panel and the read-side
//! renderer share one markup contract. Callers resolve ids (and, for stations,
//! the per-station type-id) from the SDE and hand them in here.

pub const CHARACTER_TYPE_ID: i64 = 1377;

pub const CORPORATION_TYPE_ID: i64 = 2;

pub const SOLAR_SYSTEM_TYPE_ID: i64 = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Link {
  Character { id: i64, name: String },
  Corporation { id: i64, name: String },
  Http { url: String, text: String },
  SolarSystem { id: i64, name: String },
}

impl Link {
  pub fn character(id: i64, name: impl Into<String>) -> Self {
    Self::Character {
      id,
      name: name.into(),
    }
  }

  pub fn corporation(id: i64, name: impl Into<String>) -> Self {
    Self::Corporation {
      id,
      name: name.into(),
    }
  }

  pub fn http(url: impl Into<String>, text: impl Into<String>) -> Self {
    Self::Http {
      url: url.into(),
      text: text.into(),
    }
  }

  pub fn solar_system(id: i64, name: impl Into<String>) -> Self {
    Self::SolarSystem {
      id,
      name: name.into(),
    }
  }

  pub fn to_markup(&self) -> String {
    match self {
      Self::Character {
        id,
        name,
      } => showinfo_link(CHARACTER_TYPE_ID, *id, name),
      Self::Corporation {
        id,
        name,
      } => showinfo_link(CORPORATION_TYPE_ID, *id, name),
      Self::Http {
        url,
        text,
      } => http_link(url, text),
      Self::SolarSystem {
        id,
        name,
      } => showinfo_link(SOLAR_SYSTEM_TYPE_ID, *id, name),
    }
  }
}

pub fn bold(text: &str) -> String {
  format!("<b>{text}</b>")
}

pub fn italic(text: &str) -> String {
  format!("<i>{text}</i>")
}

fn showinfo_link(type_id: i64, item_id: i64, text: &str) -> String {
  format!("<a href=\"showinfo:{type_id}//{item_id}\">{text}</a>")
}

fn http_link(url: &str, text: &str) -> String {
  format!("<a href=\"{url}\">{text}</a>")
}

#[cfg(test)]
mod tests {
  use super::*;

  mod constants {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pins_the_showinfo_type_ids() {
      assert_eq!(CHARACTER_TYPE_ID, 1377);
      assert_eq!(CORPORATION_TYPE_ID, 2);
      assert_eq!(SOLAR_SYSTEM_TYPE_ID, 5);
    }
  }

  mod emphasis {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_wraps_bold() {
      assert_eq!(bold("Form up"), "<b>Form up</b>");
    }

    #[test]
    fn it_wraps_italic() {
      assert_eq!(italic("19:15"), "<i>19:15</i>");
    }
  }

  mod to_markup {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_a_character_link_with_type_id_1377() {
      let link = Link::character(90_000_001, "Pod Pilot");

      assert_eq!(link.to_markup(), "<a href=\"showinfo:1377//90000001\">Pod Pilot</a>");
    }

    #[test]
    fn it_builds_a_corporation_link_with_type_id_2() {
      let link = Link::corporation(98_000_001, "Test Corp");

      assert_eq!(link.to_markup(), "<a href=\"showinfo:2//98000001\">Test Corp</a>");
    }

    #[test]
    fn it_builds_a_solar_system_link_with_type_id_5() {
      let link = Link::solar_system(30_000_142, "Jita");

      assert_eq!(link.to_markup(), "<a href=\"showinfo:5//30000142\">Jita</a>");
    }

    #[test]
    fn it_builds_an_http_link() {
      let link = Link::http("https://example.com/op", "Op details");

      assert_eq!(link.to_markup(), "<a href=\"https://example.com/op\">Op details</a>");
    }
  }
}
