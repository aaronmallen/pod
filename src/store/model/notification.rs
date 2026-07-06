use getset::{CopyGetters, Getters};
use sqlx::FromRow;

const DEST_ASSETS: &str = "assets";

const DEST_CALENDAR: &str = "calendar";

const DEST_CAPTAINS_LOG: &str = "captains_log";

const DEST_CHARACTER_DETAIL: &str = "character_detail";

const DEST_INDUSTRY: &str = "industry";

const DEST_KILLMAIL: &str = "killmail";

const DEST_MAIL: &str = "mail";

const DEST_SKILLS: &str = "skills";

const DEST_WALLET: &str = "wallet";

const KIND_CALENDAR: &str = "calendar";

const KIND_CAPTAINS_LOG: &str = "captains_log";

const KIND_EXTRACTION_CRACKED: &str = "extraction_cracked";

const KIND_EXTRACTION_SCHEDULED: &str = "extraction_scheduled";

const KIND_INDUSTRY: &str = "industry";

const KIND_KILLMAIL: &str = "killmail";

const KIND_MAIL: &str = "mail";

const KIND_SKILL: &str = "skill";

const KIND_WALLET_GAP: &str = "wallet_gap";

const OWNER_CHARACTER: &str = "character";

const OWNER_CORPORATION: &str = "corporation";

