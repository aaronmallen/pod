use iced::{
  Background, Border, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, image, text},
};

use super::Message as Parent;
use crate::{
  store::{
    images::IconResolution,
    repo::captains_log_rollup::{DayMoney, NetWorthDelta},
  },
  ui::{
    components::{clip::clip_layer, eyebrow::eyebrow, icon::Icon, icon_tile::icon_tile, rule},
    format::fmt_isk,
    style::{color, radius, spacing, typography},
  },
};

const CHIP_GLYPH_BOX: f32 = 32.0;
const EM_DASH: &str = "\u{2014}";
const ENGAGEMENT_ICON: f32 = 30.0;
const INDUSTRY_CAP: usize = 3;
const LABEL_ICON: f32 = 16.0;
const MINUS: &str = "\u{2212}";
const VALUE_SIZE: f32 = 26.0;

pub(super) struct Engagement {
  pub character: String,
  pub icon: IconResolution,
  pub is_kill: bool,
  pub ship: String,
  pub system: String,
  pub time: String,
  pub value: f64,
}

pub(super) struct SkillLine {
  pub level: i64,
  pub skill: String,
}

pub(super) struct Summary {
  pub engagements: Vec<Engagement>,
  pub industry: Vec<String>,
  pub kill_count: usize,
  pub loss_count: usize,
  pub loss_value: f64,
  pub money: DayMoney,
  pub net_worth: Option<NetWorthDelta>,
  pub pilot_count: usize,
  pub skills: Vec<SkillLine>,
}

impl Summary {
  #[cfg(test)]
  pub(super) fn empty() -> Self {
    Summary {
      engagements: Vec::new(),
      industry: Vec::new(),
      kill_count: 0,
      loss_count: 0,
      loss_value: 0.0,
      money: DayMoney::default(),
      net_worth: None,
      pilot_count: 0,
      skills: Vec::new(),
    }
  }
}

pub(super) fn render(summary: &Summary) -> Element<'static, Parent> {
  let mut sections = vec![header(summary.pilot_count), tiles_row(summary)];
  if !summary.industry.is_empty() {
    sections.push(industry_chip(summary));
  }
  if !summary.engagements.is_empty() {
    sections.push(engagements_panel(summary));
  }

  Column::with_children(sections)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn activity_chip(icon: Icon, label: String, value: String) -> Element<'static, Parent> {
  let glyph = container(icon.size(17.0).color(color::text::secondary()).render())
    .width(Length::Fixed(CHIP_GLYPH_BOX))
    .height(Length::Fixed(CHIP_GLYPH_BOX))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      border: Border {
        color: iced::Color::TRANSPARENT,
        radius: radius::CONTROL.into(),
        width: 0.0,
      },
      ..container::Style::default()
    });

  let body = Column::with_children(vec![
    eyebrow(&label, None),
    text(value)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::UNIT - 1.0)
  .width(Length::Fill);

  container(
    Row::with_children(vec![glyph.into(), body.into()])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 11.0,
    right: 13.0,
    bottom: 11.0,
    left: 13.0,
  })
  .style(sunken_style)
  .into()
}

fn combat_tile(summary: &Summary) -> Element<'static, Parent> {
  let value = kills_losses(summary.kill_count, summary.loss_count);
  let sub = (summary.loss_count > 0).then(|| loss_value_sub(summary.loss_value));

  stat_tile(
    Icon::notif_combat(),
    t!("captains_log.rollup_tiles.kills_losses").into_owned(),
    value,
    color::text::PRIMARY,
    sub,
  )
}

