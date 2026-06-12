use iced::{
  Element, Length,
  alignment::Horizontal,
  widget::{Column, Space, container, text},
};

use crate::ui::{
  components::progress_bar::progress_bar,
  style::{color, spacing, typography},
};

const BAR_WIDTH: f32 = 240.0;
const BAR_HEIGHT: f32 = 2.0;

pub fn status_message<'a, M>(label: &str, progress: Option<f32>) -> Element<'a, M>
where
  M: 'a,
{
  let label_el = text(label.to_string())
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let bar: Element<'a, M> = match progress {
    Some(value) => container(progress_bar(value, color::accent::PLASMA, BAR_HEIGHT))
      .width(Length::Fixed(BAR_WIDTH))
      .into(),
    None => Space::new()
      .width(Length::Fixed(BAR_WIDTH))
      .height(Length::Fixed(BAR_HEIGHT))
      .into(),
  };

  Column::with_children(vec![label_el.into(), bar])
    .align_x(Horizontal::Center)
    .spacing(spacing::SPACE_3)
    .into()
}
