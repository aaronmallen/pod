use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, Stack, container, mouse_area, text},
};

use super::{Message, card::TagChip};
use crate::{
  store::images,
  sync::Phase,
  ui::{
    components::{avatar::Avatar, chip::chip, eyebrow::eyebrow, rule},
    style::{color, radius, spacing, typography},
  },
};

const CHIP_GAP: f32 = 5.0;
const HAIRLINE: f32 = 1.0;
const LOGO_SIZE: f32 = 72.0;
const MEMBERS_VALUE_SIZE: f32 = 22.0;
const PLACEHOLDER: &str = "—";
const PLATE_HEIGHT: f32 = 140.0;
const TICKER_SIZE: f32 = 34.0;

#[derive(Clone, Debug)]
pub struct CorpCardModel {
  pub alliance: Option<String>,
  pub alliance_ticker: Option<String>,
  pub ceo: Option<String>,
  pub corporation_id: i64,
  pub hq: Option<String>,
  pub logo: images::ImageState,
  pub members: Option<i64>,
  pub name: String,
  pub tags: Vec<TagChip>,
  pub tax_rate: Option<f64>,
  pub ticker: String,
}

pub(super) fn corp_card(model: &CorpCardModel, failure: Option<Phase>) -> Element<'_, Message> {
  let mut sections: Vec<Element<'_, Message>> = vec![
    plate(model),
    identity(model),
    tag_row(model),
    rule::horizontal(),
    members_section(model),
    rule::horizontal(),
    stats_row("CEO", model.ceo.clone(), "HQ", model.hq.clone(), false),
  ];

  if let Some(indicator) = reauth_indicator(failure) {
    sections.push(indicator);
  }

  let body = container(Column::with_children(sections))
    .width(Length::Fill)
    .style(card_surface);

  mouse_area(body)
    .on_right_press(Message::CorpRightPressed(model.corporation_id))
    .into()
}

fn plate(model: &CorpCardModel) -> Element<'_, Message> {
  let logo = Avatar::new(
    model.corporation_id,
    &model.ticker,
    Length::Fixed(LOGO_SIZE),
    LOGO_SIZE,
    model.logo.path(),
  )
  .border(color::with_alpha(color::text::PRIMARY, 0.1), HAIRLINE)
  .radius(radius::SUBTLE)
  .view::<Message>();

  let ticker = text(model.ticker.clone())
    .font(typography::mono::MEDIUM)
    .size(TICKER_SIZE)
    .style(|_| text::Style {
      color: Some(color::accent::PLASMA),
    });

  let affiliation: Element<'_, Message> = match &model.alliance_ticker {
    Some(alliance_ticker) => text(format!("\u{2039} {alliance_ticker} \u{203A}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    None => text("UNAFFILIATED")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  };

  let identity = Column::with_children(vec![ticker.into(), affiliation]).spacing(spacing::SPACE_2);

  container(
    Row::with_children(vec![logo, identity.into()])
      .spacing(spacing::SPACE_3_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .height(Length::Fixed(PLATE_HEIGHT))
  .padding([0.0, spacing::SPACE_6])
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn identity(model: &CorpCardModel) -> Element<'_, Message> {
  let name = text(model.name.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let alliance = text(model.alliance.clone().unwrap_or_else(|| "No alliance".to_owned()))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    });

  container(Column::with_children(vec![name.into(), alliance.into()]).spacing(spacing::UNIT))
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
      bottom: 0.0,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn tag_row(model: &CorpCardModel) -> Element<'_, Message> {
  let chips: Vec<Element<'_, Message>> = model.tags.iter().map(|tag| chip(tag.name.clone(), tag.color)).collect();

  container(Row::with_children(chips).spacing(CHIP_GAP))
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn members_section(model: &CorpCardModel) -> Element<'_, Message> {
  let labels = Row::with_children(vec![
    eyebrow("Members", None),
    Space::new().width(Length::Fill).into(),
    eyebrow("Tax rate", None),
  ])
  .width(Length::Fill);

  let values = Row::with_children(vec![
    big_value(format_members(model.members)),
    Space::new().width(Length::Fill).into(),
    big_value(format_tax(model.tax_rate)),
  ])
  .width(Length::Fill)
  .align_y(Vertical::Bottom);

  container(Column::with_children(vec![labels.into(), values.into()]).spacing(spacing::SPACE_2))
    .padding(spacing::SPACE_3)
    .into()
}

fn big_value<'a>(value: String) -> Element<'a, Message> {
  text(value)
    .font(typography::mono::MEDIUM)
    .size(MEMBERS_VALUE_SIZE)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into()
}

fn stats_row<'a>(
  left_label: &'a str,
  left_value: Option<String>,
  right_label: &'a str,
  right_value: Option<String>,
  mono: bool,
) -> Element<'a, Message> {
  let columns = Row::with_children(vec![
    stat(left_label, left_value, mono),
    Space::new().width(Length::Fixed(HAIRLINE)).into(),
    stat(right_label, right_value, mono),
  ])
  .width(Length::Fill);

  Stack::with_children(vec![
    columns.into(),
    container(rule::vertical_fill(0.1)).center_x(Length::Fill).into(),
  ])
  .width(Length::Fill)
  .into()
}

