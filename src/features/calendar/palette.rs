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
