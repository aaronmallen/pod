use std::collections::HashMap;

use crate::store::{
  images::{self, ImageKind, ImageState, Store},
  model::{CharacterContact, CharacterContactLabel},
};

#[derive(Clone, Debug, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct CharacterContacts {
  pub contacts: Vec<CharacterContact>,
  /// Per-contact entity avatar, keyed by `contact_id`: a character portrait, corporation logo, or alliance logo
  /// resolved at load time. Surfaced through `stale_images` so evicted avatars get refetched.
  pub images: HashMap<i64, ImageState>,
  pub labels: Vec<CharacterContactLabel>,
}

impl CharacterContacts {
  /// Looks up a contact's resolved avatar by `contact_id`. The character-detail tab now paginates contacts and
  /// resolves each page's avatars itself, so this whole-set accessor survives only for the view-model tests and
  /// any future whole-set consumer.
  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn image(&self, contact_id: i64) -> Option<&ImageState> {
    self.images.get(&contact_id)
  }

  /// Builds the view from raw rows, resolving each contact's avatar against the image cache.
  // Public store API exercised by unit tests; not yet wired into a production call site.
  pub fn resolved(store: &Store, contacts: Vec<CharacterContact>, labels: Vec<CharacterContactLabel>) -> Self {
    let images = contacts
      .iter()
      .map(|contact| {
        let kind = image_kind(contact.contact_type());
        (contact.contact_id(), images::resolve(store, kind, contact.contact_id()))
      })
      .collect();

    CharacterContacts {
      contacts,
      images,
      labels,
    }
  }
}

/// Maps an ESI contact type to its avatar image kind. Unknown types fall back to a corporation logo.
pub fn image_kind(contact_type: &str) -> ImageKind {
  match contact_type.to_ascii_lowercase().as_str() {
    "alliance" => ImageKind::AllianceLogo,
    "character" => ImageKind::CharacterPortrait,
    _ => ImageKind::CorporationLogo,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod image_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_a_corporation_logo_for_unknown_types() {
      assert_eq!(image_kind("faction"), ImageKind::CorporationLogo);
    }

    #[test]
    fn it_ignores_case() {
      assert_eq!(image_kind("Alliance"), ImageKind::AllianceLogo);
      assert_eq!(image_kind("CHARACTER"), ImageKind::CharacterPortrait);
    }

    #[test]
    fn it_maps_each_contact_type_to_its_avatar_kind() {
      assert_eq!(image_kind("alliance"), ImageKind::AllianceLogo);
      assert_eq!(image_kind("character"), ImageKind::CharacterPortrait);
      assert_eq!(image_kind("corporation"), ImageKind::CorporationLogo);
    }
  }

  mod resolved {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    fn contact(id: i64, kind: &str) -> CharacterContact {
      CharacterContact {
        character_id: 1,
        contact_id: id,
        contact_name: "Test".to_owned(),
        contact_type: kind.to_owned(),
        is_blocked: false,
        is_watched: false,
        label_ids: "[]".to_owned(),
        standing: 0.0,
      }
    }

    #[test]
    fn it_resolves_an_avatar_for_each_contact_keyed_by_id() {
      let store = Store::new(PathBuf::from("/data/images"));
      let contacts = vec![contact(100, "character"), contact(200, "alliance")];

      let view = CharacterContacts::resolved(&store, contacts, Vec::new());

      assert_eq!(
        view.image(100).and_then(ImageState::stale_key),
        Some((ImageKind::CharacterPortrait, 100))
      );
      assert_eq!(
        view.image(200).and_then(ImageState::stale_key),
        Some((ImageKind::AllianceLogo, 200))
      );
    }
  }
}
