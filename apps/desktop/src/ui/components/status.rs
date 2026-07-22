use iced::{
  Background, Border, Color, Element, Length,
  widget::{Space, container},
};

const DOT_SIZE: f32 = 6.0;

pub fn dot<'a, M>(fill: Color) -> Element<'a, M>
where
  M: 'a,
{
  dot_sized(fill, DOT_SIZE)
}

pub fn dot_sized<'a, M>(fill: Color, size: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new().width(Length::Fixed(size)).height(Length::Fixed(size)))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: (size / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

pub fn format_since(secs: u64) -> String {
  if secs < 60 {
    format!("{secs}s ago")
  } else if secs < 3_600 {
    format!("{}m ago", secs / 60)
  } else {
    format!("{}h ago", secs / 3_600)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod dot {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_builds_a_circular_dot() {
      let _el: Element<'_, ()> = dot(color::status::ONLINE);
    }
  }

  mod dot_sized {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_builds_a_dot_at_a_custom_size() {
      let _small: Element<'_, ()> = dot_sized(color::status::ONLINE, 3.0);
      let _large: Element<'_, ()> = dot_sized(color::status::DANGER, 10.0);
    }
  }

  mod format_since {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_seconds_minutes_and_hours() {
      assert_eq!(format_since(5), "5s ago");
      assert_eq!(format_since(125), "2m ago");
      assert_eq!(format_since(7_400), "2h ago");
    }
  }
}
