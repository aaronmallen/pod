use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Row, Space, button, container, stack, text},
};

use crate::ui::style::{color, radius, typography};

const COUNT_SIZE: f32 = typography::size::XS_PLUS;
const LABEL_SIZE: f32 = typography::size::MD;
const TAB_CELL_PADDING_X: f32 = 12.0;
const TAB_LABEL_GAP: f32 = 8.0;
const UNDERLINE_HEIGHT: f32 = 2.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TabLayout {
  #[allow(dead_code)]
  Centered,
  Fill,
  #[default]
  Start,
}

impl TabLayout {
  fn align_x(self) -> Horizontal {
    match self {
      TabLayout::Centered => Horizontal::Center,
      TabLayout::Fill | TabLayout::Start => Horizontal::Left,
    }
  }

  fn cell_width(self) -> Length {
    match self {
      TabLayout::Centered | TabLayout::Fill => Length::Fill,
      TabLayout::Start => Length::Shrink,
    }
  }
}

pub struct Tab<'a, M> {
  pub count: String,
  pub label: &'a str,
  pub on_press: Option<M>,
  pub selected: bool,
}

pub fn tab_select_with<'a, M>(tabs: Vec<Tab<'a, M>>, layout: TabLayout) -> Element<'a, M>
where
  M: Clone + 'a,
{
  Row::with_children(
    tabs
      .into_iter()
      .map(|descriptor| tab(descriptor, layout))
      .collect::<Vec<_>>(),
  )
  .width(layout.cell_width())
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .into()
}

fn tab<'a, M>(descriptor: Tab<'a, M>, layout: TabLayout) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let Tab {
    count,
    label,
    on_press,
    selected,
  } = descriptor;

  let label_color = if selected {
    color::text::PRIMARY
  } else {
    color::text::SECONDARY
  };
  let count_color = if selected {
    color::accent::PLASMA
  } else {
    color::text::TERTIARY
  };

  let content = Row::with_children(vec![
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(LABEL_SIZE)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
    text(count)
      .font(typography::mono::REGULAR)
      .size(COUNT_SIZE)
      .style(move |_| text::Style {
        color: Some(count_color),
      })
      .into(),
  ])
  .spacing(TAB_LABEL_GAP)
  .align_y(Vertical::Center);

  let cell_padding = Padding {
    left: TAB_CELL_PADDING_X,
    right: TAB_CELL_PADDING_X,
    ..Padding::ZERO
  };
  let labelled: Element<'a, M> = match on_press {
    Some(message) => button(
      container(content)
        .width(layout.cell_width())
        .height(Length::Fill)
        .align_x(layout.align_x())
        .align_y(Vertical::Center),
    )
    .padding(cell_padding)
    .width(layout.cell_width())
    .height(Length::Fill)
    .on_press(message)
    .style(move |_, status| tab_button_style(selected, status))
    .into(),
    None => container(content)
      .padding(cell_padding)
      .width(layout.cell_width())
      .height(Length::Fill)
      .align_x(layout.align_x())
      .align_y(Vertical::Center)
      .style(move |_| container::Style {
        background: selected.then(|| Background::Color(tab_tint())),
        ..container::Style::default()
      })
      .into(),
  };

  if !selected {
    return labelled;
  }

  let underline = container(
    container(Space::new().width(Length::Fill).height(Length::Fixed(UNDERLINE_HEIGHT)))
      .width(Length::Fill)
      .height(Length::Fixed(UNDERLINE_HEIGHT))
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_y(Vertical::Bottom);

  stack![labelled, underline].into()
}

fn tab_button_style(selected: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: (selected || hovered).then(|| Background::Color(tab_tint())),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn tab_tint() -> iced::Color {
  color::with_alpha(color::text::PRIMARY, 0.04)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn characters_tab(selected: bool, on_press: Option<()>) -> Tab<'static, ()> {
    Tab {
      count: "12".to_owned(),
      label: "Characters",
      on_press,
      selected,
    }
  }

  #[test]
  fn it_renders_a_clickable_unselected_tab() {
    let _el: Element<'_, ()> = tab(characters_tab(false, Some(())), TabLayout::default());
  }

  #[test]
  fn it_renders_a_selected_clickable_tab_with_an_underline() {
    let el: Element<'_, ()> = tab(characters_tab(true, Some(())), TabLayout::default());
    let mut tree = iced::advanced::widget::Tree::new(&el);
    tree.diff(&el);

    assert!(
      tree.children.len() >= 2,
      "selected tab should stack the underline over the cell"
    );
  }

  #[test]
  fn it_renders_a_selected_presentational_tab_with_an_underline() {
    let el: Element<'_, ()> = tab(characters_tab(true, None), TabLayout::default());
    let mut tree = iced::advanced::widget::Tree::new(&el);
    tree.diff(&el);

    assert!(
      tree.children.len() >= 2,
      "selected presentational tab should stack the underline over the cell"
    );
  }

  #[test]
  fn it_renders_a_presentational_tab_with_a_count() {
    let _el: Element<'_, ()> = tab_select_with(vec![characters_tab(true, None)], TabLayout::default());
  }

  #[test]
  fn it_renders_multiple_tabs_in_order() {
    let _el: Element<'_, ()> = tab_select_with(
      vec![characters_tab(true, None), characters_tab(false, Some(()))],
      TabLayout::default(),
    );
  }

  #[test]
  fn it_renders_with_no_tabs() {
    let _el: Element<'_, ()> = tab_select_with(vec![], TabLayout::default());
  }

  #[test]
  fn it_defaults_to_the_start_layout() {
    assert_eq!(TabLayout::default(), TabLayout::Start);
  }

  #[test]
  fn it_keeps_the_start_layout_left_aligned_and_intrinsic_width() {
    assert_eq!(TabLayout::Start.align_x(), Horizontal::Left);
    assert_eq!(TabLayout::Start.cell_width(), Length::Shrink);
  }

  #[test]
  fn it_fills_equal_width_cells_for_the_fill_layout() {
    assert_eq!(TabLayout::Fill.align_x(), Horizontal::Left);
    assert_eq!(TabLayout::Fill.cell_width(), Length::Fill);
  }

  #[test]
  fn it_centers_and_fills_for_the_centered_layout() {
    assert_eq!(TabLayout::Centered.align_x(), Horizontal::Center);
    assert_eq!(TabLayout::Centered.cell_width(), Length::Fill);
  }

  #[test]
  fn it_renders_a_fill_layout_selected_tab_with_an_underline() {
    let el: Element<'_, ()> = tab(characters_tab(true, Some(())), TabLayout::Fill);
    let mut tree = iced::advanced::widget::Tree::new(&el);
    tree.diff(&el);

    assert!(tree.children.len() >= 2, "selected fill tab should stack the underline");
  }

  #[test]
  fn it_renders_a_centered_tab_select() {
    let _el: Element<'_, ()> = tab_select_with(
      vec![characters_tab(true, None), characters_tab(false, Some(()))],
      TabLayout::Centered,
    );
  }
}
