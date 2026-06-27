use iced::{
  Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, container, text},
};

use crate::ui::{
  components::status::dot,
  style::{color, spacing, typography},
};

pub fn esi_status<'a, M>(connected: bool) -> Element<'a, M>
where
  M: 'a,
{
  let dot_color = if connected {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  container(
    Row::with_children(vec![
      dot(dot_color),
      text(t!("common.esi_status.label"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::dim()),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_3_5,
    bottom: 0.0,
    left: spacing::SPACE_3_5,
  })
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .into()
}
