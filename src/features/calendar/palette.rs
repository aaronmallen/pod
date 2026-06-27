use iced::Color;

use crate::ui::{components::icon::Icon, style::color};

pub const TYPE_LEGEND_ORDER: [OwnerType; 5] = [
  OwnerType::Alliance,
  OwnerType::Corporation,
  OwnerType::Faction,
  OwnerType::EveServer,
  OwnerType::Character,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerType {
  Alliance,
  Character,
  Corporation,
  EveServer,
  Faction,
  Pod,
}

impl OwnerType {
  pub fn from_esi(owner_type: &str) -> Self {
    match owner_type {
      "alliance" => OwnerType::Alliance,
      "corporation" => OwnerType::Corporation,
      "faction" => OwnerType::Faction,
      "eve_server" => OwnerType::EveServer,
      "pod" => OwnerType::Pod,
      _ => OwnerType::Character,
    }
  }

  pub fn color(self) -> Color {
    match self {
      OwnerType::Alliance => color::accent::PLASMA,
      OwnerType::Character => color::chart::VIOLET,
      OwnerType::Corporation => color::with_alpha(color::chart::VIOLET, 0.78),
      OwnerType::EveServer => color::status::DANGER,
      OwnerType::Faction => color::status::WARNING,
      OwnerType::Pod => color::status::ONLINE,
    }
  }

  pub fn icon(self) -> Icon {
    match self {
      OwnerType::Alliance => Icon::fleet(),
      OwnerType::Character => Icon::personal(),
      OwnerType::Corporation => Icon::corp(),
      OwnerType::EveServer => Icon::live(),
      OwnerType::Faction => Icon::faction(),
      OwnerType::Pod => Icon::skills(),
    }
  }

  pub fn label(self) -> String {
    match self {
      OwnerType::Alliance => t!("calendar.owner.alliance"),
      OwnerType::Character => t!("calendar.owner.character"),
      OwnerType::Corporation => t!("calendar.owner.corporation"),
      OwnerType::EveServer => t!("calendar.owner.eve_server"),
      OwnerType::Faction => t!("calendar.owner.faction"),
      OwnerType::Pod => t!("calendar.owner.pod"),
    }
    .into_owned()
  }

  pub fn respondable(self) -> bool {
    matches!(self, OwnerType::Alliance | OwnerType::Corporation | OwnerType::Faction)
  }

  pub fn short_label(self) -> String {
    match self {
      OwnerType::Alliance => t!("calendar.owner_short.alliance"),
      OwnerType::Character => t!("calendar.owner_short.character"),
      OwnerType::Corporation => t!("calendar.owner_short.corporation"),
      OwnerType::EveServer => t!("calendar.owner_short.eve_server"),
      OwnerType::Faction => t!("calendar.owner_short.faction"),
      OwnerType::Pod => t!("calendar.owner_short.pod"),
    }
    .into_owned()
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
  Accepted,
  Declined,
  NotResponded,
  Tentative,
}

impl Response {
  pub fn from_esi(response: &str) -> Self {
    match response {
      "accepted" => Response::Accepted,
      "declined" => Response::Declined,
      "tentative" => Response::Tentative,
      _ => Response::NotResponded,
    }
  }

  pub fn as_esi(self) -> &'static str {
    match self {
      Response::Accepted => "accepted",
      Response::Declined => "declined",
      Response::NotResponded => "not_responded",
      Response::Tentative => "tentative",
    }
  }

  pub fn color(self) -> Color {
    match self {
      Response::Accepted => color::status::ONLINE,
      Response::Declined => color::status::DANGER,
      Response::NotResponded => color::text::secondary(),
      Response::Tentative => color::status::WARNING,
    }
  }

  pub fn label(self) -> String {
    match self {
      Response::Accepted => t!("calendar.response.accepted"),
      Response::Declined => t!("calendar.response.declined"),
      Response::NotResponded => t!("calendar.response.not_responded"),
      Response::Tentative => t!("calendar.response.tentative"),
    }
    .into_owned()
  }

  pub fn pill_label(self) -> String {
    match self {
      Response::Accepted => t!("calendar.response_pill.accepted"),
      Response::Declined => t!("calendar.response_pill.declined"),
      Response::NotResponded => t!("calendar.response_pill.not_responded"),
      Response::Tentative => t!("calendar.response_pill.tentative"),
    }
    .into_owned()
  }
}

pub fn pilot_color(index: usize) -> Color {
  color::chart::series(index)
}

#[cfg(test)]
mod tests {
  use super::*;

  const OWNERS: [OwnerType; 6] = [
    OwnerType::Alliance,
    OwnerType::Character,
    OwnerType::Corporation,
    OwnerType::EveServer,
    OwnerType::Faction,
    OwnerType::Pod,
  ];

  const RESPONSES: [Response; 4] = [
    Response::Accepted,
    Response::Declined,
    Response::NotResponded,
    Response::Tentative,
  ];

  mod owner_type {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_exposes_a_color_icon_and_labels_for_every_owner() {
      for owner in OWNERS {
        let _ = owner.color();
        let _ = owner.icon();
        assert!(!owner.label().is_empty());
        assert!(!owner.short_label().is_empty());
        let _ = owner.respondable();
      }
    }

    #[test]
    fn it_parses_each_esi_owner_type_and_defaults_to_character() {
      assert_eq!(OwnerType::from_esi("alliance"), OwnerType::Alliance);
      assert_eq!(OwnerType::from_esi("corporation"), OwnerType::Corporation);
      assert_eq!(OwnerType::from_esi("faction"), OwnerType::Faction);
      assert_eq!(OwnerType::from_esi("eve_server"), OwnerType::EveServer);
      assert_eq!(OwnerType::from_esi("pod"), OwnerType::Pod);
      assert_eq!(OwnerType::from_esi("character"), OwnerType::Character);
      assert_eq!(OwnerType::from_esi("anything else"), OwnerType::Character);
    }

    #[test]
    fn only_org_owners_are_respondable() {
      assert!(OwnerType::Alliance.respondable());
      assert!(OwnerType::Corporation.respondable());
      assert!(OwnerType::Faction.respondable());
      assert!(!OwnerType::Character.respondable());
      assert!(!OwnerType::EveServer.respondable());
      assert!(!OwnerType::Pod.respondable());
    }
  }

  mod response {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_exposes_a_color_and_labels_for_every_response() {
      for response in RESPONSES {
        let _ = response.color();
        assert!(!response.label().is_empty());
        assert!(!response.pill_label().is_empty());
        assert!(!response.as_esi().is_empty());
      }
    }

    #[test]
    fn it_round_trips_through_esi_strings() {
      for response in RESPONSES {
        assert_eq!(Response::from_esi(response.as_esi()), response);
      }
      assert_eq!(Response::from_esi("unknown"), Response::NotResponded);
    }
  }
}
