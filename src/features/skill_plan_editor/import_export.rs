use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, row, text},
};
use serde::{Deserialize, Serialize};

use super::{IoPanel, Message};
use crate::{
  features::skills::optimizer::Attributes,
  ui::style::{color, radius, spacing, typography},
};

const DROPDOWN_WIDTH: f32 = 180.0;
const PROMPT_WIDTH: f32 = 440.0;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanFile {
  pub entries: Vec<PlanFileEntry>,
  #[serde(default)]
  pub remaps: Vec<PlanFileRemap>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanFileAttrs {
  pub charisma: u32,
  pub intelligence: u32,
  pub memory: u32,
  pub perception: u32,
  pub willpower: u32,
}

impl PlanFileAttrs {
  pub fn from_attributes(attrs: Attributes) -> Self {
    PlanFileAttrs {
      charisma: attrs.charisma,
      intelligence: attrs.intelligence,
      memory: attrs.memory,
      perception: attrs.perception,
      willpower: attrs.willpower,
    }
  }

  pub fn to_attributes(&self) -> Attributes {
    Attributes {
      charisma: self.charisma,
      intelligence: self.intelligence,
      memory: self.memory,
      perception: self.perception,
      willpower: self.willpower,
    }
  }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanFileEntry {
  pub name: String,
  #[serde(default)]
  pub note: String,
  #[serde(default)]
  pub priority: String,
  pub to_level: u8,
  pub type_id: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanFileRemap {
  #[serde(default)]
  pub after_index: Option<usize>,
  pub base: PlanFileAttrs,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Payload {
  Json(PlanFile),
  Text(Vec<(String, u8)>),
}

pub fn detect(raw: &str) -> Option<Payload> {
  if let Ok(plan) = serde_json::from_str::<PlanFile>(raw.trim()) {
    return Some(Payload::Json(plan));
  }
  let lines = parse_plan_text(raw);
  (!lines.is_empty()).then_some(Payload::Text(lines))
}

pub fn parse_plan_text(text: &str) -> Vec<(String, u8)> {
  text
    .lines()
    .filter_map(|line| {
      let line = line.trim();
      if line.is_empty() {
        return None;
      }
      let (name, level_token) = line.rsplit_once(char::is_whitespace)?;
      let name = name.trim();
      let level = parse_level(level_token)?;
      if name.is_empty() {
        return None;
      }
      Some((name.to_owned(), level))
    })
    .collect()
}

pub fn parse_level(token: &str) -> Option<u8> {
  let token = token.trim();
  if let Ok(n) = token.parse::<u8>() {
    return (1..=5).contains(&n).then_some(n);
  }
  match token.to_uppercase().as_str() {
    "I" => Some(1),
    "II" => Some(2),
    "III" => Some(3),
    "IV" => Some(4),
    "V" => Some(5),
    _ => None,
  }
}

pub fn to_json(plan: &PlanFile) -> String {
  serde_json::to_string_pretty(plan).unwrap_or_default()
}

pub(super) fn overlay<'a>(panel: &IoPanel) -> Element<'a, Message> {
  match panel {
    IoPanel::Export => dropdown_overlay(
      Horizontal::Right,
      export_trigger_offset(),
      vec![
        ("To clipboard", Message::ExportToClipboard),
        ("To file\u{2026}", Message::ExportToFile),
      ],
    ),
    IoPanel::Import => dropdown_overlay(
      Horizontal::Right,
      import_trigger_offset(),
      vec![
        ("From clipboard", Message::ImportFromClipboard),
        ("From file\u{2026}", Message::ImportFromFile),
      ],
    ),
    IoPanel::ImportPrompt => prompt_overlay(),
  }
}

fn export_trigger_offset() -> f32 {
  spacing::SPACE_3_5 + 70.0
}

fn import_trigger_offset() -> f32 {
  export_trigger_offset() + DROPDOWN_WIDTH + spacing::SPACE_2 * 3.0
}

fn dropdown_overlay<'a>(
  align: Horizontal,
  right_pad: f32,
  items: Vec<(&'static str, Message)>,
) -> Element<'a, Message> {
  let menu = column(
    items
      .into_iter()
      .map(|(label, msg)| menu_item(label, msg))
      .collect::<Vec<_>>(),
  )
  .width(Length::Fixed(DROPDOWN_WIDTH));

  let panel = container(menu).style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  let dismiss = button(Space::new().width(Length::Fill).height(Length::Fill))
    .on_press(Message::IoDismissed)
    .style(|_, _| button::Style {
      background: None,
      ..button::Style::default()
    });

  iced::widget::stack(vec![
    dismiss.into(),
    container(panel)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(align)
      .padding(Padding {
        top: 52.0,
        right: right_pad,
        ..Padding::ZERO
      })
      .into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn menu_item<'a>(label: &'a str, on_press: Message) -> Element<'a, Message> {
  button(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
  })
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
      }
      _ => None,
    },
    border: Border::default(),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn prompt_overlay<'a>() -> Element<'a, Message> {
  let body = column(vec![
    text("Import plan")
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(spacing::SPACE_2).into(),
    text("Replace the current plan, or append the imported skills to the end?")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().height(spacing::SPACE_6).into(),
    row(vec![
      ghost_btn("Cancel", Message::IoDismissed),
      Space::new().width(Length::Fill).into(),
      ghost_btn("Append", Message::ImportAppend),
      Space::new().width(spacing::SPACE_2).into(),
      primary_btn("Replace", Message::ImportReplace),
    ])
    .align_y(Vertical::Center)
    .into(),
  ])
  .width(Length::Fill);

  let card = container(body)
    .width(Length::Fixed(PROMPT_WIDTH))
    .padding(Padding::new(spacing::SPACE_6))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      ..container::Style::default()
    })
    .into()
}

