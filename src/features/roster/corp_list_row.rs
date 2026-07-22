use iced::{
  Background, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, container, mouse_area, text},
};

use super::{Message, card::reauth_badge, corp_card::CorpCardModel, name_link::name_link};
use crate::{
  sync::Phase,
  ui::{
    components::{avatar::Avatar, chip::chip, eyebrow::eyebrow, rule},
    style::{color, radius, spacing, typography},
  },
};

const CORP_BADGE_INSET: f32 = 6.0;
const HAIRLINE: f32 = 1.0;
const LOGO_SIZE: f32 = 58.0;
const LOGO_STRIP_WIDTH: f32 = 120.0;
const MAX_TAGS: usize = 4;
const PANEL_GAP: f32 = 16.0;
const PANEL_DIVIDER_HEIGHT: f32 = 34.0;
const PLACEHOLDER: &str = "—";
const RAIL_WIDTH: f32 = 30.0;
const RIGHT_WIDTH: f32 = 210.0;
const ROW_GAP: f32 = 9.0;
const ROW_HEIGHT: f32 = 84.0;
const ROW_INLINE_GAP: f32 = 10.0;
const STAT_VALUE_SIZE: f32 = 18.0;
const TAG_GAP: f32 = 5.0;
const TAX_PERCENT_SIZE: f32 = 11.0;
const TICKER_SIZE: f32 = 16.0;
const TINT_ALPHA: f32 = 0.55;
const TRACK_ALPHA: f32 = 0.1;

pub(super) fn corp_list_row<'a>(model: &'a CorpCardModel, failure: Option<Phase>) -> Element<'a, Message> {
  let tinted = model.needs_reauth || is_failing(failure);

  let columns: Vec<Element<'a, Message>> = vec![
    drag_rail(),
    rule::vertical_fill(TRACK_ALPHA),
    logo_strip(model),
    rule::vertical_fill(TRACK_ALPHA),
    center(model),
    rule::vertical_fill(TRACK_ALPHA),
    right_panel(model),
  ];

  let body = container(Row::with_children(columns).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(ROW_HEIGHT))
    .clip(true)
    .style(surface(tinted));

  mouse_area(body)
    .on_right_press(Message::CorpRightPressed(model.corporation_id))
    .into()
}

fn drag_rail<'a>() -> Element<'a, Message> {
  container(grip())
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn grip<'a>() -> Element<'a, Message> {
  const GRIP_DOT: f32 = 3.0;
  const GRIP_DOT_ALPHA: f32 = 0.55;
  const GRIP_GAP: f32 = 3.0;

  let dot =
    || crate::ui::components::status::dot_sized(color::with_alpha(color::text::PRIMARY, GRIP_DOT_ALPHA), GRIP_DOT);
  let dots = || Row::with_children(vec![dot(), dot()]).spacing(GRIP_GAP).into();

  Column::with_children(vec![dots(), dots(), dots()])
    .spacing(GRIP_GAP)
    .into()
}

fn logo_strip(model: &CorpCardModel) -> Element<'_, Message> {
  let tile = Avatar::new(
    model.corporation_id,
    &model.ticker,
    Length::Fixed(LOGO_SIZE),
    LOGO_SIZE,
    model.logo.path(),
  )
  .border(color::with_alpha(color::text::PRIMARY, 0.1), HAIRLINE)
  .radius(radius::SUBTLE)
  .view::<Message>();

  let logo: Element<'_, Message> = if model.needs_reauth {
    Stack::with_children(vec![
      tile,
      container(reauth_badge(Message::ReauthCorporationRequested(model.corporation_id)))
        .width(Length::Fixed(LOGO_SIZE))
        .height(Length::Fixed(LOGO_SIZE))
        .align_x(Horizontal::Left)
        .align_y(Vertical::Top)
        .padding(CORP_BADGE_INSET)
        .into(),
    ])
    .width(Length::Fixed(LOGO_SIZE))
    .height(Length::Fixed(LOGO_SIZE))
    .into()
  } else {
    tile
  };

  container(logo)
    .width(Length::Fixed(LOGO_STRIP_WIDTH))
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn center(model: &CorpCardModel) -> Element<'_, Message> {
  container(
    Column::with_children(vec![identity_row(model), affiliation_row(model)])
      .spacing(ROW_GAP)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: spacing::SPACE_3 + HAIRLINE,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3 + HAIRLINE,
    left: spacing::SPACE_4_5,
  })
  .into()
}

