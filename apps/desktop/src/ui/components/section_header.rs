use iced::{
  Element, Length,
  alignment::Vertical,
  widget::{Row, Space, text},
};
#[cfg(test)]
use iced::{alignment::Horizontal, widget::container};

use crate::ui::style::{color, typography};

#[cfg(test)]
pub fn column_headers<'a, M: 'a>(columns: &[(&str, bool)]) -> Element<'a, M> {
  let cells: Vec<Element<'a, M>> = columns
    .iter()
    .map(|&(label, right)| column_cell(label, right))
    .collect();

  Row::with_children(cells)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

pub fn section_header<'a, M: 'a>(label: &str, right: Option<&str>) -> Element<'a, M> {
  let mut children: Vec<Element<'a, M>> = vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
  ];

  if let Some(right) = right {
    children.push(
      text(right.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::dim()),
        })
        .into(),
    );
  }

  Row::with_children(children)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
fn column_cell<'a, M: 'a>(label: &str, right: bool) -> Element<'a, M> {
  let cell = text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    })
    .width(Length::Fill);

  container(cell)
    .width(Length::Fill)
    .align_x(if right { Horizontal::Right } else { Horizontal::Left })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug)]
  enum Message {}

  mod column_headers {
    use super::*;

    #[test]
    fn it_builds_left_aligned_columns() {
      let _el: Element<'_, Message> = column_headers(&[("Item", false), ("Group", false)]);
    }

    #[test]
    fn it_builds_mixed_alignment_columns() {
      let _el: Element<'_, Message> = column_headers(&[("Item", false), ("Qty", true), ("Value", true)]);
    }
  }

  mod section_header {
    use super::*;

    #[test]
    fn it_builds_an_eyebrow_with_right_meta() {
      let _el: Element<'_, Message> = section_header("Overview", Some("3 items"));
    }

    #[test]
    fn it_builds_an_eyebrow_without_right_meta() {
      let _el: Element<'_, Message> = section_header("Overview", None);
    }
  }
}