fn engagement_row(engagement: &Engagement) -> Element<'static, Parent> {
  let value_color = kill_color(engagement.is_kill);
  let sign = if engagement.is_kill { "+" } else { MINUS };

  let info = Column::with_children(vec![
    text(engagement.ship.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    mono_small(
      format!(
        "{} \u{b7} {} \u{b7} {}",
        engagement.character, engagement.system, engagement.time
      ),
      color::text::tertiary(),
    ),
  ])
  .spacing(spacing::UNIT - 2.0)
  .width(Length::Fill);

  Row::with_children(vec![
    type_icon(&engagement.icon, ENGAGEMENT_ICON),
    info.into(),
    kind_badge(engagement.is_kill),
    mono_small(format!("{sign}{}", fmt_isk(engagement.value)), value_color),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn engagements_panel(summary: &Summary) -> Element<'static, Parent> {
  let mut header_row = vec![
    eyebrow(&t!("captains_log.rollup_tiles.engagements").into_owned(), None),
    Space::new().width(Length::Fill).into(),
  ];
  if summary.loss_value > 0.0 {
    header_row.push(mono_small(
      format!("{MINUS}{}", fmt_isk(summary.loss_value)),
      color::status::DANGER,
    ));
  }

  let mut children = vec![
    Row::with_children(header_row)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
  ];
  children.extend(summary.engagements.iter().map(engagement_row));

  container(Column::with_children(children).spacing(spacing::SPACE_2_5))
    .width(Length::Fill)
    .padding(Padding {
      top: 11.0,
      right: 14.0,
      bottom: 11.0,
      left: 14.0,
    })
    .style(sunken_style)
    .into()
}

fn header(pilot_count: usize) -> Element<'static, Parent> {
  let label = t!("captains_log.rollup_tiles.automated", count => pilot_count).into_owned();

  Row::with_children(vec![
    eyebrow(&label, None),
    container(rule::horizontal()).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn industry_chip(summary: &Summary) -> Element<'static, Parent> {
  let jobs = summary.industry.len();
  let jobs_label = if jobs == 1 {
    t!("captains_log.rollup_tiles.jobs_one", count => jobs)
  } else {
    t!("captains_log.rollup_tiles.jobs_other", count => jobs)
  };
  let value = format!("{jobs_label} \u{b7} {}", industry_summary(&summary.industry));

  activity_chip(
    Icon::industry(),
    t!("captains_log.rollup_tiles.industry").into_owned(),
    value,
  )
}

fn industry_summary(products: &[String]) -> String {
  let mut order: Vec<(&str, usize)> = Vec::new();
  for product in products {
    match order.iter_mut().find(|(name, _)| *name == product.as_str()) {
      Some(entry) => entry.1 += 1,
      None => order.push((product.as_str(), 1)),
    }
  }

  let shown: Vec<String> = order
    .iter()
    .take(INDUSTRY_CAP)
    .map(|(name, count)| {
      if *count > 1 {
        format!("{count}\u{d7} {name}")
      } else {
        (*name).to_owned()
      }
    })
    .collect();

  let mut joined = shown.join(", ");
  if order.len() > INDUSTRY_CAP {
    joined.push('\u{2026}');
  }
  joined
}

fn isk_net_sub(money: DayMoney) -> Element<'static, Parent> {
  Row::with_children(vec![
    mono_small(format!("\u{25b2} {}", fmt_isk(money.earned)), color::status::ONLINE),
    mono_small(format!("\u{25bc} {}", fmt_isk(money.spent)), color::status::DANGER),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn isk_net_tile(summary: &Summary) -> Element<'static, Parent> {
  let net = summary.money.net();

  stat_tile(
    Icon::wallet(),
    t!("captains_log.rollup_tiles.isk_net").into_owned(),
    signed_isk(net),
    signed_color(net),
    Some(isk_net_sub(summary.money)),
  )
}

fn kill_color(is_kill: bool) -> iced::Color {
  if is_kill {
    color::status::ONLINE
  } else {
    color::status::DANGER
  }
}

fn kills_losses(kills: usize, losses: usize) -> String {
  format!("{kills} / {losses}")
}

fn kind_badge(is_kill: bool) -> Element<'static, Parent> {
  let (key, tint) = if is_kill {
    ("captains_log.rollup_tiles.kill", color::status::ONLINE)
  } else {
    ("captains_log.rollup_tiles.loss", color::status::DANGER)
  };

  container(
    text(t!(key).to_uppercase())
      .font(typography::mono::SEMIBOLD)
      .size(typography::size::XS)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 3.0,
    right: 7.0,
    bottom: 3.0,
    left: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.12))),
    border: Border {
      color: color::with_alpha(tint, 0.4),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn loss_value_sub(loss_value: f64) -> Element<'static, Parent> {
  Row::with_children(vec![
    mono_small(format!("{MINUS}{}", fmt_isk(loss_value)), color::status::DANGER),
    mono_small(
      t!("captains_log.rollup_tiles.lost").into_owned(),
      color::text::secondary(),
    ),
  ])
  .spacing(spacing::UNIT)
  .into()
}

fn mono_small(value: String, fill: iced::Color) -> Element<'static, Parent> {
  text(value)
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(fill))
    .into()
}

