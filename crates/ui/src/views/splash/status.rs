use iced::{Element, Length};

use crate::{
  components::{ProgressBar, splash_status},
  style::color,
};

pub struct Component<'a> {
  label: &'a str,
  progress: Option<f32>,
}

impl<'a> Component<'a> {
  pub fn new(label: &'a str) -> Self {
    Self {
      label,
      progress: None,
    }
  }

  pub fn progress(mut self, value: f32) -> Self {
    self.progress = Some(value);
    self
  }

  pub fn render<MSG: 'static>(self) -> Element<'a, MSG> {
    let label_el = splash_status::Component::new(self.label).render::<MSG>();

    let bar: Element<'a, MSG> = match self.progress {
      Some(p) => ProgressBar::new(p)
        .fill_color(color::text::PRIMARY)
        .height(2.0)
        .radius(1.0)
        .total_width(Length::Fixed(240.0))
        .render(),
      None => iced::widget::Space::new().width(240.0).height(2.0).into(),
    };

    iced::widget::column([label_el, bar])
      .align_x(iced::alignment::Horizontal::Center)
      .spacing(14)
      .into()
  }
}
