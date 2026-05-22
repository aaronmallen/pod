use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{Space, button, column, container, row, text},
};

use crate::style::{color, spacing, typography as font};

/// A single tab entry.
#[derive(Clone, Debug)]
pub struct TabItem {
  pub label: String,
  pub count: Option<usize>,
}

/// Horizontal tab strip with an active-tab plasma underline.
pub struct Component {
  items: Vec<TabItem>,
  active: usize,
}

impl Component {
  pub fn new(items: Vec<TabItem>) -> Self {
    Self {
      items,
      active: 0,
    }
  }

  pub fn active(mut self, index: usize) -> Self {
    self.active = index;
    self
  }

  pub fn render<'a, MSG: 'a + Clone>(self, on_select: impl Fn(usize) -> MSG + 'a) -> Element<'a, MSG> {
    let active = self.active;
    let tabs: Vec<Element<'a, MSG>> = self
      .items
      .into_iter()
      .enumerate()
      .map(|(i, item)| {
        let msg = on_select(i);
        tab_item(i, item, i == active, msg)
      })
      .collect();

    let tab_row = row(tabs).spacing(0.0);

    let tabs_container = container(tab_row)
      .width(Length::Fill)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      });

    let bottom_rule = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      });

    column([tabs_container.into(), bottom_rule.into()])
      .width(Length::Fill)
      .into()
  }
}

fn tab_item<'a, MSG: 'a + Clone>(_index: usize, item: TabItem, is_active: bool, msg: MSG) -> Element<'a, MSG> {
  let label_row = tab_label_row(&item, is_active);
  let underline = tab_underline(is_active);
  let btn = button(label_row)
    .padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: spacing::SPACE_5,
      right: spacing::SPACE_5,
    })
    .on_press(msg)
    .style(move |_, status| {
      let text_color = match (is_active, status) {
        (true, _) => color::text::PRIMARY,
        (false, button::Status::Hovered | button::Status::Pressed) => color::text::PRIMARY,
        _ => color::text::SECONDARY,
      };
      button::Style {
        background: None,
        border: Border::default(),
        text_color,
        ..button::Style::default()
      }
    });
  column([btn.into(), underline]).width(Length::Shrink).into()
}

fn tab_label_row<'a, MSG: 'a>(item: &TabItem, is_active: bool) -> iced::widget::Row<'a, MSG> {
  let label_el = text(item.label.clone())
    .font(font::body::MEDIUM)
    .size(13.0)
    .style(move |_| iced::widget::text::Style {
      color: Some(if is_active {
        color::text::PRIMARY
      } else {
        color::text::SECONDARY
      }),
    });
  let mut children: Vec<Element<'a, MSG>> = vec![label_el.into()];
  if let Some(count) = item.count {
    let badge_color = if is_active {
      color::accent::PLASMA
    } else {
      color::text::TERTIARY
    };
    let count_el = text(count.to_string())
      .font(font::mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(badge_color),
      });
    children.push(Space::new().width(6.0).into());
    children.push(count_el.into());
  }
  row(children).align_y(iced::alignment::Vertical::Center)
}

fn tab_underline<'a, MSG: 'a>(is_active: bool) -> Element<'a, MSG> {
  let underline_color = if is_active {
    color::accent::PLASMA
  } else {
    Color::TRANSPARENT
  };
  container(Space::new().width(Length::Fill).height(2.0))
    .width(Length::Fill)
    .height(2.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(underline_color)),
      ..container::Style::default()
    })
    .into()
}
