use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, button, container, text},
};

use super::super::Message;
use crate::ui::style::{color, spacing, typography};

pub(in crate::features::skill_plan_editor) fn group_header(
  id: i64,
  name: &str,
  trained_count: usize,
  total_skills: usize,
  open: bool,
) -> Element<'_, Message> {
  let chevron = text(if open { "\u{25be}" } else { "\u{25b8}" })
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .width(Length::Fixed(12.0))
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let label = text(name.to_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let summary = text(format!("{trained_count}/{total_skills}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let inner = Row::with_children(vec![
    chevron.into(),
    container(label).width(Length::Fill).into(),
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
    .on_press(Message::PickerGroupToggled(id))
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
