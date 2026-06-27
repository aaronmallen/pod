use iced::{Element, widget::text};

use crate::ui::style::{color, typography};

pub fn version<'a, M>(update: Option<&str>) -> Element<'a, M>
where
  M: 'a,
{
  match update {
    Some(next) => text(t!("splash.version.update", version => next).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    None => text(t!("splash.version.current", version => env!("CARGO_PKG_VERSION")).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  }
}
