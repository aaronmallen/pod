use iced::{
  Element, Length,
  alignment::Horizontal,
  widget::{Column, Space, text},
};

use crate::ui::{
  components::progress_bar::progress_bar,
  style::{color, spacing, typography},
};

const BAR_HEIGHT: f32 = 3.0;

pub fn status_message<'a, M>(label: &str, progress: Option<f32>, align: Horizontal) -> Element<'a, M>
where
  M: 'a,
{
  let label_el = text(label.to_string())
    .width(Length::Fill)
    .align_x(align)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let bar: Element<'a, M> = match progress {
    Some(value) => progress_bar(value, color::accent(), BAR_HEIGHT),
    None => Space::new()
      .width(Length::Fill)
      .height(Length::Fixed(BAR_HEIGHT))
      .into(),
  };

  Column::with_children(vec![label_el.into(), bar])
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .spacing(spacing::SPACE_4_5)
    .into()
}
