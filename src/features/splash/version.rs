use iced::{Element, widget::text};

use crate::ui::style::{color, typography};

pub fn version<'a, M>(update: Option<&str>) -> Element<'a, M>
where
  M: 'a,
{
  match update {
    Some(next) => text(format!("Update \u{00B7} v{next}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    None => text(format!("v{}", env!("CARGO_PKG_VERSION")))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  }
}
