use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, button, container, mouse_area, svg, text},
};

use super::{Message, corp_card::CorpCardModel, name_link::name_link};
use crate::{
  sync::Phase,
  ui::{
    components::{avatar::Avatar, chip::chip, rule, status},
    style::{color, spacing, typography},
  },
};

const ALERT_ICON: &[u8] = include_bytes!("../../../assets/images/icons/alert-triangle.svg");
const AVATAR_RADIUS: f32 = 9.0;
const AVATAR_SIZE: f32 = 46.0;
const CARD_PAD_X: f32 = 16.0;
const CHIP_CAP: usize = 3;
const CHIP_GAP: f32 = 5.0;
const GRIP_DOT: f32 = 3.0;
const GRIP_GAP: f32 = 3.0;
const HAIRLINE: f32 = 1.0;
const PLACEHOLDER: &str = "—";
const STAT_DIVIDER_HEIGHT: f32 = 34.0;
const STAT_PAD_Y: f32 = 12.0;
const TOKEN_ALERT_GAP: f32 = 5.0;
const TOKEN_ALERT_ICON: f32 = 11.0;
const TOKEN_ALERT_RADIUS: f32 = 5.0;
const TOKEN_BORDER_ALPHA: f32 = 0.55;

pub(super) fn corp_compact_card<'a>(model: &'a CorpCardModel, failure: Option<Phase>) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![header(model)];
  if !model.tags.is_empty() {
    children.push(tag_row(model));
  }
  children.push(rule::horizontal());
  children.push(footer(model));

  if let Some(indicator) = sync_indicator(failure) {
    children.push(indicator);
  }

  let body = container(Column::with_children(children))
    .width(Length::Fill)
    .style(card_surface(model.needs_reauth));

  mouse_area(body)
    .on_right_press(Message::CorpRightPressed(model.corporation_id))
    .into()
}

fn header(model: &CorpCardModel) -> Element<'_, Message> {
  let logo = Avatar::new(
    model.corporation_id,
    &model.ticker,
    Length::Fixed(AVATAR_SIZE),
    AVATAR_SIZE,
    model.logo.path(),
  )
  .border(color::with_alpha(color::text::PRIMARY, 0.1), HAIRLINE)
  .radius(AVATAR_RADIUS)
  .view::<Message>();

  let mut children: Vec<Element<'_, Message>> =
    vec![drag_grip(), logo, container(identity(model)).width(Length::Fill).into()];
  if model.needs_reauth {
    children.push(token_alert(Message::ReauthCorporationRequested(model.corporation_id)));
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: CARD_PAD_X,
    bottom: spacing::SPACE_3,
    left: CARD_PAD_X,
  })
  .into()
}

fn drag_grip<'a>() -> Element<'a, Message> {
  let dot = || status::dot_sized(color::text::tertiary(), GRIP_DOT);
  let grip_row = || Row::with_children(vec![dot(), dot()]).spacing(GRIP_GAP).into();

  Column::with_children(vec![grip_row(), grip_row(), grip_row()])
    .spacing(GRIP_GAP)
    .into()
}

fn identity(model: &CorpCardModel) -> Element<'_, Message> {
  let name = name_link(
    model.name.clone(),
    typography::size::LG - HAIRLINE,
    Message::CorporationSelected(model.corporation_id),
  );

  let ticker = text(model.ticker.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .wrapping(text::Wrapping::None)
    .style(|_| text::Style {
      color: Some(color::accent()),
    });

  let alliance = model
    .alliance
    .clone()
    .unwrap_or_else(|| t!("roster.card.no_alliance").into_owned());
  let affiliation = text(format!("\u{00B7} {alliance}").to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .wrapping(text::Wrapping::None)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let subtitle = Row::with_children(vec![ticker.into(), affiliation.into()])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center);

  Column::with_children(vec![name, subtitle.into()])
    .spacing(spacing::UNIT - 1.0)
    .into()
}

fn token_alert<'a>(on_press: Message) -> Element<'a, Message> {
  let glyph = svg(svg::Handle::from_memory(ALERT_ICON))
    .width(Length::Fixed(TOKEN_ALERT_ICON))
    .height(Length::Fixed(TOKEN_ALERT_ICON))
    .style(|_, _| svg::Style {
      color: Some(color::status::DANGER_INK),
    });

  let label = text(t!("roster.compact.token_badge").to_uppercase())
    .font(typography::mono::SEMIBOLD)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::status::DANGER_INK),
    });

  button(
    Row::with_children(vec![glyph.into(), label.into()])
      .spacing(TOKEN_ALERT_GAP)
      .align_y(Vertical::Center),
  )
  .padding([GRIP_GAP, spacing::SPACE_2])
  .on_press(on_press)
  .style(token_alert_style)
  .into()
}

