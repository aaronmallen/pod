use iced::{
  Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use crate::ui::style::{color, spacing, typography};

pub fn panel_header<'a, M>(
  title: impl Into<String>,
  subtitle: Option<String>,
  right: Option<String>,
  accent: bool,
) -> Element<'a, M>
where
  M: 'a,
{
  let mut left: Vec<Element<'a, M>> = vec![
    text(title.into())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(subtitle) = subtitle {
    left.push(
      text(subtitle.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    );
  }

  let mut children: Vec<Element<'a, M>> = vec![
    Column::with_children(left).spacing(spacing::UNIT - 2.0).into(),
    Space::new().width(Length::Fill).into(),
  ];
  if let Some(right) = right {
    let tag_color = if accent {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    };
    children.push(
      text(right.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(move |_| text::Style {
          color: Some(tag_color),
        })
        .into(),
    );
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Bottom)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3_5 + spacing::UNIT / 2.0,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5 + spacing::UNIT / 2.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.08),
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod panel_header {
    use super::*;

    #[test]
    fn it_renders_a_title_only_header() {
      let _el: Element<'_, ()> = panel_header("Standings", None, None, false);
    }

    #[test]
    fn it_renders_with_subtitle_and_right_meta() {
      let _el: Element<'_, ()> = panel_header("Standings", Some("corp".into()), Some("3 items".into()), true);
    }
  }
}
