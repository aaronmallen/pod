use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Row, Space, button, container, stack, text},
};

use crate::ui::style::{color, radius, typography};

const COUNT_SIZE: f32 = typography::size::SM;
const LABEL_SIZE: f32 = 20.0;
const TAB_CELL_PADDING_X: f32 = 2.0;
const TAB_GAP: f32 = 28.0;
const TAB_LABEL_GAP: f32 = 10.0;
const UNDERLINE_HEIGHT: f32 = 2.0;

pub struct Tab<M> {
  pub count: String,
  pub label: String,
  pub on_press: Option<M>,
  pub selected: bool,
}

pub fn roster_tabs<'a, M>(tabs: Vec<Tab<M>>) -> Element<'a, M>
where
  M: Clone + 'a,
{
  Row::with_children(tabs.into_iter().map(tab).collect::<Vec<_>>())
    .spacing(TAB_GAP)
    .height(Length::Fill)
    .align_y(Vertical::Center)
    .into()
}

fn tab<'a, M>(descriptor: Tab<M>) -> Element<'a, M>
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
    color::text::secondary()
  };
  let count_color = if selected {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };

  let content = Row::with_children(vec![
    text(label)
      .font(typography::body::MEDIUM)
      .size(LABEL_SIZE)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
    text(count)
      .font(typography::mono::MEDIUM)
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
        .height(Length::Fill)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Center),
    )
    .padding(cell_padding)
    .height(Length::Fill)
    .on_press(message)
    .style(|_, _| button::Style::default())
    .into(),
    None => container(content)
      .padding(cell_padding)
      .height(Length::Fill)
      .align_x(Horizontal::Left)
      .align_y(Vertical::Center)
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

#[cfg(test)]
mod tests {
  use super::*;

  fn characters_tab(selected: bool, on_press: Option<()>) -> Tab<()> {
    Tab {
      count: "12".to_owned(),
      label: "Characters".to_owned(),
      on_press,
      selected,
    }
  }

  #[test]
  fn it_renders_a_clickable_unselected_tab() {
    let _el: Element<'_, ()> = tab(characters_tab(false, Some(())));
  }

  #[test]
  fn it_renders_a_selected_tab_with_an_underline() {
    let el: Element<'_, ()> = tab(characters_tab(true, None));
    let mut tree = iced::advanced::widget::Tree::new(&el);
    tree.diff(&el);

    assert!(
      tree.children.len() >= 2,
      "selected tab should stack the underline over the cell"
    );
  }

  #[test]
  fn it_renders_multiple_tabs_in_order() {
    let _el: Element<'_, ()> = roster_tabs(vec![characters_tab(true, None), characters_tab(false, Some(()))]);
  }

  #[test]
  fn it_renders_with_no_tabs() {
    let _el: Element<'_, ()> = roster_tabs(vec![]);
  }
}
