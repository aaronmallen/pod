use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, mouse_area, text},
};

use super::{StockpileCard, item_row};
use crate::{
  features::assets::Message,
  ui::{
    components::{eyebrow::eyebrow, progress_bar::progress_bar, rule, status::dot_sized},
    style::{color, radius, spacing, typography},
  },
};

const BAR_HEIGHT: f32 = 6.0;
const CARD_WIDTH: f32 = 440.0;
const DOT_SIZE: f32 = 8.0;

pub(super) fn view(model: &StockpileCard) -> Element<'_, Message> {
  let ready = model.is_full();
  let pct = model.overall_pct;

  let dot_color = if ready {
    color::status::ONLINE
  } else if pct > 0.6 {
    color::status::WARNING
  } else {
    color::status::DANGER
  };
  let pct_color = if ready {
    color::status::ONLINE
  } else {
    color::status::WARNING
  };
  let bar_color = if ready {
    color::status::ONLINE
  } else {
    color::accent::PLASMA
  };

  let body = Column::with_children(vec![
    meta(model, dot_color, pct_color, bar_color),
    rule::horizontal(),
    items(model),
  ])
  .width(Length::Fill);

  let card = container(body)
    .width(Length::Fixed(CARD_WIDTH))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: if ready {
          color::with_alpha(color::status::ONLINE, 0.35)
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  mouse_area(card)
    .on_right_press(Message::StockpileCardRightPressed(model.id))
    .into()
}

fn items(model: &StockpileCard) -> Element<'_, Message> {
  let rows: Vec<Element<'_, Message>> = model.items.iter().map(item_row::view).collect();

  container(Column::with_children(rows).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn location_caption(model: &StockpileCard) -> (String, iced::Color) {
  match (&model.location_name, model.location_id) {
    (Some(name), _) => (name.to_uppercase(), color::text::SECONDARY),
    (None, Some(_)) => ("Unknown location".to_uppercase(), color::text::TERTIARY),
    (None, None) => ("Any location".to_uppercase(), color::text::TERTIARY),
  }
}

fn meta(
  model: &StockpileCard,
  dot_color: iced::Color,
  pct_color: iced::Color,
  bar_color: iced::Color,
) -> Element<'_, Message> {
  let head = Row::with_children(vec![
    dot_sized(dot_color, DOT_SIZE),
    text(model.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    text(format!("{}%", (model.overall_pct * 100.0).round() as i64))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(pct_color),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let (caption, caption_color) = location_caption(model);

  container(
    Column::with_children(vec![
      head.into(),
      eyebrow(&caption, Some(caption_color)),
      progress_bar(model.overall_pct as f32, bar_color, BAR_HEIGHT),
    ])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3_5)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn card_model(location_name: Option<&str>, location_id: Option<i64>) -> StockpileCard {
    StockpileCard {
      character_id: None,
      fill_isk: 0.0,
      id: 1,
      items: vec![super::super::StockpileItemLine {
        have: 400,
        pct: 0.4,
        target: 1000,
        type_id: 34,
        type_name: "Tritanium".to_owned(),
      }],
      location_id,
      location_name: location_name.map(str::to_owned),
      name: "Cache".to_owned(),
      overall_pct: 0.4,
      target_isk: 0.0,
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_card_with_a_resolved_location() {
      let model = card_model(Some("Jita IV - Moon 4"), Some(60_003_760));

      let _el: Element<'_, Message> = view(&model);
    }

    #[test]
    fn it_renders_a_card_with_an_unresolved_location_fallback() {
      let model = card_model(None, Some(60_003_760));

      let _el: Element<'_, Message> = view(&model);
    }

    #[test]
    fn it_renders_an_unscoped_card() {
      let model = card_model(None, None);

      let _el: Element<'_, Message> = view(&model);
    }
  }

  mod location_caption {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uppercases_a_resolved_name() {
      let (caption, _) = super::super::location_caption(&card_model(Some("Jita IV"), Some(1)));

      assert_eq!(caption, "JITA IV");
    }

    #[test]
    fn it_falls_back_for_an_unresolved_or_unscoped_location() {
      let (unresolved, _) = super::super::location_caption(&card_model(None, Some(1)));
      let (unscoped, _) = super::super::location_caption(&card_model(None, None));

      assert_eq!(unresolved, "UNKNOWN LOCATION");
      assert_eq!(unscoped, "ANY LOCATION");
    }
  }
}
