use chrono::{DateTime, Datelike, Timelike, Utc};
use iced::{
  Background, Border, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use super::{Activity, Filter, GroupBy, IndustryJob, Message, State};
pub(super) use crate::ui::format::{fmt_duration, fmt_isk};
use crate::{
  store::images::IconResolution,
  ui::{
    components::{
      clip::clip_layer,
      icon::Icon,
      icon_tile::icon_tile,
      resizable_pane::pane_handle,
      rule,
      virtual_list::{self, VirtualList, VirtualListConfig},
    },
    style::{color, radius, spacing, typography},
  },
};

const COUNTDOWN_WARNING_SECS: i64 = 3_600;
/// Estimated visual-row height (px) feeding the [`VirtualList`] windowing math. Job rows are two-line
/// with generous padding; group headers are shorter, but overscan absorbs the variance.
const ESTIMATED_ROW_HEIGHT: f32 = 74.0;
const ROW_SIDE_PADDING: f32 = 24.0;
const TILE_BOX_COMFORTABLE: f32 = 40.0;
const VALUE_WIDTH: f32 = 132.0;

pub(super) fn tab<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let view = state.job_view();
  let jobs = state.jobs();

  let body: Element<'a, Message> = if view.rows.is_empty() {
    empty_state()
  } else {
    let offset = state.jobs_scroll_offset();
    let rows = &view.rows;
    virtual_list::responsive_window(move |viewport_height| {
      let config = VirtualListConfig::new(rows.len(), ESTIMATED_ROW_HEIGHT)
        .viewport_height(viewport_height)
        .scroll_offset(offset);
      let windowed = VirtualList::new(config, |index| match &rows[index] {
        JobRowItem::Header(header) => group_header(&header.label, header.count, header.ready),
        JobRowItem::Job(job_index) => job_row(&jobs[*job_index], now),
      })
      .view();
      scrollable(windowed)
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_scroll(|viewport| Message::JobsScrolled {
          absolute: viewport.absolute_offset().y,
        })
        .into()
    })
  };

  let left = Column::with_children(vec![filter_bar(state, view.counts), body])
    .width(Length::Fill)
    .height(Length::Fill);

  let children: Vec<Element<'a, Message>> = vec![
    container(left).width(Length::Fill).height(Length::Fill).into(),
    pane_handle(Message::RailPaneDragStart),
    super::side_rail::rail(state, now),
  ];

  Row::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

