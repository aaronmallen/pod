use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Id, button, container, row, text, text_input},
};

use crate::{
  components,
  style::{color, typography},
};

/// Rounded search input with a leading icon and standard transparent text field.
pub struct SearchBox<'a, MSG: Clone + 'static> {
  placeholder: &'a str,
  value: &'a str,
  on_input: fn(String) -> MSG,
  width: Length,
  height: f32,
  font_size: f32,
  horizontal_padding: f32,
  icon_size: f32,
  icon_spacing: f32,
  input_id: Option<Id>,
  right_element: Option<Element<'a, MSG>>,
  background: Color,
}

impl<'a, MSG: Clone + 'static> SearchBox<'a, MSG> {
  pub fn new(placeholder: &'a str, value: &'a str, on_input: fn(String) -> MSG) -> Self {
    Self {
      placeholder,
      value,
      on_input,
      width: Length::Fill,
      height: 32.0,
      font_size: 13.0,
      horizontal_padding: 10.0,
      icon_size: 14.0,
      icon_spacing: 8.0,
      input_id: None,
      right_element: None,
      background: color::surface::SUNKEN,
    }
  }

  pub fn width(mut self, width: Length) -> Self {
    self.width = width;
    self
  }

  pub fn height(mut self, height: f32) -> Self {
    self.height = height;
    self
  }

  pub fn font_size(mut self, size: f32) -> Self {
    self.font_size = size;
    self
  }

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

  pub fn right_element(mut self, el: Element<'a, MSG>) -> Self {
    self.right_element = Some(el);
    self
  }

  pub fn background(mut self, color: Color) -> Self {
    self.background = color;
    self
  }

  pub fn render(self) -> Element<'a, MSG>
  where
    MSG: 'a,
  {
    let font_size = self.font_size;
    let on_input = self.on_input;

    let mut input = text_input(self.placeholder, self.value)
      .on_input(on_input)
      .font(typography::body::REGULAR)
      .size(font_size)
      .style(|_, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      })
      .padding(Padding::ZERO)
      .width(Length::Fill);

    if let Some(id) = self.input_id {
      input = input.id(id);
    }

    let mut children: Vec<Element<'_, MSG>> = vec![
      components::Icon::search()
        .size(self.icon_size)
        .color(color::text::SECONDARY)
        .render::<MSG>(),
      input.into(),
    ];
    if let Some(right) = self.right_element {
      children.push(right);
    }

    let bg = self.background;
    container(
      row(children)
        .spacing(self.icon_spacing)
        .align_y(iced::alignment::Vertical::Center),
    )
    .height(self.height)
    .width(self.width)
    .align_y(iced::alignment::Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: self.horizontal_padding,
      right: self.horizontal_padding,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}

/// Row of pill buttons. `T: PartialEq + Clone`, options as `(label, value)` pairs.
pub struct PillFilter<'a, T, MSG> {
  options: Vec<(&'a str, T)>,
  selected: &'a T,
  on_select: fn(T) -> MSG,
}

impl<'a, T, MSG> PillFilter<'a, T, MSG>
where
  T: PartialEq + Clone + 'static,
  MSG: Clone + 'static,
{
  pub fn new(options: Vec<(&'a str, T)>, selected: &'a T, on_select: fn(T) -> MSG) -> Self {
    Self {
      options,
      selected,
      on_select,
    }
  }

  pub fn render(self) -> Element<'a, MSG>
  where
    MSG: 'a,
  {
    let on_select = self.on_select;
    let btns: Vec<Element<'_, MSG>> = self
      .options
      .into_iter()
      .map(|(label, value)| {
        let is_active = &value == self.selected;
        let v = value.clone();
        button(
          text(label)
            .font(typography::mono::REGULAR)
            .size(10.0)
            .style(move |_: &Theme| iced::widget::text::Style {
              color: Some(if is_active {
                color::accent::PLASMA
              } else {
                color::text::SECONDARY
              }),
            }),
        )
        .padding(Padding {
          top: 6.0,
          bottom: 6.0,
          left: 12.0,
          right: 12.0,
        })
        .on_press(on_select(v))
        .style(move |_, _| button::Style {
          background: if is_active {
            Some(Background::Color(color::accent::PLASMA_SUBTLE))
          } else {
            None
          },
          border: Border {
            color: Color::TRANSPARENT,
            radius: 0.0.into(),
            width: 0.0,
          },
          text_color: if is_active {
            color::accent::PLASMA
          } else {
            color::text::SECONDARY
          },
          ..button::Style::default()
        })
        .into()
      })
      .collect();
    container(row(btns))
      .style(|_| container::Style {
        border: Border {
          color: color::border::SUBTLE,
          radius: 6.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}
