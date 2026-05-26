//! Settings sidebar: category navigation pane.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::{Category, Message, State, features_tab};
use crate::style::{color, radius, spacing, typography};

pub(super) fn render_categories_pane(state: &State) -> Element<'_, Message> {
  let enabled = state.features.enabled_count();
  let total = features_tab::State::total_count();
  let label = text("Categories").size(9.0).color(color::text::SECONDARY);

  let features_row = categories_item_row(
    "Features",
    Some(format!("{enabled}/{total}")),
    state.active_category == Category::Features,
    Message::CategorySelected(Category::Features),
  );
  let storage_row = categories_item_row(
    "Storage",
    None,
    state.active_category == Category::Storage,
    Message::CategorySelected(Category::Storage),
  );
  let tags_row = categories_item_row(
    "Tags",
    Some(state.tags.colored_count().to_string()),
    state.active_category == Category::Tags,
    Message::CategorySelected(Category::Tags),
  );

  let categories_col: Element<'_, Message> = column([
    container(label)
      .padding(Padding {
        top: 18.0,
        bottom: 10.0,
        left: spacing::SPACE_1 + 2.0,
        right: 0.0,
      })
      .into(),
    features_row,
    storage_row,
    tags_row,
  ])
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  })
  .into();

  let right_border = container(Space::new().width(1.0).height(Length::Fill))
    .width(1.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });

  row([
    container(categories_col)
      .width(220.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        ..container::Style::default()
      })
      .into(),
    right_border.into(),
  ])
  .into()
}

fn categories_active_indicator() -> Element<'static, Message> {
  container(
    container(Space::new())
      .width(2.0)
      .height(24.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: iced::border::Radius {
            top_left: 0.0,
            top_right: radius::SUBTLE,
            bottom_right: radius::SUBTLE,
            bottom_left: 0.0,
          },
          ..Border::default()
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(iced::alignment::Horizontal::Left)
  .align_y(Vertical::Center)
  .into()
}

fn item_label_el(label: impl ToString, is_active: bool) -> Element<'static, Message> {
  let col = if is_active {
    color::text::PRIMARY
  } else {
    color::text::SECONDARY
  };
  text(label.to_string())
    .size(13.0)
    .style(move |_| iced::widget::text::Style {
      color: Some(col),
    })
    .into()
}

fn item_badge_el(badge: Option<String>, is_active: bool) -> Element<'static, Message> {
  let col = if is_active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  match badge {
    Some(b) => text(b)
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(col),
      })
      .into(),
    None => Space::new().into(),
  }
}

fn item_container_style(is_active: bool) -> impl Fn(&iced::Theme) -> container::Style {
  move |_| container::Style {
    background: if is_active {
      Some(Background::Color(color::accent::PLASMA_SUBTLE))
    } else {
      None
    },
    border: Border {
      radius: radius::CHIP.into(),
      ..Border::default()
    },
    ..container::Style::default()
  }
}

fn categories_item_row(
  label: impl ToString,
  badge: Option<String>,
  is_active: bool,
  msg: Message,
) -> Element<'static, Message> {
  let inner = container(
    row([
      item_label_el(label, is_active),
      Space::new().width(Length::Fill).into(),
      item_badge_el(badge, is_active),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    }),
  )
  .width(Length::Fill)
  .style(item_container_style(is_active));

  let indicator: Element<'static, Message> = if is_active {
    categories_active_indicator()
  } else {
    Space::new().width(Length::Fill).height(Length::Fill).into()
  };

  button(iced::widget::stack([inner.into(), indicator]).width(Length::Fill))
    .padding(Padding::ZERO)
    .on_press(msg)
    .style(|_, _| button::Style::default())
    .into()
}
