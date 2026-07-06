use chrono::NaiveDate;
use iced::{
  Background, Border, Element, Length,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text},
};

use super::{Message as Parent, eve_date, prompts::Completeness};
use crate::{
  store::repo::captains_log::AnswerKey,
  ui::{
    components::{eyebrow::eyebrow, icon::Icon},
    format::fmt_isk,
    style::{color, radius, spacing, typography},
  },
};

const LIST_GAP: f32 = 6.0;
const MIDDOT: &str = "\u{b7}";
const MINUS: &str = "\u{2212}";
const PAST_ROW_PADDING: [f32; 2] = [12.0, 14.0];
const STATUS_ICON: f32 = 13.0;
const TODAY_ROW_PADDING: [f32; 2] = [13.0, 14.0];
const WARN: &str = "\u{26a0}";

#[derive(Clone, Debug)]
pub enum Message {
  Selected(Option<String>),
}

#[allow(dead_code)]
pub(super) struct DayEntry {
  pub completeness: Completeness,
  pub date_iso: String,
  pub narrative: Option<String>,
  pub summary: String,
}

#[allow(dead_code)]
pub(super) struct Log {
  pub past: Vec<DayEntry>,
  pub today: Today,
}

#[allow(dead_code)]
pub(super) struct Today {
  pub completeness: Completeness,
  pub date: NaiveDate,
  pub kill_count: usize,
  pub loss_count: usize,
  pub net_isk: f64,
  pub skill_count: usize,
}

impl Today {
  #[cfg(test)]
  fn empty() -> Self {
    Today {
      completeness: Completeness::default(),
      date: chrono::Utc::now().date_naive(),
      kill_count: 0,
      loss_count: 0,
      net_isk: 0.0,
      skill_count: 0,
    }
  }
}

#[allow(dead_code)]
pub(super) fn merged_days(logged: Vec<String>, active: Vec<String>) -> Vec<String> {
  let mut days: Vec<String> = logged.into_iter().chain(active).collect();
  days.sort_unstable();
  days.dedup();
  days.reverse();
  days
}

#[allow(dead_code)]
pub(super) fn render(log: &Log, selected: Option<&str>) -> Element<'static, Parent> {
  let flagged = flagged_count(log);

  let mut header = vec![day_count_header(1 + log.past.len())];
  if flagged > 0 {
    header.push(flagged_banner(flagged));
  }

  let mut rows: Vec<Element<'static, Parent>> = vec![today_row(&log.today, selected.is_none())];
  for entry in &log.past {
    let active = selected == Some(entry.date_iso.as_str());
    rows.push(past_row(entry, active));
  }

  header.push(Column::with_children(rows).spacing(LIST_GAP).width(Length::Fill).into());

  Column::with_children(header)
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn day_count_header(total: usize) -> Element<'static, Parent> {
  let count = text(t!("captains_log.entries.days", count => total).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  Row::with_children(vec![
    eyebrow(&t!("captains_log.entries.kicker").into_owned(), None),
    Space::new().width(Length::Fill).into(),
    count.into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom)
  .into()
}

fn flagged_banner(flagged: usize) -> Element<'static, Parent> {
  let label = if flagged == 1 {
    t!("captains_log.entries.needs_info_one", count => flagged)
  } else {
    t!("captains_log.entries.needs_info_other", count => flagged)
  };

  container(
    Row::with_children(vec![
      warn_glyph(STATUS_ICON),
      text(label.into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::status::WARNING))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding([8.0, 11.0])
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.1))),
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.3),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn flagged_count(log: &Log) -> usize {
  let today = usize::from(!log.today.completeness.is_complete());
  let past = log
    .past
    .iter()
    .filter(|entry| !entry.completeness.is_complete())
    .count();
  today + past
}

fn human_past(date_iso: &str) -> String {
  NaiveDate::parse_from_str(date_iso, "%Y-%m-%d")
    .map(|date| date.format("%a \u{b7} %-d %b").to_string())
    .unwrap_or_else(|_| date_iso.to_owned())
}

fn missing_labels(completeness: &Completeness) -> Vec<String> {
  let mut labels: Vec<String> = completeness
    .missing_prompts
    .iter()
    .map(|key| prompt_label(*key))
    .collect();
  if !completeness.missing_debriefs.is_empty() {
    labels.push(t!("captains_log.entries.missing.debrief").into_owned());
  }
  labels
}

