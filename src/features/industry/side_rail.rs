use std::collections::HashMap;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, scrollable, text},
};

use super::{
  Activity, IndustryJob, Message, Owner, RosterOwner, Scope, State,
  jobs::{activity_chip, activity_color, progress_bar},
  loaders::SlotBucket,
};
use crate::ui::{
  components::{eyebrow::eyebrow_text, icon_tile::icon_tile, rule},
  style::{color, radius, spacing, typography},
};

const RAIL_WIDTH: f32 = 280.0;
const SLOT_GAP: f32 = 3.0;
const SLOT_HEIGHT: f32 = 6.0;

pub(super) fn rail<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let jobs = state.visible_jobs();
  let body = Column::with_children(vec![
    slots_section(state),
    rule::horizontal(),
    next_section(&jobs, now),
    rule::horizontal(),
    activity_section(&jobs),
  ])
  .width(Length::Fill);

  container(scrollable(body).style(crate::ui::style::control::scrollbar))
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn activity_section<'a>(jobs: &[&'a IndustryJob]) -> Element<'a, Message> {
  let mut counts: HashMap<Activity, usize> = HashMap::new();
  for job in jobs {
    *counts.entry(job.activity).or_default() += 1;
  }
  let mut ordered: Vec<(Activity, usize)> = counts.into_iter().collect();
  ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.short().cmp(b.0.short())));
  let max = ordered.first().map(|(_, n)| *n).unwrap_or(0).max(1);

  let rows: Vec<Element<'a, Message>> = ordered
    .into_iter()
    .map(|(activity, count)| {
      Row::with_children(vec![
        container(activity_chip(activity, true))
          .width(Length::Fixed(62.0))
          .into(),
        container(progress_bar(
          (count as f32 / max as f32) * 100.0,
          activity_color(activity),
          5.0,
          false,
        ))
        .width(Length::Fill)
        .into(),
        text(count.to_string())
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
      ])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center)
      .into()
    })
    .collect();

  section(
    "Activity mix",
    Column::with_children(rows).spacing(spacing::SPACE_2).into(),
  )
}

fn next_section<'a>(jobs: &[&'a IndustryJob], now: DateTime<Utc>) -> Element<'a, Message> {
  let mut active: Vec<&IndustryJob> = jobs.iter().copied().filter(|job| !job.is_ready(now)).collect();
  active.sort_by_key(|job| job.end());
  active.truncate(5);

  let content: Element<'a, Message> = if active.is_empty() {
    text("Nothing in progress.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into()
  } else {
    let rows: Vec<Element<'a, Message>> = active.into_iter().map(|job| next_row(job, now)).collect();
    Column::with_children(rows).spacing(spacing::SPACE_3).into()
  };

  section("Next to complete", content)
}

fn next_row<'a>(job: &'a IndustryJob, now: DateTime<Utc>) -> Element<'a, Message> {
  let remaining = job.remaining_seconds(now);
  let countdown_color = if remaining < 3_600 {
    color::status::WARNING
  } else {
    color::text::secondary()
  };

  Row::with_children(vec![
    icon_tile(Space::new(), 24.0),
    Column::with_children(vec![
      text(job.product_name.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      text(format!(
        "{} \u{00B7} {}",
        job.activity.short(),
        job.system_name.clone().unwrap_or_else(|| "\u{2014}".to_owned())
      ))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
    ])
    .spacing(2.0)
    .width(Length::Fill)
    .into(),
    text(super::jobs::fmt_duration(remaining))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(countdown_color))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn owner_meters<'a>(owner: &'a RosterOwner, used: &SlotUsage) -> Element<'a, Message> {
  let mut meters: Vec<Element<'a, Message>> = vec![slot_meter(
    "Manuf.",
    used.manufacturing,
    owner.slots.manufacturing,
    color::accent::PLASMA,
  )];
  if owner.slots.reactions > 0 {
    meters.push(slot_meter(
      "React.",
      used.reactions,
      owner.slots.reactions,
      color::status::DANGER,
    ));
  }
  meters.push(slot_meter(
    "Science",
    used.science,
    owner.slots.science,
    color::chart::VIOLET,
  ));

  Column::with_children(vec![
    text(owner.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Column::with_children(meters)
      .spacing(spacing::SPACE_2)
      .padding(Padding {
        left: spacing::SPACE_3,
        ..Padding::ZERO
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn section<'a>(label: &str, content: Element<'a, Message>) -> Element<'a, Message> {
  container(
    Column::with_children(vec![eyebrow_text(label, None).into(), content])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3_5)
  .into()
}

fn slot_meter<'a>(label: &str, used: i64, max: i64, fill: iced::Color) -> Element<'a, Message> {
  let full = max > 0 && used >= max;
  let pip_color = if full { color::status::WARNING } else { fill };

  let pips: Vec<Element<'a, Message>> = (0..max.max(0))
    .map(|index| {
      let on = index < used;
      container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(SLOT_HEIGHT))
        .style(move |_| container::Style {
          background: Some(Background::Color(if on {
            pip_color
          } else {
            color::with_alpha(color::text::PRIMARY, 0.08)
          })),
          border: Border {
            radius: radius::SUBTLE.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  let used_color = if full {
    color::status::WARNING
  } else {
    color::text::PRIMARY
  };

  Column::with_children(vec![
    Row::with_children(vec![
      text(label.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Space::new().width(Length::Fill).into(),
      text(format!("{used}/{max}"))
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(used_color))
        .into(),
    ])
    .align_y(Vertical::Center)
    .into(),
    Row::with_children(pips).spacing(SLOT_GAP).into(),
  ])
  .spacing(spacing::UNIT + 1.0)
  .into()
}

fn slots_section<'a>(state: &'a State) -> Element<'a, Message> {
  let owners = slot_pool(state);
  let used = slot_usage(state);

  let rows: Vec<Element<'a, Message>> = owners
    .into_iter()
    .map(|owner| {
      let usage = used.get(&owner_key(owner)).copied().unwrap_or_default();
      owner_meters(owner, &usage)
    })
    .collect();

  let content: Element<'a, Message> = if rows.is_empty() {
    text("No characters loaded.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into()
  } else {
    Column::with_children(rows).spacing(spacing::SPACE_3_5).into()
  };

  section("Job slots", content)
}

fn owner_key(owner: &RosterOwner) -> Owner {
  if owner.is_corporation {
    Owner::Corporation(owner.id)
  } else {
    Owner::Character(owner.id)
  }
}

fn slot_pool(state: &State) -> Vec<&RosterOwner> {
  match state.active() {
    Scope::All => state.roster().iter().collect(),
    Scope::Char(id) => state
      .roster()
      .iter()
      .filter(|owner| owner.id == id && !owner.is_corporation)
      .collect(),
    Scope::Corp(id) => state
      .roster()
      .iter()
      .filter(|owner| owner.is_corporation && owner.id == id)
      .collect(),
  }
}

fn slot_usage(state: &State) -> HashMap<Owner, SlotUsage> {
  let mut usage: HashMap<Owner, SlotUsage> = HashMap::new();
  for job in &state.visible_jobs() {
    let entry = usage.entry(job.owner).or_default();
    match job.activity.bucket() {
      SlotBucket::Manufacturing => entry.manufacturing += 1,
      SlotBucket::Reactions => entry.reactions += 1,
      SlotBucket::Science => entry.science += 1,
    }
  }
  usage
}

#[derive(Clone, Copy, Default)]
struct SlotUsage {
  manufacturing: i64,
  reactions: i64,
  science: i64,
}
