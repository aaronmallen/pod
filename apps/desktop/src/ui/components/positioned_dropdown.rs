use iced::{Element, Length, Padding, alignment::Horizontal, widget::container};

pub fn positioned_dropdown<'a, M>(content: Element<'a, M>, top: f32, left: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(content)
    .padding(Padding {
      top,
      left,
      ..Padding::ZERO
    })
    .into()
}

pub fn positioned_dropdown_right<'a, M>(content: Element<'a, M>, top: f32, right: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .padding(Padding {
      top,
      right,
      ..Padding::ZERO
    })
    .into()
}

#[cfg(test)]
mod tests {
  use iced::widget::Space;

  use super::*;

  fn space() -> Element<'static, ()> {
    Space::new().into()
  }

  mod positioned_dropdown {
    use super::*;

    #[test]
    fn it_renders_a_left_positioned_dropdown() {
      let _el: Element<'_, ()> = positioned_dropdown(space(), 56.0, 24.0);
    }
  }

  mod positioned_dropdown_right {
    use super::*;

    #[test]
    fn it_renders_a_right_aligned_dropdown() {
      let _el: Element<'_, ()> = positioned_dropdown_right(space(), 56.0, 24.0);
    }
  }
}
