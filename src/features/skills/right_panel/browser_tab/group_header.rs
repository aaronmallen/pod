use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, button, container, text},
};

use super::{super::super::browse::GroupRow, Message};
use crate::ui::style::{color, spacing, typography};

fn fmt_group_sp(sp: i64) -> String {
  if sp >= 1_000_000 {
    format!("{:.2}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}K", sp as f64 / 1_000.0)
  } else {
    sp.to_string()
  }
}

pub fn group_header(group: &GroupRow, open: bool) -> Element<'_, Message> {
  let chevron = text(if open { "\u{25be}" } else { "\u{25b8}" })
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .width(Length::Fixed(12.0))
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let name = text(group.name.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let summary = text(format!(
    "{}/{} \u{00b7} {} SP",
    group.trained_count,
    group.total_skills,
    fmt_group_sp(group.total_sp)
  ))
  .font(typography::mono::REGULAR)
  .size(typography::size::XS)
  .style(|_| text::Style {
    color: Some(color::text::secondary()),
  });

  let inner = Row::with_children(vec![
    chevron.into(),
    container(name).width(Length::Fill).into(),
    summary.into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  button(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .on_press(Message::GroupToggled(group.id))
    .style(|_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: hover.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.03))),
        text_color: color::text::PRIMARY,
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.1),
          width: 1.0,
          ..Border::default()
        },
        ..button::Style::default()
      }
    })
    .into()
}
