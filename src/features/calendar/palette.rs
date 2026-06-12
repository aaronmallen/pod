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

  pub fn label(self) -> &'static str {
    match self {
      OwnerType::Alliance => "Alliance",
      OwnerType::Character => "Personal",
      OwnerType::Corporation => "Corporation",
      OwnerType::EveServer => "EVE Server",
      OwnerType::Faction => "Faction",
      OwnerType::Pod => "Pod \u{00B7} derived",
    }
  }

  pub fn respondable(self) -> bool {
    matches!(self, OwnerType::Alliance | OwnerType::Corporation | OwnerType::Faction)
  }

  pub fn short_label(self) -> &'static str {
    match self {
      OwnerType::Alliance => "Alliance",
      OwnerType::Character => "Personal",
      OwnerType::Corporation => "Corp",
      OwnerType::EveServer => "EVE",
      OwnerType::Faction => "Faction",
      OwnerType::Pod => "Pod",
    }
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

  pub fn label(self) -> &'static str {
    match self {
      Response::Accepted => "Accepted",
      Response::Declined => "Declined",
      Response::NotResponded => "No reply",
      Response::Tentative => "Tentative",
    }
  }

  pub fn pill_label(self) -> &'static str {
    match self {
      Response::Accepted => "Going",
      Response::Declined => "Can't",
      Response::NotResponded => "No reply",
      Response::Tentative => "Maybe",
    }
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
