use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use crate::ui::{
  components::rule,
  style::{color, control, radius, shadow, spacing, typography},
};

const FOOTER_PAD_X: f32 = 16.0;
const FOOTER_PAD_Y: f32 = 12.0;
const HEADER_PAD_BOTTOM: f32 = 14.0;
const HEADER_PAD_X: f32 = 20.0;
const HEADER_PAD_TOP: f32 = 18.0;
const MODAL_WIDTH: f32 = 420.0;

pub fn confirm_modal<'a, M>(
  eyebrow: impl text::IntoFragment<'a>,
  title: impl text::IntoFragment<'a>,
  body: impl text::IntoFragment<'a>,
  confirm_label: impl text::IntoFragment<'a>,
  on_confirm: M,
  on_cancel: M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let eyebrow = text(eyebrow)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::status::DANGER),
    });
  let title = text(title)
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let body = text(body)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    });

  let header_block =
    container(Column::with_children(vec![eyebrow.into(), title.into(), body.into()]).spacing(spacing::SPACE_2))
      .width(Length::Fill)
      .padding(Padding {
        top: HEADER_PAD_TOP,
        right: HEADER_PAD_X,
        bottom: HEADER_PAD_BOTTOM,
        left: HEADER_PAD_X,
      });

  let cancel = button(text("Cancel").font(typography::body::MEDIUM).size(typography::size::MD))
    .padding(control::padding())
    .on_press(on_cancel)
    .style(control::ghost_button);
  let confirm = button(
    text(confirm_label)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(on_confirm)
  .style(control::danger_button);

  let footer = container(
    Row::with_children(vec![
      Space::new().width(Length::Fill).into(),
      cancel.into(),
      confirm.into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: FOOTER_PAD_Y,
    right: FOOTER_PAD_X,
    bottom: FOOTER_PAD_Y,
    left: FOOTER_PAD_X,
  });

  let card =
    container(Column::with_children(vec![header_block.into(), rule::horizontal(), footer.into()]).width(Length::Fill))
      .width(Length::Fixed(MODAL_WIDTH))
      .clip(true)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.18),
          width: 1.0,
          radius: radius::CARD.into(),
        },
        shadow: shadow::CARD,
        ..container::Style::default()
      });

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug)]
  enum Message {
    Cancel,
    Confirm,
  }

  mod confirm_modal {
    use super::*;

    #[test]
    fn it_builds_a_raised_confirm_card() {
      let _el: Element<'_, Message> = confirm_modal(
        "Remove character",
        "Remove Pilot from Pod?",
        "This unlinks the character from this app only.",
        "Remove",
        Message::Confirm,
        Message::Cancel,
      );
    }
  }
}
