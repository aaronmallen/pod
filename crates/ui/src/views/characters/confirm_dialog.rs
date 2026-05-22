use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Horizontal,
  widget::{Space, column, container, row, text},
};

use crate::{
  components,
  style::{color, radius, shadow, spacing, typography},
};

#[derive(Clone, Debug)]
pub enum Message {
  Confirmed,
  Dismissed,
}

pub struct Component {
  character_name: String,
  window_width: f32,
  window_height: f32,
}

impl Component {
  pub fn new(character_name: impl Into<String>) -> Self {
    Self {
      character_name: character_name.into(),
      window_width: 800.0,
      window_height: 600.0,
    }
  }

  pub fn window_size(mut self, width: f32, height: f32) -> Self {
    self.window_width = width;
    self.window_height = height;
    self
  }

  pub fn render(self) -> Element<'static, Message> {
    let dialog_width = (self.window_width - 48.0).min(420.0);
    let max_height = (self.window_height - 48.0).min(320.0);

    let header = components::PanelHeader::new("REMOVE CHARACTER").render();
    let body = render_body(&self.character_name);
    let footer = render_footer();

    let dialog = container(column([
      header,
      components::Separator::horizontal().render(),
      body,
      components::Separator::horizontal().render(),
      footer,
    ]))
    .width(Length::Fixed(dialog_width))
    .max_height(max_height)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      shadow: shadow::POPOVER,
      ..container::Style::default()
    });

    container(dialog)
      .align_x(Horizontal::Center)
      .center_y(Length::Fill)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }
}

fn render_body(character_name: &str) -> Element<'static, Message> {
  let title: Element<'static, Message> = text(format!("Remove {} from Pod?", character_name))
    .size(17.0)
    .font(typography::body::MEDIUM)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into();

  let body_text: Element<'static, Message> = text(
    "This unlinks the character from this app only. Their skills, assets \
    and ISK on the EVE servers are unaffected. You can re-add them later \
    via Add character.",
  )
  .size(13.0)
  .font(typography::body::REGULAR)
  .style(|_| text::Style {
    color: Some(color::text::SECONDARY),
  })
  .into();

  container(column([title, body_text]).spacing(spacing::SPACE_2))
    .padding(Padding {
      top: 20.0,
      bottom: 14.0,
      left: 20.0,
      right: 20.0,
    })
    .width(Length::Fill)
    .into()
}

fn render_footer() -> Element<'static, Message> {
  let cancel_btn =
    components::Button::ghost(text("Cancel").size(13.0).font(typography::body::MEDIUM)).on_press(Message::Dismissed);

  let remove_btn =
    components::Button::danger(text("Remove").size(13.0).font(typography::body::SEMIBOLD)).on_press(Message::Confirmed);

  container(
    row([
      Space::new().width(Length::Fill).into(),
      cancel_btn.into(),
      remove_btn.into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_3,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .width(Length::Fill)
  .into()
}
