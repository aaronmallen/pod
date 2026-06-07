use iced::{
  Color, Element,
  widget::{Text, text},
};

use crate::ui::style::{color, typography};

pub fn eyebrow<'a, M>(label: &str, color: Option<Color>) -> Element<'a, M>
where
  M: 'a,
{
  eyebrow_text(label, color).into()
}

pub fn eyebrow_text<'a>(label: &str, fill: Option<Color>) -> Text<'a> {
  let fill = fill.unwrap_or(color::text::SECONDARY);

  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(fill),
    })
}

#[cfg(test)]
mod tests {
  use super::*;

  mod eyebrow {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_renders_a_default_label() {
      let _el: Element<'_, ()> = eyebrow("overview", None);
    }

    #[test]
    fn it_renders_a_colored_label() {
      let _el: Element<'_, ()> = eyebrow("active", Some(color::accent::PLASMA));
    }
  }

  mod eyebrow_text {
    use super::*;

    #[test]
    fn it_builds_a_text_widget() {
      let _t: Text<'_> = eyebrow_text("label", None);
    }
  }
}
