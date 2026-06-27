use iced::{
  Border, Element,
  widget::{Row, button, container, text},
};

use crate::ui::{
  components::segmented::segment_button_style,
  style::{color, radius, spacing, typography},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Side {
  #[default]
  All,
  Buy,
  Sell,
}

impl Side {
  pub fn all() -> [Side; 3] {
    [Side::All, Side::Buy, Side::Sell]
  }

  pub fn label(self) -> &'static str {
    match self {
      Side::All => super::i18n::tr_static("wallet.side.all"),
      Side::Buy => super::i18n::tr_static("wallet.side.buy"),
      Side::Sell => super::i18n::tr_static("wallet.side.sell"),
    }
  }
}

pub fn side_filter<'a, M>(active: Side, on_select: impl Fn(Side) -> M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let row = Row::with_children(
    Side::all()
      .into_iter()
      .map(|side| segment(side, side == active, on_select(side)))
      .collect::<Vec<Element<'a, M>>>(),
  );

  container(row)
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn segment<'a, M>(side: Side, active: bool, message: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(
    text(side.label().to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(if active {
          color::accent::PLASMA
        } else {
          color::text::secondary()
        }),
      }),
  )
  .padding(iced::Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  })
  .on_press(message)
  .style(move |_, status| segment_button_style(active, status))
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug, PartialEq)]
  enum Msg {
    Pick(Side),
  }

  #[test]
  fn it_defaults_to_all() {
    assert_eq!(Side::default(), Side::All);
  }

  #[test]
  fn it_exposes_all_three_sides() {
    assert_eq!(Side::all(), [Side::All, Side::Buy, Side::Sell]);
  }

  #[test]
  fn it_renders_for_each_active_side() {
    for side in Side::all() {
      let _el: Element<'_, Msg> = side_filter(side, Msg::Pick);
    }
  }
}
