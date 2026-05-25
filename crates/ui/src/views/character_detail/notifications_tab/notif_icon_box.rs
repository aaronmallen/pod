//! Notification icon box: category-coloured icon in a rounded container.

use iced::{Background, Border, Color, Element, widget::container};

use crate::{components::Icon, style::color, views::character_detail::Message};

/// Builder for a notification category icon box.
pub struct Component {
  category: String,
}

impl Component {
  /// Creates a new icon box for the given notification category.
  pub fn new(category: impl Into<String>) -> Self {
    Self {
      category: category.into(),
    }
  }

  /// Renders the icon box.
  pub fn render(self) -> Element<'static, Message> {
    let cat_color = category_color(&self.category);
    let icon_el = category_icon(&self.category)
      .size(16.0)
      .color(cat_color)
      .render::<Message>();
    container(icon_el)
      .width(28.0)
      .height(28.0)
      .align_x(iced::alignment::Horizontal::Center)
      .align_y(iced::alignment::Vertical::Center)
      .style(move |_| container::Style {
        background: Some(Background::Color(color::state::HOVER_OVERLAY)),
        border: Border {
          color: cat_color,
          radius: 6.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn category_color_combat_corp(category: &str) -> Option<Color> {
  match category {
    "war" | "incursion" | "combat" => Some(color::status::DANGER),
    "corp" | "alliance" | "fw" => Some(color::status::CAUTION),
    _ => None,
  }
}

fn category_color_other(category: &str) -> Color {
  match category {
    "structure" | "mission" | "industry" | "standing" => color::accent::PLASMA,
    "market" | "insurance" | "reward" => color::status::ONLINE,
    "contract" | "clone" | "contact" => color::accent::COBALT,
    _ => color::text::SECONDARY,
  }
}

pub(super) fn category_color(category: &str) -> Color {
  category_color_combat_corp(category).unwrap_or_else(|| category_color_other(category))
}

fn category_icon(category: &str) -> Icon {
  category_icon_combat(category)
    .or_else(|| category_icon_corp(category))
    .or_else(|| category_icon_operational(category))
    .or_else(|| category_icon_financial(category))
    .unwrap_or_else(Icon::notif_system)
}

fn category_icon_combat(category: &str) -> Option<Icon> {
  match category {
    "combat" => Some(Icon::notif_combat()),
    "fw" => Some(Icon::notif_fw()),
    "incursion" => Some(Icon::notif_incursion()),
    "war" => Some(Icon::notif_war()),
    _ => None,
  }
}

fn category_icon_corp(category: &str) -> Option<Icon> {
  match category {
    "alliance" => Some(Icon::notif_alliance()),
    "contact" => Some(Icon::notif_contact()),
    "corp" => Some(Icon::notif_corp()),
    "standing" => Some(Icon::notif_standing()),
    _ => None,
  }
}

fn category_icon_financial(category: &str) -> Option<Icon> {
  match category {
    "contract" => Some(Icon::notif_contract()),
    "insurance" => Some(Icon::notif_insurance()),
    "market" => Some(Icon::notif_market()),
    "reward" => Some(Icon::notif_reward()),
    _ => None,
  }
}

fn category_icon_operational(category: &str) -> Option<Icon> {
  match category {
    "clone" => Some(Icon::notif_clone()),
    "industry" => Some(Icon::notif_industry()),
    "mission" => Some(Icon::notif_mission()),
    "structure" => Some(Icon::notif_structure()),
    _ => None,
  }
}