fn token_alert_style(_theme: &iced::Theme, _status: button::Status) -> button::Style {
  button::Style {
    background: Some(Background::Color(color::status::DANGER)),
    text_color: color::status::DANGER_INK,
    border: Border {
      radius: TOKEN_ALERT_RADIUS.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn tag_row(model: &CorpCardModel) -> Element<'_, Message> {
  let mut chips: Vec<Element<'_, Message>> = model
    .tags
    .iter()
    .take(CHIP_CAP)
    .map(|tag| chip(tag.name.clone(), None))
    .collect();

  if let Some(extra) = overflow_count(model.tags.len()) {
    chips.push(overflow_chip(extra));
  }

  container(Row::with_children(chips).spacing(CHIP_GAP).align_y(Vertical::Center))
    .padding(Padding {
      top: 0.0,
      right: CARD_PAD_X,
      bottom: spacing::SPACE_3,
      left: CARD_PAD_X,
    })
    .into()
}

fn overflow_count(total: usize) -> Option<usize> {
  total.checked_sub(CHIP_CAP).filter(|extra| *extra > 0)
}

fn overflow_chip<'a>(extra: usize) -> Element<'a, Message> {
  container(
    text(format!("+{extra}"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding([spacing::UNIT / 2.0, spacing::UNIT + 2.0])
  .into()
}

fn footer(model: &CorpCardModel) -> Element<'_, Message> {
  Row::with_children(vec![
    stat(
      t!("roster.card.members").into_owned(),
      format_members(model.members),
      true,
    ),
    rule::vertical(STAT_DIVIDER_HEIGHT),
    stat(
      t!("roster.card.tax_rate").into_owned(),
      format_tax(model.tax_rate),
      true,
    ),
    rule::vertical(STAT_DIVIDER_HEIGHT),
    stat(
      t!("roster.card.ceo").into_owned(),
      model.ceo.clone().unwrap_or_else(|| PLACEHOLDER.to_owned()),
      false,
    ),
  ])
  .width(Length::Fill)
  .align_y(Vertical::Center)
  .into()
}

fn stat<'a>(label: String, value: String, mono: bool) -> Element<'a, Message> {
  let value_font = if mono {
    typography::mono::MEDIUM
  } else {
    typography::body::REGULAR
  };

  let label = text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let value = text(value)
    .font(value_font)
    .size(typography::size::MD)
    .wrapping(text::Wrapping::None)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  container(Column::with_children(vec![label.into(), value.into()]).spacing(spacing::UNIT))
    .width(Length::Fill)
    .padding(Padding {
      top: STAT_PAD_Y,
      right: CARD_PAD_X,
      bottom: STAT_PAD_Y,
      left: CARD_PAD_X,
    })
    .into()
}

fn sync_indicator<'a>(failure: Option<Phase>) -> Option<Element<'a, Message>> {
  match failure? {
    Phase::BackingOff | Phase::Failed => {}
    Phase::Blocked | Phase::Done | Phase::Empty | Phase::NotReady | Phase::Syncing => {
      return None;
    }
  }

  Some(
    container(
      text(t!("roster.card.needs_reauthentication"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        }),
    )
    .padding(Padding {
      top: 0.0,
      right: CARD_PAD_X,
      bottom: spacing::SPACE_3,
      left: CARD_PAD_X,
    })
    .into(),
  )
}

fn card_surface(needs_reauth: bool) -> impl Fn(&iced::Theme) -> container::Style {
  move |theme: &iced::Theme| {
    let mut style = crate::ui::style::control::card(theme);
    if needs_reauth {
      style.border.color = color::with_alpha(color::status::DANGER, TOKEN_BORDER_ALPHA);
    }
    style
  }
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
      grouped.push('\u{2009}'); // thin space (U+2009) as thousands separator
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
    use super::{super::super::card::TagChip, *};

    #[test]
    fn it_renders_a_tagless_corp_card() {
      let model = base_model();

      let _el: Element<'_, Message> = corp_compact_card(&model, None);
    }

    #[test]
    fn it_renders_a_corp_card_with_capped_tags() {
      let mut model = base_model();
      model.tags = (0..5)
        .map(|id| TagChip {
          color: Some(color::accent()),
          id,
          name: format!("Tag{id}"),
        })
        .collect();

      let _el: Element<'_, Message> = corp_compact_card(&model, None);
    }

    #[test]
    fn it_renders_an_unaffiliated_corp_with_placeholders() {
      let mut model = base_model();
      model.alliance = None;
      model.ceo = None;
      model.members = None;
      model.tax_rate = None;

      let _el: Element<'_, Message> = corp_compact_card(&model, None);
    }

    #[test]
    fn it_renders_a_reauth_corp_with_the_token_alert() {
      let mut model = base_model();
      model.needs_reauth = true;

      let _el: Element<'_, Message> = corp_compact_card(&model, None);
    }

    #[test]
    fn it_renders_a_corp_card_with_a_sync_error() {
      let model = base_model();

      for failure in [Phase::Failed, Phase::BackingOff] {
        let _el: Element<'_, Message> = corp_compact_card(&model, Some(failure));
      }
    }
  }

  mod overflow_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_the_tags_beyond_the_cap() {
      assert_eq!(overflow_count(5), Some(2));
    }

    #[test]
    fn it_stays_quiet_at_or_below_the_cap() {
      assert_eq!(overflow_count(3), None);
      assert_eq!(overflow_count(0), None);
    }
  }

  mod format_members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_millions_thin_space_thousands_and_raw_figures() {
      assert_eq!(format_members(Some(2_400_000)), "2.4M");
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
}