// A keyset cursor into the surfaced-row history: the (created_at, id) of the last row a page
// returned. The next page returns surfaced rows strictly older than this in the (created_at DESC,
// id DESC) order, so paging walks the whole history with no duplicated or skipped rows even when new
// rows are inserted between fetches (a newer insert sorts before the cursor and never reappears in a
// later page).
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryCursor {
  pub created_at: String,
  pub id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewNotification {
  pub body: String,
  pub dedup_key: String,
  pub kind: NotificationKind,
  pub owner: NotificationOwner,
  pub target: NotificationTarget,
  pub title: String,
}

#[derive(Clone, CopyGetters, Debug, Getters, PartialEq)]
pub struct Notification {
  #[getset(get = "pub")]
  pub body: String,
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get = "pub")]
  pub dedup_key: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub kind: NotificationKind,
  #[getset(get_copy = "pub")]
  pub owner: NotificationOwner,
  #[getset(get = "pub")]
  pub read_at: Option<String>,
  #[getset(get = "pub")]
  pub target: NotificationTarget,
  #[getset(get = "pub")]
  pub title: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NotificationDestination {
  Assets,
  Calendar,
  CaptainsLog,
  CharacterDetail,
  Industry,
  Killmail,
  Mail,
  Skills,
  Wallet,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NotificationKind {
  Calendar,
  CaptainsLog,
  ExtractionCracked,
  ExtractionScheduled,
  Industry,
  Killmail,
  Mail,
  Skill,
  WalletGap,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NotificationOwner {
  Character(i64),
  Corporation(i64),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NotificationTarget {
  pub character: Option<i64>,
  pub destination: NotificationDestination,
  pub sub: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
pub(crate) struct NotificationRow {
  pub body: String,
  pub created_at: String,
  pub dedup_key: String,
  pub id: i64,
  pub kind: String,
  pub owner_id: i64,
  pub owner_type: String,
  pub read_at: Option<String>,
  pub target_char: Option<i64>,
  pub target_dest: String,
  pub target_sub: Option<String>,
  pub title: String,
}

impl HistoryCursor {
  pub fn from_page(page: &[Notification]) -> Option<Self> {
    page.last().map(|last| Self {
      created_at: last.created_at().clone(),
      id: last.id(),
    })
  }
}

impl NotificationDestination {
  pub fn as_str(self) -> &'static str {
    match self {
      NotificationDestination::Assets => DEST_ASSETS,
      NotificationDestination::Calendar => DEST_CALENDAR,
      NotificationDestination::CaptainsLog => DEST_CAPTAINS_LOG,
      NotificationDestination::CharacterDetail => DEST_CHARACTER_DETAIL,
      NotificationDestination::Industry => DEST_INDUSTRY,
      NotificationDestination::Killmail => DEST_KILLMAIL,
      NotificationDestination::Mail => DEST_MAIL,
      NotificationDestination::Skills => DEST_SKILLS,
      NotificationDestination::Wallet => DEST_WALLET,
    }
  }

  /// Parses a DB key, returning `Wallet` for any unrecognised value rather than panicking; a stored
  /// destination should always be one of the known keys, so an unknown one is a corrupted row that
  /// still routes somewhere sane instead of crashing the notification list.
  pub fn from_key(key: &str) -> Self {
    match key {
      DEST_ASSETS => NotificationDestination::Assets,
      DEST_CALENDAR => NotificationDestination::Calendar,
      DEST_CAPTAINS_LOG => NotificationDestination::CaptainsLog,
      DEST_CHARACTER_DETAIL => NotificationDestination::CharacterDetail,
      DEST_INDUSTRY => NotificationDestination::Industry,
      DEST_KILLMAIL => NotificationDestination::Killmail,
      DEST_MAIL => NotificationDestination::Mail,
      DEST_SKILLS => NotificationDestination::Skills,
      _ => NotificationDestination::Wallet,
    }
  }
}

impl NotificationKind {
  pub fn as_str(self) -> &'static str {
    match self {
      NotificationKind::Calendar => KIND_CALENDAR,
      NotificationKind::CaptainsLog => KIND_CAPTAINS_LOG,
      NotificationKind::ExtractionCracked => KIND_EXTRACTION_CRACKED,
      NotificationKind::ExtractionScheduled => KIND_EXTRACTION_SCHEDULED,
      NotificationKind::Industry => KIND_INDUSTRY,
      NotificationKind::Killmail => KIND_KILLMAIL,
      NotificationKind::Mail => KIND_MAIL,
      NotificationKind::Skill => KIND_SKILL,
      NotificationKind::WalletGap => KIND_WALLET_GAP,
    }
  }

  /// Parses a DB key, returning `None` for any unrecognised value. A row whose kind no longer maps to
  /// a known variant is treated as undecodable rather than silently coerced, so the repo can drop it.
  pub fn from_key(key: &str) -> Option<Self> {
    match key {
      KIND_CALENDAR => Some(NotificationKind::Calendar),
      KIND_CAPTAINS_LOG => Some(NotificationKind::CaptainsLog),
      KIND_EXTRACTION_CRACKED => Some(NotificationKind::ExtractionCracked),
      KIND_EXTRACTION_SCHEDULED => Some(NotificationKind::ExtractionScheduled),
      KIND_INDUSTRY => Some(NotificationKind::Industry),
      KIND_KILLMAIL => Some(NotificationKind::Killmail),
      KIND_MAIL => Some(NotificationKind::Mail),
      KIND_SKILL => Some(NotificationKind::Skill),
      KIND_WALLET_GAP => Some(NotificationKind::WalletGap),
      _ => None,
    }
  }
}

impl NotificationOwner {
  pub fn from_key(owner_type: &str, owner_id: i64) -> Option<Self> {
    match owner_type {
      OWNER_CHARACTER => Some(NotificationOwner::Character(owner_id)),
      OWNER_CORPORATION => Some(NotificationOwner::Corporation(owner_id)),
      _ => None,
    }
  }

  pub fn owner_id(self) -> i64 {
    match self {
      NotificationOwner::Character(id) | NotificationOwner::Corporation(id) => id,
    }
  }

  pub fn owner_type(self) -> &'static str {
    match self {
      NotificationOwner::Character(_) => OWNER_CHARACTER,
      NotificationOwner::Corporation(_) => OWNER_CORPORATION,
    }
  }
}

impl NotificationRow {
  pub(crate) fn into_notification(self) -> Option<Notification> {
    Some(Notification {
      body: self.body,
      created_at: self.created_at,
      dedup_key: self.dedup_key,
      id: self.id,
      kind: NotificationKind::from_key(&self.kind)?,
      owner: NotificationOwner::from_key(&self.owner_type, self.owner_id)?,
      read_at: self.read_at,
      target: NotificationTarget {
        character: self.target_char,
        destination: NotificationDestination::from_key(&self.target_dest),
        sub: self.target_sub,
      },
      title: self.title,
    })
  }
}

impl NotificationTarget {
  pub fn killmail(owner: NotificationOwner, killmail_id: i64) -> Self {
    NotificationTarget {
      character: match owner {
        NotificationOwner::Character(id) => Some(id),
        NotificationOwner::Corporation(_) => None,
      },
      destination: NotificationDestination::Killmail,
      sub: Some(format!("{}:{}:{}", owner.owner_type(), owner.owner_id(), killmail_id)),
    }
  }

  pub fn killmail_link(&self) -> Option<(NotificationOwner, i64)> {
    let mut parts = self.sub.as_deref()?.split(':');
    let owner_type = parts.next()?;
    let owner_id = parts.next()?.parse().ok()?;
    let killmail_id = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
      return None;
    }

    Some((NotificationOwner::from_key(owner_type, owner_id)?, killmail_id))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod notification_destination {
    use super::*;

    mod from_key {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_defaults_unknown_keys_to_wallet() {
        assert_eq!(
          NotificationDestination::from_key("garbage"),
          NotificationDestination::Wallet
        );
      }
    }

    mod round_trip {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_every_destination() {
        for destination in [
          NotificationDestination::Assets,
          NotificationDestination::Calendar,
          NotificationDestination::CaptainsLog,
          NotificationDestination::CharacterDetail,
          NotificationDestination::Industry,
          NotificationDestination::Killmail,
          NotificationDestination::Mail,
          NotificationDestination::Skills,
          NotificationDestination::Wallet,
        ] {
          assert_eq!(NotificationDestination::from_key(destination.as_str()), destination);
        }
      }
    }
  }

  mod notification_kind {
    use super::*;

    mod from_key {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_rejects_an_unknown_key() {
        assert_eq!(NotificationKind::from_key("garbage"), None);
      }
    }

    mod round_trip {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_every_kind() {
        for kind in [
          NotificationKind::Calendar,
          NotificationKind::CaptainsLog,
          NotificationKind::ExtractionCracked,
          NotificationKind::ExtractionScheduled,
          NotificationKind::Industry,
          NotificationKind::Killmail,
          NotificationKind::Mail,
          NotificationKind::Skill,
          NotificationKind::WalletGap,
        ] {
          assert_eq!(NotificationKind::from_key(kind.as_str()), Some(kind));
        }
      }
    }
  }

  mod notification_target {
    use super::*;

    mod killmail_link {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_a_character_owned_killmail() {
        let target = NotificationTarget::killmail(NotificationOwner::Character(42), 100);

        assert_eq!(target.destination, NotificationDestination::Killmail);
        assert_eq!(target.character, Some(42));
        assert_eq!(target.killmail_link(), Some((NotificationOwner::Character(42), 100)));
      }

      #[test]
      fn it_round_trips_a_corporation_owned_killmail() {
        let target = NotificationTarget::killmail(NotificationOwner::Corporation(99), 100);

        assert_eq!(target.character, None);
        assert_eq!(target.killmail_link(), Some((NotificationOwner::Corporation(99), 100)));
      }

      #[test]
      fn it_returns_none_for_a_missing_or_malformed_sub() {
        for sub in [
          None,
          Some(String::new()),
          Some("character:42".to_owned()),
          Some("x:y:z".to_owned()),
        ] {
          let target = NotificationTarget {
            character: Some(42),
            destination: NotificationDestination::Killmail,
            sub,
          };

          assert_eq!(target.killmail_link(), None);
        }
      }
    }
  }

  mod notification_owner {
    use super::*;

    mod from_key {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_maps_the_known_types() {
        assert_eq!(
          NotificationOwner::from_key("character", 7),
          Some(NotificationOwner::Character(7))
        );
        assert_eq!(
          NotificationOwner::from_key("corporation", 9),
          Some(NotificationOwner::Corporation(9))
        );
      }

      #[test]
      fn it_rejects_an_unknown_type() {
        assert_eq!(NotificationOwner::from_key("alliance", 1), None);
      }
    }

    mod round_trip {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_round_trips_through_the_persisted_type() {
        for owner in [NotificationOwner::Character(3), NotificationOwner::Corporation(8)] {
          assert_eq!(
            NotificationOwner::from_key(owner.owner_type(), owner.owner_id()),
            Some(owner)
          );
        }
      }
    }
  }
}