fn ghost_btn<'a>(label: &'a str, on_press: Message) -> Element<'a, Message> {
  button(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
      }
      _ => None,
    },
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    text_color: color::text::secondary(),
    ..button::Style::default()
  })
  .into()
}

fn primary_btn<'a>(label: &'a str, on_press: Message) -> Element<'a, Message> {
  button(
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::surface::BASE),
      }),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
  })
  .on_press(on_press)
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::accent::PLASMA)),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    text_color: color::surface::BASE,
    ..button::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn attrs() -> PlanFileAttrs {
    PlanFileAttrs {
      charisma: 19,
      intelligence: 21,
      memory: 19,
      perception: 21,
      willpower: 19,
    }
  }

  fn sample_plan() -> PlanFile {
    PlanFile {
      entries: vec![
        PlanFileEntry {
          name: "Gunnery".to_owned(),
          note: "first".to_owned(),
          priority: "high".to_owned(),
          to_level: 4,
          type_id: 3300,
        },
        PlanFileEntry {
          name: "Small Hybrid Turret".to_owned(),
          note: String::new(),
          priority: "normal".to_owned(),
          to_level: 5,
          type_id: 3301,
        },
      ],
      remaps: vec![
        PlanFileRemap {
          after_index: None,
          base: attrs(),
        },
        PlanFileRemap {
          after_index: Some(0),
          base: attrs(),
        },
      ],
    }
  }

  mod detect {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_a_json_plan_losslessly() {
      let plan = sample_plan();
      let json = to_json(&plan);

      match detect(&json) {
        Some(Payload::Json(parsed)) => assert_eq!(parsed, plan),
        other => panic!("expected a JSON payload, got {other:?}"),
      }
    }

    #[test]
    fn it_preserves_entry_and_remap_order_on_round_trip() {
      let plan = sample_plan();
      let parsed: PlanFile = serde_json::from_str(&to_json(&plan)).unwrap();

      let ids: Vec<i64> = parsed.entries.iter().map(|e| e.type_id).collect();
      assert_eq!(ids, vec![3300, 3301]);
      let anchors: Vec<Option<usize>> = parsed.remaps.iter().map(|r| r.after_index).collect();
      assert_eq!(anchors, vec![None, Some(0)]);
    }

    #[test]
    fn it_falls_back_to_text_for_non_json_lines() {
      match detect("Gunnery V\nSmall Hybrid Turret 4") {
        Some(Payload::Text(lines)) => {
          assert_eq!(
            lines,
            vec![("Gunnery".to_owned(), 5), ("Small Hybrid Turret".to_owned(), 4)]
          );
        }
        other => panic!("expected a text payload, got {other:?}"),
      }
    }

    #[test]
    fn it_prefers_json_when_the_payload_is_valid_json() {
      let json = to_json(&sample_plan());

      assert!(matches!(detect(&json), Some(Payload::Json(_))));
    }

    #[test]
    fn it_returns_none_for_unparseable_input() {
      assert_eq!(detect(""), None);
      assert_eq!(detect("   \n  "), None);
      assert_eq!(detect("not a skill line"), None);
    }
  }

  mod parse_level {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_arabic_one_through_five() {
      assert_eq!(parse_level("1"), Some(1));
      assert_eq!(parse_level("5"), Some(5));
      assert_eq!(parse_level("0"), None);
      assert_eq!(parse_level("6"), None);
    }

    #[test]
    fn it_accepts_roman_numerals() {
      assert_eq!(parse_level("iv"), Some(4));
      assert_eq!(parse_level("V"), Some(5));
      assert_eq!(parse_level("vi"), None);
    }
  }

  mod attrs_round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_through_the_optimizer_type() {
      let attrs = Attributes {
        charisma: 17,
        intelligence: 27,
        memory: 18,
        perception: 20,
        willpower: 17,
      };

      assert_eq!(PlanFileAttrs::from_attributes(attrs).to_attributes(), attrs);
    }
  }
}