fn identity_row(model: &CorpCardModel) -> Element<'_, Message> {
  let name = name_link(
    model.name.clone(),
    typography::size::LG,
    Message::CorporationSelected(model.corporation_id),
  );

  let mut children: Vec<Element<'_, Message>> = vec![name];
  if model.needs_reauth {
    children.push(reauth_badge(Message::ReauthCorporationRequested(model.corporation_id)));
  }
  children.push(Space::new().width(Length::Fill).into());
  children.push(
    text(model.ticker.clone())
      .font(typography::mono::MEDIUM)
      .size(TICKER_SIZE)
      .wrapping(text::Wrapping::None)
      .style(|_| text::Style {
        color: Some(color::accent()),
      })
      .into(),
  );

  Row::with_children(children)
    .spacing(ROW_INLINE_GAP)
    .align_y(Vertical::Center)
    .into()
}

fn affiliation_row(model: &CorpCardModel) -> Element<'_, Message> {
  let affiliation = match &model.alliance_ticker {
    Some(ticker) => text(
      format!(
        "\u{2039} {ticker} \u{203A} {}",
        model.alliance.clone().unwrap_or_default()
      )
      .to_uppercase(),
    )
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    }),
    None => text(t!("roster.card.unaffiliated").into_owned()).style(|_| text::Style {
      color: Some(color::text::tertiary()),
    }),
  }
  .font(typography::mono::REGULAR)
  .size(typography::size::XS_PLUS)
  .wrapping(text::Wrapping::None);

  let mut children: Vec<Element<'_, Message>> = vec![affiliation.into()];
  for tag in model.tags.iter().take(MAX_TAGS) {
    children.push(chip(tag.name.clone(), None));
  }

  let overflow = model.tags.len().saturating_sub(MAX_TAGS);
  if overflow > 0 {
    children.push(
      text(format!("+{overflow}"))
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  Row::with_children(children)
    .spacing(TAG_GAP)
    .align_y(Vertical::Center)
    .into()
}

fn right_panel(model: &CorpCardModel) -> Element<'_, Message> {
  let members = stat_block(
    t!("roster.card.members").into_owned(),
    members_value(format_members(model.members)),
  );

  let tax = stat_block(t!("roster.card.tax_rate").into_owned(), tax_value(model.tax_rate));

  container(
    Row::with_children(vec![members, rule::vertical(PANEL_DIVIDER_HEIGHT), tax])
      .spacing(PANEL_GAP)
      .align_y(Vertical::Center),
  )
  .width(Length::Fixed(RIGHT_WIDTH))
  .height(Length::Fill)
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: spacing::SPACE_3 + HAIRLINE,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3 + HAIRLINE,
    left: spacing::SPACE_4_5,
  })
  .into()
}

fn members_value<'a>(value: String) -> Element<'a, Message> {
  text(value)
    .font(typography::mono::MEDIUM)
    .size(STAT_VALUE_SIZE)
    .wrapping(text::Wrapping::None)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into()
}

fn tax_value<'a>(tax_rate: Option<f64>) -> Element<'a, Message> {
  match format_tax(tax_rate) {
    Some(rate) => Row::with_children(vec![
      text(rate)
        .font(typography::mono::MEDIUM)
        .size(STAT_VALUE_SIZE)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text("%")
        .font(typography::mono::REGULAR)
        .size(TAX_PERCENT_SIZE)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    ])
    .align_y(Vertical::Bottom)
    .into(),
    None => members_value(PLACEHOLDER.to_owned()),
  }
}