fn net_delta_pct(delta: &NetWorthDelta) -> String {
  let sign = if delta.percent >= 0.0 { "+" } else { MINUS };
  format!("{sign}{:.2}%", delta.percent.abs())
}

fn net_worth_tile(summary: &Summary) -> Element<'static, Parent> {
  let label = t!("captains_log.rollup_tiles.net_worth_delta").into_owned();

  match &summary.net_worth {
    Some(delta) => stat_tile(
      Icon::pulse(),
      label,
      net_delta_pct(delta),
      signed_color(delta.percent),
      Some(mono_small(signed_isk(delta.isk), color::text::secondary())),
    ),
    None => stat_tile(
      Icon::pulse(),
      label,
      EM_DASH.to_owned(),
      color::text::secondary(),
      Some(mono_small(
        t!("captains_log.rollup_tiles.no_prior").into_owned(),
        color::text::secondary(),
      )),
    ),
  }
}

fn roman(level: i64) -> &'static str {
  match level {
    1 => "I",
    2 => "II",
    3 => "III",
    4 => "IV",
    5 => "V",
    _ => "",
  }
}

fn signed_color(value: f64) -> iced::Color {
  if value >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  }
}

fn signed_isk(value: f64) -> String {
  let sign = if value < 0.0 { MINUS } else { "+" };
  format!("{sign}{}", fmt_isk(value.abs()))
}

fn skills_sub(skills: &[SkillLine]) -> Option<String> {
  let first = skills.first()?;
  let head = format!("{} {}", first.skill, roman(first.level)).trim_end().to_owned();

  if skills.len() > 1 {
    Some(format!("{head} +{}", skills.len() - 1))
  } else {
    Some(head)
  }
}

fn skills_tile(summary: &Summary) -> Element<'static, Parent> {
  let sub = skills_sub(&summary.skills).map(|line| mono_small(line, color::text::secondary()));

  stat_tile(
    Icon::skills(),
    t!("captains_log.rollup_tiles.skills_done").into_owned(),
    summary.skills.len().to_string(),
    color::text::PRIMARY,
    sub,
  )
}