fn stat<'a>(label: &'a str, value: Option<String>, mono: bool) -> Element<'a, Message> {
  let value_font = if mono {
    typography::mono::MEDIUM
  } else {
    typography::body::REGULAR
  };
  let label = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    });
  let value = text(value.unwrap_or_else(|| PLACEHOLDER.to_owned()))
    .font(value_font)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  container(Column::with_children(vec![label.into(), value.into()]).spacing(spacing::UNIT))
    .width(Length::Fill)
    .padding(spacing::SPACE_3)
    .into()
}

fn reauth_indicator<'a>(failure: Option<Phase>) -> Option<Element<'a, Message>> {
  match failure? {
    Phase::BackingOff | Phase::Failed => {}
    Phase::Blocked | Phase::Done | Phase::Empty | Phase::NotReady | Phase::Syncing => return None,
  }

  Some(
    container(
      text("Needs re-authentication")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        }),
    )
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .into(),
  )
}

fn card_surface(theme: &iced::Theme) -> container::Style {
  crate::ui::style::control::card(theme)
}

fn format_members(members: Option<i64>) -> String {
  let Some(value) = members else {
    return PLACEHOLDER.to_owned();
  };
  if value >= 1_000_000 {
    format!("{:.1}M", value as f64 / 1e6)
  } else if value >= 1_000 {
    group_thousands(value)
  } else {
    value.to_string()
  }
}

fn group_thousands(value: i64) -> String {
  let digits = value.to_string();
  let mut grouped = String::new();
  let len = digits.len();
  for (index, ch) in digits.chars().enumerate() {
    if index > 0 && (len - index).is_multiple_of(3) {
      grouped.push('\u{2009}');
    }
    grouped.push(ch);
  }
  grouped
}

fn format_tax(tax_rate: Option<f64>) -> String {
  match tax_rate {
    Some(rate) => format!("{:.1}%", rate * 100.0),
    None => PLACEHOLDER.to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn base_model() -> CorpCardModel {
    CorpCardModel {
      alliance: Some("Iron Helix Pact".to_owned()),
      alliance_ticker: Some("IHP".to_owned()),
      ceo: Some("Vex Voronova".to_owned()),
      corporation_id: 98_000_001,
      hq: Some("Jita IV — Moon 4".to_owned()),
      logo: images::ImageState::Stale {
        id: 98_000_001,
        kind: images::ImageKind::CorporationLogo,
      },
      members: Some(1247),
      name: "Cobalt Syndicate".to_owned(),
      tags: Vec::new(),
      tax_rate: Some(0.10),
      ticker: "COBSY".to_owned(),
    }
  }

  mod format_members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_millions_thin_space_thousands_and_raw_figures() {
      assert_eq!(format_members(Some(2_400_000)), "2.4M");
      assert_eq!(format_members(Some(12_400)), "12\u{2009}400");
      assert_eq!(format_members(Some(1_247)), "1\u{2009}247");
      assert_eq!(format_members(Some(89)), "89");
    }

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_count() {
      assert_eq!(format_members(None), PLACEHOLDER);
    }
  }

  mod format_tax {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_a_one_decimal_percentage() {
      assert_eq!(format_tax(Some(0.10)), "10.0%");
      assert_eq!(format_tax(Some(0.025)), "2.5%");
    }

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_rate() {
      assert_eq!(format_tax(None), PLACEHOLDER);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_a_player_corp_card() {
      let model = base_model();

      let _el: Element<'_, Message> = corp_card(&model, None);
    }

    #[test]
    fn it_renders_an_unaffiliated_corp_with_placeholders() {
      let mut model = base_model();
      model.alliance = None;
      model.alliance_ticker = None;
      model.ceo = None;
      model.hq = None;
      model.members = None;
      model.tax_rate = None;

      let _el: Element<'_, Message> = corp_card(&model, None);
    }

    #[test]
    fn it_renders_the_needs_reauthentication_treatment() {
      let model = base_model();

      for failure in [Phase::Failed, Phase::BackingOff] {
        let _el: Element<'_, Message> = corp_card(&model, Some(failure));
      }
    }

    #[test]
    fn it_renders_with_tags() {
      let mut model = base_model();
      model.tags = vec![TagChip {
        color: Some(color::accent::PLASMA),
        id: 1,
        name: "Industry".to_owned(),
      }];

      let _el: Element<'_, Message> = corp_card(&model, None);
    }
  }
}