fn stat_block<'a>(label: String, value: Element<'a, Message>) -> Element<'a, Message> {
  let label = container(eyebrow(&label, None))
    .width(Length::Fill)
    .align_x(Horizontal::Right);
  let value = container(value).width(Length::Fill).align_x(Horizontal::Right);

  container(Column::with_children(vec![label.into(), value.into()]).spacing(spacing::UNIT))
    .align_x(Horizontal::Right)
    .into()
}

fn is_failing(failure: Option<Phase>) -> bool {
  matches!(failure, Some(Phase::BackingOff | Phase::Failed))
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

/// Groups digits into thousands using a thin space (U+2009), not a regular ASCII space.
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

fn format_tax(tax_rate: Option<f64>) -> Option<String> {
  tax_rate.map(|rate| format!("{:.1}", rate * 100.0))
}

fn surface(tinted: bool) -> impl Fn(&iced::Theme) -> container::Style {
  move |theme: &iced::Theme| {
    let mut style = crate::ui::style::control::card(theme);
    if tinted {
      style.border.color = color::with_alpha(color::status::DANGER, TINT_ALPHA);
    }
    style
  }
}

#[cfg(test)]
mod tests {
  use super::{super::card::TagChip, *};
  use crate::store::images;

  fn base_model() -> CorpCardModel {
    CorpCardModel {
      alliance: Some("Iron Helix Pact".to_owned()),
      alliance_ticker: Some("IHP".to_owned()),
      ceo: Some("Vex Voronova".to_owned()),
      corporation_id: 98_000_001,
      granted_scopes: None,
      hq: Some("Jita IV — Moon 4".to_owned()),
      logo: images::ImageState::Stale {
        id: 98_000_001,
        kind: images::ImageKind::CorporationLogo,
      },
      members: Some(1247),
      name: "Cobalt Syndicate".to_owned(),
      needs_reauth: false,
      tags: Vec::new(),
      tax_rate: Some(0.10),
      ticker: "COBSY".to_owned(),
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_a_player_corp_list_row() {
      let model = base_model();

      let _el: Element<'_, Message> = corp_list_row(&model, None);
    }

    #[test]
    fn it_renders_an_unaffiliated_corp_with_placeholders() {
      let mut model = base_model();
      model.alliance = None;
      model.alliance_ticker = None;
      model.members = None;
      model.tax_rate = None;

      let _el: Element<'_, Message> = corp_list_row(&model, None);
    }

    #[test]
    fn it_renders_a_needs_reauth_row_with_a_sync_failure() {
      let mut model = base_model();
      model.needs_reauth = true;

      let _el: Element<'_, Message> = corp_list_row(&model, Some(Phase::Failed));
    }

    #[test]
    fn it_caps_the_tag_row_and_shows_an_overflow_count() {
      let mut model = base_model();
      model.tags = (0..7)
        .map(|id| TagChip {
          color: None,
          id,
          name: format!("Tag {id}"),
        })
        .collect();

      let _el: Element<'_, Message> = corp_list_row(&model, None);
    }
  }

  mod format_members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_count() {
      assert_eq!(format_members(None), PLACEHOLDER);
    }

    #[test]
    fn it_uses_millions_thin_space_thousands_and_raw_figures() {
      assert_eq!(format_members(Some(2_400_000)), "2.4M");
      assert_eq!(format_members(Some(12_400)), "12\u{2009}400");
      assert_eq!(format_members(Some(1_247)), "1\u{2009}247");
      assert_eq!(format_members(Some(89)), "89");
    }
  }

  mod format_tax {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_a_one_decimal_percentage_without_the_symbol() {
      assert_eq!(format_tax(Some(0.10)), Some("10.0".to_owned()));
      assert_eq!(format_tax(Some(0.025)), Some("2.5".to_owned()));
    }

    #[test]
    fn it_returns_nothing_for_an_unknown_rate() {
      assert_eq!(format_tax(None), None);
    }
  }

  mod is_failing {
    use super::*;

    #[test]
    fn it_flags_only_failing_or_backing_off_phases() {
      assert!(is_failing(Some(Phase::Failed)));
      assert!(is_failing(Some(Phase::BackingOff)));

      assert!(!is_failing(None));
      assert!(!is_failing(Some(Phase::Syncing)));
    }
  }
}
