use iced::{Element, widget::text};

use crate::ui::style::{color, typography};

pub fn version<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  text(format!("v{}", env!("CARGO_PKG_VERSION")))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    })
    .into()
}
