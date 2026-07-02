use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Id, container, row, text_input},
};

use crate::ui::{
  components::icon::Icon,
  style::{color, radius, typography},
};

const DEFAULT_FONT_SIZE: f32 = 13.0;
const DEFAULT_HEIGHT: f32 = 36.0;
const DEFAULT_HORIZONTAL_PADDING: f32 = 12.0;
const DEFAULT_ICON_SIZE: f32 = 14.0;
const DEFAULT_ICON_SPACING: f32 = 8.0;
const DEFAULT_PADDING: f32 = 8.0;
const DEFAULT_RADIUS: f32 = 5.0;

pub struct TextInput<'a, M: Clone + 'static> {
  background: Color,
  font_size: f32,
  height: f32,
  horizontal_padding: f32,
  icon_size: f32,
  icon_spacing: f32,
  input_id: Option<Id>,
  leading_icon: Option<Icon>,
  on_input: Box<dyn Fn(String) -> M + 'a>,
  on_submit: Option<M>,
  padding: f32,
  placeholder: &'a str,
  trailing: Option<Element<'a, M>>,
  value: &'a str,
  width: Length,
}

impl<'a, M: Clone + 'static> TextInput<'a, M> {
  pub fn new(placeholder: &'a str, value: &'a str, on_input: impl Fn(String) -> M + 'a) -> Self {
    Self {
      background: Color::TRANSPARENT,
      font_size: DEFAULT_FONT_SIZE,
      height: DEFAULT_HEIGHT,
      horizontal_padding: DEFAULT_HORIZONTAL_PADDING,
      icon_size: DEFAULT_ICON_SIZE,
      icon_spacing: DEFAULT_ICON_SPACING,
      input_id: None,
      leading_icon: None,
      on_input: Box::new(on_input),
      on_submit: None,
      padding: DEFAULT_PADDING,
      placeholder,
      trailing: None,
      value,
      width: Length::Fill,
    }
  }

  pub fn background(mut self, color: Color) -> Self {
    self.background = color;
    self
  }

  pub fn font_size(mut self, size: f32) -> Self {
    self.font_size = size;
    self
  }

  #[expect(dead_code)]
  pub fn height(mut self, height: f32) -> Self {
    self.height = height;
    self
  }

  #[expect(dead_code)]
  pub fn horizontal_padding(mut self, padding: f32) -> Self {
    self.horizontal_padding = padding;
    self
  }

  pub fn icon_size(mut self, size: f32) -> Self {
    self.icon_size = size;
    self
  }

  pub fn icon_spacing(mut self, spacing: f32) -> Self {
    self.icon_spacing = spacing;
    self
  }

  pub fn input_id(mut self, id: Id) -> Self {
    self.input_id = Some(id);
    self
  }

  pub fn leading_icon(mut self, icon: Icon) -> Self {
    self.leading_icon = Some(icon);
    self
  }

  pub fn on_submit(mut self, message: M) -> Self {
    self.on_submit = Some(message);
    self
  }

  pub fn padding(mut self, padding: f32) -> Self {
    self.padding = padding;
    self
  }

  pub fn render(self) -> Element<'a, M>
  where
    M: 'a,
  {
    if self.leading_icon.is_some() || self.trailing.is_some() {
      self.render_wrapped()
    } else {
      self.render_bare()
    }
  }

  pub fn trailing(mut self, element: Element<'a, M>) -> Self {
    self.trailing = Some(element);
    self
  }

  pub fn width(mut self, width: Length) -> Self {
    self.width = width;
    self
  }

  fn render_bare(self) -> Element<'a, M>
  where
    M: 'a,
  {
    let mut input = text_input(self.placeholder, self.value)
      .on_input(self.on_input)
      .font(typography::body::REGULAR)
      .size(self.font_size)
      .padding(Padding::from(self.padding))
      .width(self.width)
      .style(style());

    if let Some(id) = self.input_id {
      input = input.id(id);
    }

    if let Some(message) = self.on_submit {
      input = input.on_submit(message);
    }

    input.into()
  }

  fn render_wrapped(self) -> Element<'a, M>
  where
    M: 'a,
  {
    let background = self.background;

    let mut input = text_input(self.placeholder, self.value)
      .on_input(self.on_input)
      .font(typography::body::REGULAR)
      .size(self.font_size)
      .padding(Padding::ZERO)
      .width(Length::Fill)
      .style(inner_style());

    if let Some(id) = self.input_id {
      input = input.id(id);
    }

    if let Some(message) = self.on_submit {
      input = input.on_submit(message);
    }

    let mut children: Vec<Element<'a, M>> = Vec::new();
    if let Some(icon) = self.leading_icon {
      children.push(icon.size(self.icon_size).color(color::text::secondary()).render::<M>());
    }
    children.push(input.into());
    if let Some(trailing) = self.trailing {
      children.push(trailing);
    }

    container(row(children).spacing(self.icon_spacing).align_y(Vertical::Center))
      .width(self.width)
      .height(self.height)
      .align_y(Vertical::Center)
      .padding(Padding {
        bottom: 0.0,
        left: self.horizontal_padding,
        right: self.horizontal_padding,
        top: 0.0,
      })
      .style(move |_| container::Style {
        background: Some(Background::Color(background)),
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.1),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}

pub fn inner_style() -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
  |_, _| text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    selection: color::accent_muted(),
    value: color::text::PRIMARY,
  }
}

pub fn style() -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
  |_, _| text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      radius: DEFAULT_RADIUS.into(),
      width: 1.0,
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    selection: color::accent_muted(),
    value: color::text::PRIMARY,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug, Eq, PartialEq)]
  enum Msg {
    Changed(String),
  }

  mod render {
    use super::*;

    #[test]
    fn it_builds_a_bare_input() {
      let _el: Element<'_, Msg> = TextInput::new("Name", "", Msg::Changed).render();
    }

    #[test]
    fn it_builds_a_bare_input_with_options() {
      let _el: Element<'_, Msg> = TextInput::new("Name", "value", Msg::Changed)
        .font_size(14.0)
        .padding(6.0)
        .width(Length::Fixed(200.0))
        .render();
    }

    #[test]
    fn it_builds_a_wrapped_input_with_a_leading_icon() {
      let _el: Element<'_, Msg> = TextInput::new("Search", "", Msg::Changed)
        .leading_icon(Icon::search())
        .render();
    }

    #[test]
    fn it_builds_a_wrapped_input_with_a_trailing_element() {
      let trailing: Element<'_, Msg> = Icon::filter().render();

      let _el: Element<'_, Msg> = TextInput::new("Search", "query", Msg::Changed)
        .leading_icon(Icon::search())
        .background(color::surface::SUNKEN)
        .width(Length::Fixed(240.0))
        .trailing(trailing)
        .render();
    }
  }
}
