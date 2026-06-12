use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, Space, button, text},
};

use super::super::Message;
use crate::ui::style::{color, spacing, typography};

const MASTERY_LABELS: [&str; 5] = ["I", "II", "III", "IV", "V"];
const PROFICIENCY_LABELS: [&str; 4] = ["Basic", "Std", "Adv", "Elite"];

pub(in crate::features::skill_plan_editor) fn ship_row<'a>(
  ship_id: i64,
  name: &'a str,
  selected_tier: u8,
) -> Element<'a, Message> {
  let chips: Vec<Element<'a, Message>> = MASTERY_LABELS
    .iter()
    .enumerate()
    .map(|(idx, label)| {
      let tier = (idx + 1) as u8;
      chip(
        label,
        tier == selected_tier,
        Message::PickerShipMasteryChanged(ship_id, tier),
      )
    })
    .collect();

  item_row(name, chips, Message::PickerShipSelected(ship_id, selected_tier))
}

pub(in crate::features::skill_plan_editor) fn cert_row<'a>(
  cert_id: i64,
  name: &'a str,
  selected_prof: usize,
) -> Element<'a, Message> {
  let chips: Vec<Element<'a, Message>> = PROFICIENCY_LABELS
    .iter()
    .enumerate()
    .map(|(idx, label)| {
      chip(
        label,
        idx == selected_prof,
        Message::PickerCertProficiencyChanged(cert_id, idx),
      )
    })
    .collect();

  item_row(name, chips, Message::PickerCertSelected(cert_id, selected_prof))
}

pub(in crate::features::skill_plan_editor) fn module_row<'a>(module_id: i64, name: &'a str) -> Element<'a, Message> {
  item_row(name, Vec::new(), Message::PickerModuleSelected(module_id))
}

fn item_row<'a>(name: &'a str, chips: Vec<Element<'a, Message>>, add: Message) -> Element<'a, Message> {
  let label = text(name)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .width(Length::Fill)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let mut children: Vec<Element<'a, Message>> = vec![label.into()];
  if !chips.is_empty() {
    children.push(Row::with_children(chips).spacing(2.0).align_y(Vertical::Center).into());
    children.push(Space::new().width(spacing::SPACE_2).into());
  }
  children.push(add_button(add));

  Row::with_children(children)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 30.0,
      right: 12.0,
    })
    .width(Length::Fill)
    .into()
}

fn add_button<'a>(on_press: Message) -> Element<'a, Message> {
  button(
    text("Add")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
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
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.15))),
      border: Border {
        color: color::accent::PLASMA,
        radius: 4.0.into(),
        width: 1.0,
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  })
  .into()
}

fn chip<'a>(label: &'a str, active: bool, on_press: Message) -> Element<'a, Message> {
  let (text_color, border_color, fill): (Color, Color, Option<Background>) = if active {
    (
      color::accent::PLASMA,
      color::accent::PLASMA,
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.25))),
    )
  } else {
    (
      color::text::secondary(),
      color::with_alpha(color::text::PRIMARY, 0.2),
      None,
    )
  };

  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(text_color),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 5.0,
    right: 5.0,
  })
  .on_press(on_press)
  .style(move |_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: if hover && !active {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
      } else {
        fill
      },
      border: Border {
        color: border_color,
        radius: 3.0.into(),
        width: 1.0,
      },
      text_color,
      ..button::Style::default()
    }
  })
  .into()
}
