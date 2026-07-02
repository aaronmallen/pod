use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use super::{Filters, module_type_picker::modal_selected_label, stat_ranges};
use crate::{
  features::assets::{Message, State},
  ui::{
    components::{count_badge::count_badge, eyebrow::eyebrow, icon::Icon, rule},
    style::{color, radius, spacing, typography},
  },
};

pub(in crate::features::assets) fn rail(state: &State) -> Element<'_, Message> {
  let filters = state.abyssal_filters();

  let body = scrollable(
    Column::with_children(vec![module_type_section(filters), stat_ranges_section(state)]).width(Length::Fill),
  )
  .style(crate::ui::style::control::scrollbar)
  .height(Length::Fill);

  Column::with_children(vec![header(filters.is_active()), rule::horizontal(), body.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn header(active: bool) -> Element<'static, Message> {
  let mut items: Vec<Element<'static, Message>> = vec![
    container(eyebrow(&t!("assets.abyssals.filters"), Some(color::text::tertiary())))
      .width(Length::Fill)
      .into(),
  ];
  if active {
    items.push(reset_button());
  }

  container(Row::with_children(items).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn reset_button() -> Element<'static, Message> {
  button(
    text(t!("assets.abyssals.reset").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT / 2.0,
    right: spacing::UNIT,
    bottom: spacing::UNIT / 2.0,
    left: spacing::UNIT,
  })
  .on_press(Message::AbyssalFilterReset)
  .style(|_, _| button::Style {
    text_color: color::text::tertiary(),
    ..button::Style::default()
  })
  .into()
}

fn module_type_section(filters: &Filters) -> Element<'static, Message> {
  let selected = filters.source_type_id;
  let mut items: Vec<Element<'static, Message>> = vec![picker_trigger(selected.is_some())];
  if let Some(type_id) = selected {
    let name = modal_selected_label(type_id).unwrap_or_else(|| t!("assets.abyssals.module_unknown").into_owned());
    items.push(Space::new().height(spacing::SPACE_2_5).into());
    items.push(selected_chip(name));
  }
  section(
    &t!("assets.abyssals.module_type"),
    Column::with_children(items).width(Length::Fill).into(),
  )
}

fn picker_trigger(has_filter: bool) -> Element<'static, Message> {
  let (border_color, label_color, icon_color, label) = if has_filter {
    (
      color::with_alpha(color::accent(), 0.35),
      color::text::PRIMARY,
      color::accent(),
      t!("assets.abyssals.edit_module_filter").into_owned(),
    )
  } else {
    (
      color::with_alpha(color::text::PRIMARY, 0.12),
      color::text::secondary(),
      color::text::tertiary(),
      t!("assets.abyssals.filter_by_module_type").into_owned(),
    )
  };
  let background = has_filter.then(|| Background::Color(color::with_alpha(color::accent(), 0.1)));

  let mut row_items: Vec<Element<'static, Message>> = vec![
    Icon::filter().size(14.0).color(icon_color).render::<Message>(),
    Space::new().width(spacing::SPACE_2_5).into(),
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .width(Length::Fill)
      .into(),
  ];
  if has_filter {
    row_items.push(count_badge(1, color::accent()));
  }

  button(Row::with_children(row_items).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .on_press(Message::AbyssalTypeModalOpened)
    .style(move |_, _| button::Style {
      background,
      border: Border {
        color: border_color,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: label_color,
      ..button::Style::default()
    })
    .into()
}

fn selected_chip(name: String) -> Element<'static, Message> {
  container(
    Row::with_children(vec![
      text(name)
        .font(typography::body::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      button(
        text("\u{00d7}")
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          }),
      )
      .padding(Padding {
        top: 0.0,
        right: spacing::UNIT / 2.0,
        bottom: 0.0,
        left: spacing::UNIT / 2.0,
      })
      .on_press(Message::AbyssalSourceTypeSelected(None))
      .style(|_, _| button::Style {
        text_color: color::text::secondary(),
        ..button::Style::default()
      })
      .into(),
    ])
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::UNIT,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn stat_ranges_section(state: &State) -> Element<'_, Message> {
  let panel = if state.abyssal_filters().source_type_id.is_some() {
    stat_ranges::panel(state)
  } else {
    stat_ranges::placeholder()
  };
  section(&t!("assets.abyssals.stat_ranges"), panel)
}

fn section<'a>(label: &str, content: Element<'a, Message>) -> Element<'a, Message> {
  Column::with_children(vec![
    container(
      Column::with_children(vec![
        eyebrow(label, Some(color::text::tertiary())),
        Space::new().height(spacing::SPACE_2_5).into(),
        content,
      ])
      .width(Length::Fill),
    )
    .padding(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into(),
    rule::horizontal(),
  ])
  .width(Length::Fill)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::assets::State;

  mod rail {
    use super::*;

    #[test]
    fn it_renders_the_rail_prompting_a_type_selection() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.set_abyssals_for_test(vec![], vec![], Filters::default(), false);

      let _el: Element<'_, Message> = rail(&state);
    }

    #[test]
    fn it_renders_the_rail_with_a_selected_type_and_reset() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      let filters = Filters {
        source_type_id: Some(47785),
        ..Filters::default()
      };
      state.set_abyssals_for_test(vec![], vec![], filters, false);

      let _el: Element<'_, Message> = rail(&state);
    }
  }
}
