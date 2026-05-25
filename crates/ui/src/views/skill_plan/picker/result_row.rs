//! Row renderers for skills, ships, modules, and certificates in picker tabs.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};
use pod_model::{Certificate, ItemTypeSummary};

use super::{super::Message, group_header::separator};
use crate::{
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::skills::skill_data::SkillDef,
};

/// Builder for a certificate result row with proficiency chip selector.
pub struct CertRow<'a> {
  cert: &'a Certificate,
  selected_prof: u8,
}

impl<'a> CertRow<'a> {
  /// Creates a new cert row for the given certificate and selected proficiency.
  pub fn new(cert: &'a Certificate, selected_prof: u8) -> Self {
    Self {
      cert,
      selected_prof,
    }
  }

  /// Renders the cert row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let cert_id = self.cert.id;
    let cert_name = self.cert.name.clone();

    let name_el = text(self.cert.name.clone())
      .font(body::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill);

    let add_btn = item_add_btn(Message::CertSelected(cert_id, cert_name, self.selected_prof));
    let chips = prof_chips(cert_id, self.selected_prof);

    let row_content = container(
      row([
        name_el.into(),
        row(chips).spacing(2.0).align_y(Vertical::Center).into(),
        Space::new().width(spacing::SPACE_2).into(),
        add_btn.into(),
      ])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_2),
    )
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 30.0,
      right: spacing::SPACE_3,
    })
    .width(Length::Fill);

    column([separator().into(), row_content.into()]).into()
  }
}

/// Builder for a module result row.
pub struct ModuleRow<'a> {
  module: &'a ItemTypeSummary,
}

impl<'a> ModuleRow<'a> {
  /// Creates a new module row for the given item type summary.
  pub fn new(module: &'a ItemTypeSummary) -> Self {
    Self {
      module,
    }
  }

  /// Renders the module row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let type_id = self.module.id;
    let mod_name = self.module.name.clone();

    let row_content = container(
      button(
        text(self.module.name.clone())
          .font(body::REGULAR)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .width(Length::Fill),
      )
      .width(Length::Fill)
      .padding(Padding {
        top: 7.0,
        bottom: 7.0,
        left: 30.0,
        right: spacing::SPACE_3,
      })
      .on_press(Message::ModuleSelected(type_id, mod_name))
      .style(|_, status| button::Style {
        background: match status {
          button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
          _ => None,
        },
        border: Border::default(),
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      }),
    )
    .width(Length::Fill);

    column([separator().into(), row_content.into()]).into()
  }
}

/// Builder for a ship result row with mastery chip selector.
pub struct ShipRow<'a> {
  selected_mastery: u8,
  ship: &'a ItemTypeSummary,
}

impl<'a> ShipRow<'a> {
  /// Creates a new ship row for the given item type summary and mastery level.
  pub fn new(ship: &'a ItemTypeSummary, selected_mastery: u8) -> Self {
    Self {
      selected_mastery,
      ship,
    }
  }

  /// Renders the ship row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let type_id = self.ship.id;
    let ship_name = self.ship.name.clone();

    let name_el = text(self.ship.name.clone())
      .font(body::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill);

    let add_btn = item_add_btn(Message::ShipSelected(type_id, ship_name, self.selected_mastery));
    let chips = mastery_chips(type_id, self.selected_mastery);

    let row_children: Vec<Element<'_, Message>> = vec![
      name_el.into(),
      row(chips).spacing(2.0).align_y(Vertical::Center).into(),
      Space::new().width(spacing::SPACE_2).into(),
      add_btn.into(),
    ];

    let row_content = container(row(row_children).align_y(Vertical::Center).spacing(spacing::SPACE_2))
      .padding(Padding {
        top: 7.0,
        bottom: 7.0,
        left: 30.0,
        right: spacing::SPACE_3,
      })
      .width(Length::Fill);

    column([separator().into(), row_content.into()]).into()
  }
}

/// Builder for a skill result row with trained/planned pip display.
pub struct SkillRow<'a> {
  planned_level: u8,
  skill: &'a SkillDef,
}

impl<'a> SkillRow<'a> {
  /// Creates a new skill row for the given skill definition and planned level.
  pub fn new(skill: &'a SkillDef, planned_level: u8) -> Self {
    Self {
      planned_level,
      skill,
    }
  }