pub(super) fn activity_chip<'a>(activity: Activity, small: bool) -> Element<'a, Message> {
  let fill = activity_color(activity);
  let size = if small {
    typography::size::XS
  } else {
    typography::size::XS_PLUS
  };
  container(
    text(activity.short())
      .font(typography::mono::REGULAR)
      .size(size)
      .style(typography::colored(fill)),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(fill, 0.14))),
    border: Border {
      color: color::with_alpha(fill, 0.28),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

pub(super) fn activity_color(activity: Activity) -> iced::Color {
  match activity {
    Activity::Manufacturing => color::accent::PLASMA,
    Activity::Reactions => color::status::DANGER,
    Activity::Invention => color::chart::VIOLET,
    Activity::Copy => color::chart::GOLD,
    Activity::MaterialEfficiency => color::status::ONLINE,
    Activity::TimeEfficiency => color::status::WARNING,
    Activity::Other => color::text::secondary(),
  }
}

pub(super) fn progress_bar<'a>(pct: f32, fill: iced::Color, height: f32, glow: bool) -> Element<'a, Message> {
  let pct = pct.clamp(0.0, 100.0);
  let _ = glow;
  // Scale the 0..=100 percentage to integer fill portions, keeping one decimal of precision (0..=1000)
  // so filled and remainder split proportionally rather than truncating pct straight to a u16.
  let filled = container(Space::new())
    .width(Length::FillPortion((pct * 10.0) as u16))
    .height(Length::Fixed(height))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: (height / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let rest_portion = ((100.0 - pct) * 10.0) as u16;
  let mut bar: Vec<Element<'a, Message>> = vec![filled.into()];
  if rest_portion > 0 {
    bar.push(Space::new().width(Length::FillPortion(rest_portion)).into());
  }

  container(Row::with_children(bar).width(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(height))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
      border: Border {
        radius: (height / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

pub(super) fn sec_pill<'a>(security: Option<f64>) -> Element<'a, Message> {
  let (label, fill) = match security {
    Some(sec) if sec > 0.45 => (format!("{sec:.1}"), color::status::ONLINE),
    Some(sec) if sec > 0.0 => (format!("{sec:.1}"), color::status::WARNING),
    Some(sec) => (format!("{sec:.1}"), color::status::DANGER),
    None => ("\u{2014}".to_owned(), color::text::tertiary()),
  };
  text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(fill))
    .into()
}

fn blueprint_tile<'a>(blueprint_icon: &IconResolution, box_size: f32) -> Element<'a, Message> {
  match blueprint_icon {
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
    IconResolution::Missing => icon_tile(
      Icon::notif_industry()
        .color(color::text::tertiary())
        .size(box_size * 0.45)
        .render::<Message>(),
      box_size,
    ),
  }
}

fn filter_bar<'a>(state: &'a State, counts: Counts) -> Element<'a, Message> {
  let chips = [
    ("All", Filter::All, counts.total, color::accent::PLASMA),
    ("In progress", Filter::Active, counts.active, color::accent::PLASMA),
    ("Ready", Filter::Ready, counts.ready, color::status::ONLINE),
  ];
  let chip_buttons: Vec<Element<'a, Message>> = chips
    .into_iter()
    .map(|(label, filter, count, accent)| filter_chip(label, filter, count, accent, state.filter() == filter))
    .collect();

  let chip_group = container(Row::with_children(chip_buttons).spacing(spacing::UNIT))
    .padding(3.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let group_buttons: Vec<Element<'a, Message>> = [
    ("None", GroupBy::None),
    ("Owner", GroupBy::Owner),
    ("Activity", GroupBy::Activity),
    ("Facility", GroupBy::Facility),
  ]
  .into_iter()
  .map(|(label, group_by)| group_button(label, group_by, state.group_by() == group_by))
  .collect();

  let band = Row::with_children(vec![
    chip_group.into(),
    Space::new().width(Length::Fill).into(),
    text("GROUP")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Row::with_children(group_buttons)
      .spacing(2.0)
      .align_y(Vertical::Center)
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let bar = container(band)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: ROW_SIDE_PADDING,
      right: ROW_SIDE_PADDING,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  Column::with_children(vec![bar.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn filter_chip<'a>(
  label: &str,
  filter: Filter,
  count: usize,
  accent: iced::Color,
  active: bool,
) -> Element<'a, Message> {
  let text_color = if active { accent } else { color::text::secondary() };
  let inner = Row::with_children(vec![
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(text_color))
      .into(),
    text(count.to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(text_color))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(inner)
    .padding(Padding {
      top: spacing::UNIT + 1.0,
      bottom: spacing::UNIT + 1.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::FilterSelected(filter))
    .style(move |_, _| button::Style {
      background: active.then(|| Background::Color(color::with_alpha(accent, 0.14))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      text_color,
      ..button::Style::default()
    })
    .into()
}

pub(super) fn fmt_clock(time: DateTime<Utc>) -> String {
  format!("{:02}:{:02}", time.hour(), time.minute())
}

pub(super) fn fmt_day(time: DateTime<Utc>) -> String {
  const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  ];
  format!("{} {}", MONTHS[(time.month0() as usize).min(11)], time.day())
}

fn group_button<'a>(label: &str, group_by: GroupBy, active: bool) -> Element<'a, Message> {
  let text_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };
  button(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: spacing::UNIT + 1.0,
    bottom: spacing::UNIT + 1.0,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  })
  .on_press(Message::GroupBySelected(group_by))
  .style(move |_, _| button::Style {
    background: active.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
    border: Border {
      color: if active {
        color::rule_strong()
      } else {
        iced::Color::TRANSPARENT
      },
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn group_header<'a>(label: &str, count: usize, ready: usize) -> Element<'a, Message> {
  let band = Row::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(format!("{count} {}", if count == 1 { "job" } else { "jobs" }))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Space::new().width(Length::Fill).into(),
    text(format!("{ready} ready"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(band)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: ROW_SIDE_PADDING,
      right: ROW_SIDE_PADDING,
    })
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

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("No jobs match this filter.")
      .font(typography::body::REGULAR)
      .size(typography::size::LG)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6)
  .into()
}

fn job_row<'a>(job: &'a IndustryJob, now: DateTime<Utc>) -> Element<'a, Message> {
  let ready = job.is_ready(now);
  let pct = job.progress(now);
  let remaining = job.remaining_seconds(now);

  let bar = if ready {
    color::status::ONLINE
  } else {
    activity_color(job.activity)
  };

  let identity = Row::with_children(vec![
    blueprint_tile(&job.blueprint_icon, TILE_BOX_COMFORTABLE),
    job_identity(job),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let progress = Column::with_children(vec![
    Row::with_children(vec![
      text(if ready {
        "COMPLETE".to_owned()
      } else {
        format!("{}%", pct.floor() as i64)
      })
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(if ready {
        color::status::ONLINE
      } else {
        color::text::secondary()
      }))
      .into(),
      Space::new().width(Length::Fill).into(),
      text(eta_label(job, now))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .into(),
    progress_bar(pct, bar, 9.0, !ready),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  let right = countdown_column(job, now, ready, remaining);

  let row = Row::with_children(vec![
    container(identity).width(Length::FillPortion(5)).into(),
    container(progress).width(Length::FillPortion(4)).into(),
    container(right).width(Length::Fixed(VALUE_WIDTH)).into(),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3_5,
      left: ROW_SIDE_PADDING,
      right: ROW_SIDE_PADDING,
    })
    .style(move |_| container::Style {
      background: ready.then(|| Background::Color(color::with_alpha(color::status::ONLINE, 0.07))),
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn countdown_column<'a>(job: &'a IndustryJob, now: DateTime<Utc>, ready: bool, remaining: i64) -> Element<'a, Message> {
  let _ = now;
  let countdown: Element<'a, Message> = if ready {
    Row::with_children(vec![
      Icon::check()
        .color(color::status::ONLINE)
        .size(13.0)
        .render::<Message>(),
      text("Ready")
        .font(typography::mono::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::status::ONLINE))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
  } else {
    let countdown_color = if remaining < COUNTDOWN_WARNING_SECS {
      color::status::WARNING
    } else {
      color::text::PRIMARY
    };
    text(fmt_duration(remaining))
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(countdown_color))
      .into()
  };

  let (value_text, value_color) = match job.value {
    Some(value) if value > 0.0 => (format!("{} out", fmt_isk(value)), color::accent::PLASMA),
    _ => (
      match job.activity {
        Activity::Invention => "invention".to_owned(),
        Activity::Copy => "copy".to_owned(),
        _ => "\u{2014}".to_owned(),
      },
      color::text::tertiary(),
    ),
  };

  Column::with_children(vec![
    countdown,
    text(value_text)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(value_color))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_x(Horizontal::Right)
  .width(Length::Fill)
  .into()
}

fn eta_label(job: &IndustryJob, now: DateTime<Utc>) -> String {
  match job.end() {
    Some(end) if job.is_ready(now) => format!("done {}", fmt_clock(end)),
    Some(end) => format!("\u{2192} {} {}", fmt_day(end), fmt_clock(end)),
    None => "\u{2014}".to_owned(),
  }
}

fn job_identity<'a>(job: &'a IndustryJob) -> Element<'a, Message> {
  let name_size = typography::size::LG;

  let mut first_line: Vec<Element<'a, Message>> = vec![
    text(job.product_name.clone())
      .font(typography::body::MEDIUM)
      .size(name_size)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    activity_chip(job.activity, true),
  ];
  if job.activity == Activity::Invention
    && let Some(prob) = job.probability
  {
    first_line.push(
      text(format!("{}% success", (prob * 100.0).round() as i64))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  let runs_word = match job.activity {
    Activity::Copy => "copies",
    Activity::Invention => "tries",
    _ => "runs",
  };

  let mut second_line: Vec<Element<'a, Message>> = vec![
    text(format!("\u{00D7}{} {runs_word}", job.runs))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    dot(),
    text(job.facility.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
    sec_pill(job.security),
  ];
  if !job.installer.is_empty() {
    second_line.push(dot());
    second_line.push(
      text(job.installer.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  Column::with_children(vec![
    Row::with_children(first_line)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    Row::with_children(second_line)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill)
  .into()
}

fn dot<'a>() -> Element<'a, Message> {
  text("\u{00B7}")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Counts {
  pub active: usize,
  pub ready: usize,
  pub total: usize,
}

#[derive(Clone, Debug)]
pub(super) struct GroupHeader {
  pub count: usize,
  pub label: String,
  pub ready: usize,
}

#[derive(Clone, Debug)]
pub(super) enum JobRowItem {
  Header(GroupHeader),
  /// Index into `State::jobs()` — the full, unfiltered jobs slice, not the visible or filtered subset.
  Job(usize),
}

#[derive(Clone, Debug, Default)]
pub(super) struct JobView {
  pub counts: Counts,
  pub rows: Vec<JobRowItem>,
}

impl JobView {
  pub fn build(jobs: &[IndustryJob], visible: &[usize], filter: Filter, group_by: GroupBy, now: DateTime<Utc>) -> Self {
    let counts = counts_of(jobs, visible, now);
    let filtered = filter_and_sort(jobs, visible, filter, now);
    let rows = group_rows(jobs, &filtered, group_by, now);

    JobView {
      counts,
      rows,
    }
  }
}

fn counts_of(jobs: &[IndustryJob], visible: &[usize], now: DateTime<Utc>) -> Counts {
  let ready = visible.iter().filter(|&&index| jobs[index].is_ready(now)).count();
  Counts {
    active: visible.len() - ready,
    ready,
    total: visible.len(),
  }
}

fn filter_and_sort(jobs: &[IndustryJob], visible: &[usize], filter: Filter, now: DateTime<Utc>) -> Vec<usize> {
  let mut out: Vec<usize> = visible
    .iter()
    .copied()
    .filter(|&index| match filter {
      Filter::All => true,
      Filter::Active => !jobs[index].is_ready(now),
      Filter::Ready => jobs[index].is_ready(now),
    })
    .collect();
  out.sort_by(|&a, &b| {
    let (a, b) = (&jobs[a], &jobs[b]);
    let ready = b.is_ready(now).cmp(&a.is_ready(now));
    ready
      .then_with(|| a.end().cmp(&b.end()))
      .then_with(|| a.job_id.cmp(&b.job_id))
  });
  out
}

fn group_rows(jobs: &[IndustryJob], filtered: &[usize], group_by: GroupBy, now: DateTime<Utc>) -> Vec<JobRowItem> {
  if group_by == GroupBy::None {
    return filtered.iter().copied().map(JobRowItem::Job).collect();
  }

  let mut order: Vec<String> = Vec::new();
  let mut buckets: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
  for &index in filtered {
    let job = &jobs[index];
    let label = match group_by {
      GroupBy::Owner => job.owner_name.clone(),
      GroupBy::Activity => job.activity.label().to_owned(),
      GroupBy::Facility => job.facility.clone(),
      GroupBy::None => String::new(),
    };
    if !buckets.contains_key(&label) {
      order.push(label.clone());
    }
    buckets.entry(label).or_default().push(index);
  }

  let mut rows: Vec<JobRowItem> = Vec::with_capacity(filtered.len() + order.len());
  for label in order {
    let members = buckets.remove(&label).unwrap_or_default();
    let ready = members.iter().filter(|&&index| jobs[index].is_ready(now)).count();
    rows.push(JobRowItem::Header(GroupHeader {
      count: members.len(),
      label,
      ready,
    }));
    rows.extend(members.into_iter().map(JobRowItem::Job));
  }
  rows
}