fn narrative_quote(narrative: &str) -> Element<'static, Parent> {
  text(format!("\u{201c}{narrative}\u{201d}"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn past_row(entry: &DayEntry, selected: bool) -> Element<'static, Parent> {
  let status = if entry.completeness.is_complete() {
    Icon::check().size(STATUS_ICON).color(color::status::ONLINE).render()
  } else {
    warn_glyph(STATUS_ICON)
  };

  let head = Row::with_children(vec![
    text(human_past(&entry.date_iso))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Space::new().width(Length::Fill).into(),
    status,
  ])
  .align_y(Vertical::Bottom);

  let mut body = vec![head.into()];
  if !entry.summary.is_empty() {
    body.push(
      text(entry.summary.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  let labels = missing_labels(&entry.completeness);
  if !labels.is_empty() {
    body.push(warn_line(&labels));
  } else if let Some(narrative) = &entry.narrative {
    body.push(narrative_quote(narrative));
  }

  row_button(body, Some(entry.date_iso.clone()), selected, PAST_ROW_PADDING)
}

fn prompt_label(key: AnswerKey) -> String {
  match key {
    AnswerKey::Goal => t!("captains_log.entries.missing.goal").into_owned(),
    other => other.as_key().to_owned(),
  }
}

fn row_button(
  body: Vec<Element<'static, Parent>>,
  day: Option<String>,
  selected: bool,
  padding: [f32; 2],
) -> Element<'static, Parent> {
  let content = Column::with_children(body)
    .spacing(spacing::UNIT + 2.0)
    .width(Length::Fill);

  button(content)
    .width(Length::Fill)
    .padding(padding)
    .on_press(Parent::Entries(Message::Selected(day)))
    .style(move |_, status| row_style(selected, status))
    .into()
}

fn row_style(selected: bool, status: button::Status) -> button::Style {
  let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);

  let background = if selected {
    Some(Background::Color(color::with_alpha(color::accent(), 0.1)))
  } else if hover {
    Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.03)))
  } else {
    None
  };

  button::Style {
    background,
    border: Border {
      color: if selected {
        color::with_alpha(color::accent(), 0.35)
      } else {
        color::rule()
      },
      radius: radius::NAV_CARD.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn signed_isk(value: f64) -> String {
  let sign = if value < 0.0 { MINUS } else { "+" };
  format!("{sign}{}", fmt_isk(value.abs()))
}

fn summary_line(today: &Today) -> Element<'static, Parent> {
  let net_color = if today.net_isk >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let muted = |value: String| {
    text(value)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
  };

  Row::with_children(vec![
    muted(t!("captains_log.entries.skills", count => today.skill_count).into_owned()).into(),
    muted(format!(" {MIDDOT} ")).into(),
    text(signed_isk(today.net_isk))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(net_color))
      .into(),
    muted(format!(" {MIDDOT} ")).into(),
    muted(t!("captains_log.entries.combat_tally", kills => today.kill_count, losses => today.loss_count).into_owned())
      .into(),
  ])
  .into()
}

fn today_notes_pill(count: usize) -> Element<'static, Parent> {
  let label = if count == 1 {
    t!("captains_log.entries.notes_needed_one", count => count)
  } else {
    t!("captains_log.entries.notes_needed_other", count => count)
  };

  container(
    Row::with_children(vec![
      warn_glyph(11.0),
      text(label.into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::status::WARNING))
        .into(),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center),
  )
  .padding([3.0, 8.0])
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.16))),
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.42),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn today_row(today: &Today, selected: bool) -> Element<'static, Parent> {
  let head = Row::with_children(vec![
    text(t!("captains_log.today").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD + 1.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Space::new().width(Length::Fill).into(),
    text(eve_date::label(today.date))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::accent()))
      .into(),
  ])
  .align_y(Vertical::Bottom);

  let mut body = vec![head.into(), summary_line(today)];
  let missing = today.completeness.missing_prompts.len() + today.completeness.missing_debriefs.len();
  if missing > 0 {
    body.push(
      Row::with_children(vec![today_notes_pill(missing)])
        .width(Length::Fill)
        .into(),
    );
  }

  row_button(body, None, selected, TODAY_ROW_PADDING)
}

fn warn_glyph(size: f32) -> Element<'static, Parent> {
  text(WARN)
    .font(typography::body::REGULAR)
    .size(size)
    .style(typography::colored(color::status::WARNING))
    .into()
}