  /// Renders the skill row into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let pip_row = skill_pip_row(self.skill, self.planned_level);

    let row_content = container(
      row([
        text(self.skill.name.clone())
          .font(body::REGULAR)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .width(Length::Fill)
          .into(),
        text(format!("\u{d7}{}", self.skill.rank))
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          })
          .into(),
        Space::new().width(spacing::SPACE_2).into(),
        pip_row.into(),
      ])
      .align_y(Vertical::Center),
    )
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 30.0,
      right: spacing::SPACE_3,
    })
    .width(Length::Fill);

    column([separator().into(), row_content.into()]).into()
  }
}

fn item_add_btn(on_press: Message) -> button::Button<'static, Message> {
  button(
    text("Add")
      .font(body::MEDIUM)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::accent::PLASMA_HIGHLIGHT)),
      _ => Some(Background::Color(Color::TRANSPARENT)),
    },
    border: Border {
      color: color::accent::PLASMA,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
}

fn level_chip(label: &'static str, is_active: bool, on_press: Message) -> Element<'static, Message> {
  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        }),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 5.0,
    right: 5.0,
  })
  .on_press(on_press)
  .style(move |_, status| button::Style {
    background: match (is_active, status) {
      (true, _) => Some(Background::Color(color::accent::PLASMA_ACTIVE)),
      (false, button::Status::Hovered | button::Status::Pressed) => {
        Some(Background::Color(color::state::HOVER_OVERLAY))
      }
      _ => None,
    },
    border: Border {
      color: if is_active {
        color::accent::PLASMA
      } else {
        color::border::SUBTLE
      },
      radius: 3.0.into(),
      width: 1.0,
    },
    text_color: if is_active {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    },
    ..button::Style::default()
  })
  .into()
}

fn mastery_chips(type_id: i32, selected_mastery: u8) -> Vec<Element<'static, Message>> {
  ["I", "II", "III", "IV", "V"]
    .into_iter()
    .enumerate()
    .map(|(i, label)| {
      let lv = (i + 1) as u8;
      let is_active = lv == selected_mastery;
      level_chip(label, is_active, Message::ShipMasteryChanged(type_id, lv))
    })
    .collect()
}

fn prof_chips(cert_id: i32, selected_prof: u8) -> Vec<Element<'static, Message>> {
  ["Basic", "Std", "Adv", "Elite"]
    .into_iter()
    .enumerate()
    .map(|(i, label)| {
      let prof = i as u8;
      let is_active = prof == selected_prof;
      level_chip(label, is_active, Message::CertProficiencyChanged(cert_id, prof))
    })
    .collect()
}

fn skill_pip(bg: Color, border_col: Color) -> iced::widget::Container<'static, Message> {
  container(Space::new())
    .width(10.0)
    .height(8.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: border_col,
        radius: 1.5.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
}

fn skill_pip_colors(trained: bool, planned: bool) -> (Color, Color) {
  if trained {
    (color::text::PRIMARY, color::text::PRIMARY)
  } else if planned {
    (color::accent::PLASMA_MUTED, color::accent::PLASMA_HALF)
  } else {
    (Color::TRANSPARENT, color::border::SUBTLE)
  }
}

fn skill_pip_row(skill: &SkillDef, planned_level: u8) -> iced::widget::Row<'static, Message> {
  let pips: Vec<Element<'_, Message>> = (1u8..=5)
    .map(|lv| {
      let trained = lv <= skill.level;
      let planned = !trained && lv <= planned_level;
      let (bg, border_col) = skill_pip_colors(trained, planned);
      let pip = skill_pip(bg, border_col);

      if trained {
        pip.into()
      } else {
        button(pip)
          .padding(0)
          .on_press(Message::SkillPicked(skill.name.to_string(), lv))
          .style(|_, status| button::Style {
            background: match status {
              button::Status::Hovered | button::Status::Pressed => {
                Some(Background::Color(color::accent::PLASMA_ACTIVE))
              }
              _ => None,
            },
            border: Border {
              color: Color::TRANSPARENT,
              radius: 1.5.into(),
              width: 0.0,
            },
            ..button::Style::default()
          })
          .into()
      }
    })
    .collect();

  row(pips).spacing(3.0).align_y(Vertical::Center)
}
