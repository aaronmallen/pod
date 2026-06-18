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
//!
//! The compose panel (task `uwqlrkzr-C`) is the first consumer; until it lands
//! the public surface is unused, so the module allows dead code.
#![allow(dead_code)]

/// `showinfo:` type-id for a character.
pub const CHARACTER_TYPE_ID: i64 = 1377;

/// `showinfo:` type-id for a corporation.
pub const CORPORATION_TYPE_ID: i64 = 2;

/// `showinfo:` type-id for a solar system.
pub const SOLAR_SYSTEM_TYPE_ID: i64 = 5;

/// A link the composer can embed in a mail body.
///
/// Each variant carries the display text shown to the reader and whatever ids
/// are needed to build the underlying href. Construct via the helper
/// constructors so the showinfo type-ids stay correct in one place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Link {
  /// A character entity link (`showinfo:1377//<character-id>`).
  Character { id: i64, name: String },
  /// A corporation entity link (`showinfo:2//<corporation-id>`).
  Corporation { id: i64, name: String },
  /// A plain web link (`<a href="URL">text</a>`).
  Http { url: String, text: String },
  /// A solar-system entity link (`showinfo:5//<system-id>`).
  SolarSystem { id: i64, name: String },
  /// A station entity link.
  ///
  /// When `type_id` is `Some`, this renders a per-station showinfo link
  /// (`showinfo:<type-id>//<station-id>`). When it is `None` — i.e. the SDE
  /// could not supply a reliable per-station type-id — the link degrades to a
  /// solar-system-level link so the reader still gets a clickable target near
  /// the station rather than a dead href. The display text is unchanged.
  Station {
    id: i64,
    name: String,
    system_id: i64,
    type_id: Option<i64>,
  },
}

impl Link {
  /// Build a character link.
  pub fn character(id: i64, name: impl Into<String>) -> Self {
    Self::Character {
      id,
      name: name.into(),
    }
  }

  /// Build a corporation link.
  pub fn corporation(id: i64, name: impl Into<String>) -> Self {
    Self::Corporation {
      id,
      name: name.into(),
    }
  }

  /// Build a plain web link.
  pub fn http(url: impl Into<String>, text: impl Into<String>) -> Self {
    Self::Http {
      url: url.into(),
      text: text.into(),
    }
  }

  /// Build a solar-system link.
  pub fn solar_system(id: i64, name: impl Into<String>) -> Self {
    Self::SolarSystem {
      id,
      name: name.into(),
    }
  }

  /// Build a station link.
  ///
  /// `type_id` is the per-station showinfo type-id from the SDE
  /// (`stations.type_id`). Pass `None` when no reliable type-id is available to
  /// degrade to a solar-system-level link anchored at `system_id`.
  pub fn station(id: i64, name: impl Into<String>, system_id: i64, type_id: Option<i64>) -> Self {
    Self::Station {
      id,
      name: name.into(),
      system_id,
      type_id,
    }
  }

  /// Render this link as EVE wire markup (`<a href="...">text</a>`).
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
      Self::Station {
        id,
        name,
        system_id,
        type_id,
      } => match type_id {
        Some(type_id) => showinfo_link(*type_id, *id, name),
        None => showinfo_link(SOLAR_SYSTEM_TYPE_ID, *system_id, name),
      },
    }
  }
}

/// Wrap `text` in bold tags (`<b>text</b>`).
pub fn bold(text: &str) -> String {
  format!("<b>{text}</b>")
}

/// Wrap `text` in italic tags (`<i>text</i>`).
pub fn italic(text: &str) -> String {
  format!("<i>{text}</i>")
}

/// Build a `showinfo:` entity link: `<a href="showinfo:TYPEID//ITEMID">Name</a>`.
fn showinfo_link(type_id: i64, item_id: i64, text: &str) -> String {
  format!("<a href=\"showinfo:{type_id}//{item_id}\">{text}</a>")
}

/// Build a plain web link: `<a href="URL">text</a>`.
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

    #[test]
    fn it_builds_a_station_link_with_the_per_station_type_id() {
      let link = Link::station(60_003_760, "Jita IV - Moon 4", 30_000_142, Some(52678));

      assert_eq!(
        link.to_markup(),
        "<a href=\"showinfo:52678//60003760\">Jita IV - Moon 4</a>"
      );
    }

    #[test]
    fn it_degrades_a_station_without_a_type_id_to_a_system_link() {
      let link = Link::station(60_003_760, "Jita IV - Moon 4", 30_000_142, None);

      assert_eq!(
        link.to_markup(),
        "<a href=\"showinfo:5//30000142\">Jita IV - Moon 4</a>"
      );
    }
  }
}