fn warn_line(labels: &[String]) -> Element<'static, Parent> {
  text(format!("{WARN} {}", labels.join(&format!(" {MIDDOT} "))))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::status::WARNING))
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::roster::captains_log::prompts::LossEngagement;

  fn day(date_iso: &str, completeness: Completeness) -> DayEntry {
    DayEntry {
      completeness,
      date_iso: date_iso.to_owned(),
      narrative: None,
      summary: String::new(),
    }
  }

  fn incomplete() -> Completeness {
    Completeness {
      missing_debriefs: Vec::new(),
      missing_prompts: vec![AnswerKey::Goal],
    }
  }

  mod merged_days {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_unions_logged_and_active_days_newest_first() {
      let logged = vec!["2026-07-04".to_owned(), "2026-07-02".to_owned()];
      let active = vec!["2026-07-05".to_owned(), "2026-07-03".to_owned()];

      let days = merged_days(logged, active);

      assert_eq!(
        days,
        vec![
          "2026-07-05".to_owned(),
          "2026-07-04".to_owned(),
          "2026-07-03".to_owned(),
          "2026-07-02".to_owned(),
        ]
      );
    }

    #[test]
    fn it_dedupes_days_present_in_both_sources() {
      let logged = vec!["2026-07-05".to_owned(), "2026-07-04".to_owned()];
      let active = vec!["2026-07-05".to_owned(), "2026-07-03".to_owned()];

      let days = merged_days(logged, active);

      assert_eq!(
        days,
        vec![
          "2026-07-05".to_owned(),
          "2026-07-04".to_owned(),
          "2026-07-03".to_owned(),
        ]
      );
    }

    #[test]
    fn it_never_fabricates_days_from_empty_sources() {
      assert!(merged_days(Vec::new(), Vec::new()).is_empty());
    }

    #[test]
    fn it_lists_a_logged_only_day() {
      let days = merged_days(vec!["2026-07-04".to_owned()], Vec::new());

      assert_eq!(days, vec!["2026-07-04".to_owned()]);
    }

    #[test]
    fn it_lists_an_activity_only_day() {
      let days = merged_days(Vec::new(), vec!["2026-07-04".to_owned()]);

      assert_eq!(days, vec!["2026-07-04".to_owned()]);
    }
  }

  mod flagged_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_today_when_it_is_incomplete() {
      let log = Log {
        past: Vec::new(),
        today: Today {
          completeness: incomplete(),
          ..Today::empty()
        },
      };

      assert_eq!(flagged_count(&log), 1);
    }

    #[test]
    fn it_sums_incomplete_today_and_past_days() {
      let log = Log {
        past: vec![
          day("2026-07-04", incomplete()),
          day("2026-07-03", Completeness::default()),
        ],
        today: Today {
          completeness: incomplete(),
          ..Today::empty()
        },
      };

      assert_eq!(flagged_count(&log), 2);
    }

    #[test]
    fn it_reports_zero_when_everything_is_complete() {
      let log = Log {
        past: vec![day("2026-07-04", Completeness::default())],
        today: Today::empty(),
      };

      assert_eq!(flagged_count(&log), 0);
    }
  }

  mod missing_labels {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_labels_a_missing_goal() {
      let labels = missing_labels(&incomplete());

      assert_eq!(labels, vec![t!("captains_log.entries.missing.goal").into_owned()]);
    }

    #[test]
    fn it_labels_a_missing_debrief() {
      let completeness = Completeness {
        missing_debriefs: vec![LossEngagement {
          character_id: 4,
          killmail_id: 100,
        }],
        missing_prompts: Vec::new(),
      };

      let labels = missing_labels(&completeness);

      assert_eq!(labels, vec![t!("captains_log.entries.missing.debrief").into_owned()]);
    }

    #[test]
    fn it_reports_no_labels_for_a_complete_day() {
      assert!(missing_labels(&Completeness::default()).is_empty());
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

  mod human_past {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_a_short_weekday_and_day() {
      assert_eq!(human_past("2026-07-04"), "Sat \u{b7} 4 Jul");
    }

    #[test]
    fn it_falls_back_to_the_raw_string_when_unparseable() {
      assert_eq!(human_past("not-a-date"), "not-a-date");
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_a_log_with_today_and_past_days() {
      let log = Log {
        past: vec![
          day("2026-07-04", incomplete()),
          day("2026-07-03", Completeness::default()),
        ],
        today: Today::empty(),
      };

      let _el: Element<'_, Parent> = render(&log, None);
    }
  }
}