fn stat_tile(
  icon: Icon,
  label: String,
  value: String,
  value_color: iced::Color,
  sub: Option<Element<'static, Parent>>,
) -> Element<'static, Parent> {
  let head = Row::with_children(vec![
    icon.size(LABEL_ICON).color(color::text::secondary()).render(),
    eyebrow(&label, None),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let mut value_block = vec![
    text(value)
      .font(typography::mono::MEDIUM)
      .size(VALUE_SIZE)
      .style(typography::colored(value_color))
      .into(),
  ];
  if let Some(sub) = sub {
    value_block.push(sub);
  }

  container(
    Column::with_children(vec![
      head.into(),
      Column::with_children(value_block).spacing(spacing::UNIT + 2.0).into(),
    ])
    .spacing(spacing::SPACE_2 + 3.0),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 15.0,
    right: 17.0,
    bottom: 15.0,
    left: 17.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule(),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn sunken_style(_: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      radius: radius::NAV_CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  }
}

fn tiles_row(summary: &Summary) -> Element<'static, Parent> {
  Row::with_children(vec![
    isk_net_tile(summary),
    skills_tile(summary),
    combat_tile(summary),
    net_worth_tile(summary),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .into()
}

fn type_icon(icon: &IconResolution, box_size: f32) -> Element<'static, Parent> {
  match icon {
    IconResolution::Found(path) => icon_tile(
      clip_layer(
        image(image::Handle::from_path(path.clone()))
          .width(Length::Fill)
          .height(Length::Fill)
          .content_fit(ContentFit::Cover),
        Length::Fill,
        Length::Fill,
      ),
      box_size,
    ),
    IconResolution::Missing => icon_tile(Space::new(), box_size),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn skill(skill: &str, level: i64) -> SkillLine {
    SkillLine {
      level,
      skill: skill.to_owned(),
    }
  }

  fn engagement(ship: &str, is_kill: bool, value: f64) -> Engagement {
    Engagement {
      character: "Vex Voronova".to_owned(),
      icon: IconResolution::Missing,
      is_kill,
      ship: ship.to_owned(),
      system: "Tama".to_owned(),
      time: "21:00".to_owned(),
      value,
    }
  }

  mod signed_isk {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefixes_a_plus_for_zero_and_positive() {
      assert_eq!(signed_isk(0.0), "+0");
      assert_eq!(signed_isk(2_500_000.0), "+2.5M");
    }

    #[test]
    fn it_prefixes_a_unicode_minus_for_negative() {
      assert_eq!(signed_isk(-1_500_000_000.0), "\u{2212}1.5B");
    }
  }

  mod net_delta_pct {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_signs_and_fixes_two_decimals() {
      assert_eq!(
        net_delta_pct(&NetWorthDelta {
          isk: 1.0,
          percent: 2.4,
        }),
        "+2.40%"
      );
      assert_eq!(
        net_delta_pct(&NetWorthDelta {
          isk: -1.0,
          percent: -0.2,
        }),
        "\u{2212}0.20%"
      );
    }
  }

  mod kills_losses {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_joins_the_two_counts() {
      assert_eq!(kills_losses(4, 2), "4 / 2");
    }
  }

  mod roman {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_levels_one_through_five() {
      assert_eq!(roman(1), "I");
      assert_eq!(roman(5), "V");
      assert_eq!(roman(6), "");
    }
  }

  mod skills_sub {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_none_without_skills() {
      assert_eq!(skills_sub(&[]), None);
    }

    #[test]
    fn it_names_a_single_completion() {
      assert_eq!(skills_sub(&[skill("Gunnery", 5)]).as_deref(), Some("Gunnery V"));
    }

    #[test]
    fn it_counts_the_remaining_completions() {
      let skills = vec![skill("Caldari Cruiser", 5), skill("Drones", 4), skill("Shields", 3)];

      assert_eq!(skills_sub(&skills).as_deref(), Some("Caldari Cruiser V +2"));
    }
  }

  mod industry_summary {
    use pretty_assertions::assert_eq;

    use super::*;

    fn products(names: &[&str]) -> Vec<String> {
      names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn it_groups_repeats_with_a_multiplier() {
      let summary = industry_summary(&products(&["Covetor", "Covetor"]));

      assert_eq!(summary, "2\u{d7} Covetor");
    }

    #[test]
    fn it_caps_the_list_and_appends_an_ellipsis() {
      let summary = industry_summary(&products(&["Hulk", "Retriever", "Astero", "Stratios"]));

      assert_eq!(summary, "Hulk, Retriever, Astero\u{2026}");
    }

    #[test]
    fn it_joins_a_short_list_without_an_ellipsis() {
      let summary = industry_summary(&products(&["Hulk", "Retriever"]));

      assert_eq!(summary, "Hulk, Retriever");
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_empty_zero_state() {
      let _el: Element<'_, Parent> = render(&Summary::empty());
    }

    #[test]
    fn it_renders_a_busy_day_with_every_section() {
      let summary = Summary {
        engagements: vec![
          engagement("Loki", true, 612_000_000.0),
          engagement("Astero", false, 132_000_000.0),
        ],
        industry: vec!["Hulk".to_owned(), "Covetor".to_owned(), "Covetor".to_owned()],
        kill_count: 1,
        loss_count: 1,
        loss_value: 132_000_000.0,
        money: DayMoney {
          earned: 4_210_000_000.0,
          spent: 1_800_000_000.0,
        },
        net_worth: Some(NetWorthDelta {
          isk: 3_120_000_000.0,
          percent: 2.41,
        }),
        pilot_count: 6,
        skills: vec![skill("Caldari Cruiser", 5)],
      };

      let _el: Element<'_, Parent> = render(&summary);
    }
  }
}
