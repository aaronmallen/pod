use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use iced::{
  Background, Color, Element, Length,
  widget::{Space, container},
};

use super::{CalendarEvent, Message, State, palette};
use crate::config::CalendarWeekStart;

const ACCENT_WIDTH: f32 = 3.0;

/// The solid, full-saturation color bar drawn down the left edge of an event block (the design's
/// `border-left: 3px solid`). Rendered as a sibling strip rather than a border because iced borders
/// are uniform on all sides; callers place it as the first child of the block with zero left padding
/// so it sits flush against the edge.
pub(super) fn accent_strip<'a>(tint: Color) -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(ACCENT_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(tint)),
      ..container::Style::default()
    })
    .into()
}

pub(super) const MIN_BLOCK_MINUTES: i64 = 30;
pub(super) const MINUTES_PER_DAY: i64 = 1440;

#[derive(Clone, Copy)]
pub(super) struct Packed<'a> {
  pub end_minute: i64,
  pub event: &'a CalendarEvent,
  /// Zero-based column index within the overlap cluster.
  pub lane: usize,
  /// Total columns in the cluster; callers divide available width by this to size each lane.
  pub lanes: usize,
  pub start_minute: i64,
}

pub(super) fn color_for<'a>(state: &'a State, event: &'a CalendarEvent) -> Color {
  if state.tweaks().color_by_pilot() {
    palette::pilot_color(state.color_index_for(event.character_id))
  } else {
    event.owner_kind().color()
  }
}

pub(super) fn day_key(day: DateTime<Utc>) -> i64 {
  start_of_day(day).timestamp()
}

pub(super) fn events_on_day<'a>(events: &[&'a CalendarEvent], day: DateTime<Utc>) -> Vec<&'a CalendarEvent> {
  let key = day_key(day);
  let mut items: Vec<&CalendarEvent> = events
    .iter()
    .copied()
    .filter(|event| event.start().map(|start| day_key(start) == key).unwrap_or(false))
    .collect();
  items.sort_by(|a, b| {
    let all_day = b.is_all_day().cmp(&a.is_all_day());
    all_day.then_with(|| {
      let a_start = a.start().map(|s| s.timestamp()).unwrap_or(0);
      let b_start = b.start().map(|s| s.timestamp()).unwrap_or(0);
      a_start.cmp(&b_start)
    })
  });
  items
}

pub(super) fn hhmm(dt: DateTime<Utc>) -> String {
  format!("{:02}:{:02}", dt.hour(), dt.minute())
}

pub(super) fn is_instant(event: &CalendarEvent) -> bool {
  !event.is_all_day()
    && event
      .end()
      .zip(event.start())
      .map(|(end, start)| end == start)
      .unwrap_or(true)
}

pub(super) fn minutes_of(dt: DateTime<Utc>) -> i64 {
  i64::from(dt.hour()) * 60 + i64::from(dt.minute())
}

pub(super) fn month_matrix(year: i32, month0: u32, week_start: CalendarWeekStart) -> Vec<DateTime<Utc>> {
  let first = day_at(year, month0, 1);
  let grid_start = start_of_week(first, week_start);
  (0..42).map(|offset| grid_start + Duration::days(offset)).collect()
}

/// Assigns lane indices to timed events so overlapping events tile side by side.
///
/// Events are sorted by start time, grouped into clusters of mutually-overlapping spans, then each
/// cluster is packed greedily: each span takes the first lane whose previous occupant has ended.
pub(super) fn pack_day<'a>(items: &[&'a CalendarEvent]) -> Vec<Packed<'a>> {
  let mut spans: Vec<Packed<'a>> = items
    .iter()
    .filter(|event| !event.is_all_day())
    .filter_map(|event| {
      let start = event.start()?;
      let start_minute = minutes_of(start);
      let raw = event
        .end()
        .map(|end| (end - start).num_minutes())
        .unwrap_or(0)
        .max(MIN_BLOCK_MINUTES);
      Some(Packed {
        event,
        end_minute: start_minute + raw,
        lane: 0,
        lanes: 1,
        start_minute,
      })
    })
    .collect();
  spans.sort_by(|a, b| {
    a.start_minute
      .cmp(&b.start_minute)
      .then_with(|| a.end_minute.cmp(&b.end_minute))
  });

  let mut out: Vec<Packed<'a>> = Vec::with_capacity(spans.len());
  let mut cluster: Vec<Packed<'a>> = Vec::new();
  let mut cluster_end = i64::MIN;
  for span in spans {
    if !cluster.is_empty() && span.start_minute >= cluster_end {
      flush_cluster(&mut cluster, &mut out);
      cluster_end = i64::MIN;
    }
    cluster_end = cluster_end.max(span.end_minute);
    cluster.push(span);
  }
  flush_cluster(&mut cluster, &mut out);
  out
}

pub(super) fn start_of_day(dt: DateTime<Utc>) -> DateTime<Utc> {
  dt - Duration::seconds(i64::from(dt.num_seconds_from_midnight()))
}

pub(super) fn start_of_week(dt: DateTime<Utc>, week_start: CalendarWeekStart) -> DateTime<Utc> {
  let day = start_of_day(dt);
  let offset = match week_start {
    CalendarWeekStart::Monday => day.weekday().num_days_from_monday(),
    CalendarWeekStart::Sunday => day.weekday().num_days_from_sunday(),
  };
  day - Duration::days(i64::from(offset))
}

pub(super) fn timed_and_moments<'a>(packed: &[Packed<'a>]) -> (Vec<Packed<'a>>, Vec<Packed<'a>>) {
  packed.iter().copied().partition(|span| !is_instant(span.event))
}

pub(super) fn visible_weekdays(week_start: CalendarWeekStart, show_weekends: bool) -> Vec<u32> {
  let order: [u32; 7] = match week_start {
    CalendarWeekStart::Monday => [1, 2, 3, 4, 5, 6, 0],
    CalendarWeekStart::Sunday => [0, 1, 2, 3, 4, 5, 6],
  };
  order
    .into_iter()
    .filter(|weekday| show_weekends || (*weekday != 0 && *weekday != 6))
    .collect()
}

pub(super) fn week_dates(dt: DateTime<Utc>, week_start: CalendarWeekStart, show_weekends: bool) -> Vec<DateTime<Utc>> {
  let start = start_of_week(dt, week_start);
  (0..7)
    .map(|offset| start + Duration::days(offset))
    .filter(|day| show_weekends || !is_weekend(*day))
    .collect()
}

fn day_at(year: i32, month0: u32, day: u32) -> DateTime<Utc> {
  chrono::NaiveDate::from_ymd_opt(year, month0 + 1, day)
    .and_then(|date| date.and_hms_opt(0, 0, 0))
    .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
    .unwrap_or_else(Utc::now)
}

fn flush_cluster<'a>(cluster: &mut Vec<Packed<'a>>, out: &mut Vec<Packed<'a>>) {
  if cluster.is_empty() {
    return;
  }
  let mut lane_ends: Vec<i64> = Vec::new();
  for span in cluster.iter_mut() {
    let lane = lane_ends.iter().position(|end| span.start_minute >= *end);
    match lane {
      Some(index) => {
        lane_ends[index] = span.end_minute;
        span.lane = index;
      }
      None => {
        span.lane = lane_ends.len();
        lane_ends.push(span.end_minute);
      }
    }
  }
  let lanes = lane_ends.len().max(1);
  for mut span in cluster.drain(..) {
    span.lanes = lanes;
    out.push(span);
  }
}

fn is_weekend(day: DateTime<Utc>) -> bool {
  matches!(day.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn at(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp).unwrap().with_timezone(&Utc)
  }

  fn event(timestamp: &str, duration_minutes: i64) -> CalendarEvent {
    CalendarEvent {
      body: None,
      character_id: 1,
      duration_minutes,
      event_id: 1,
      importance: 0,
      owner_name: "Corp".to_owned(),
      owner_type: "corporation".to_owned(),
      response: "accepted".to_owned(),
      source: None,
      timestamp: timestamp.to_owned(),
      title: "Op".to_owned(),
    }
  }

  mod start_of_week {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rewinds_to_monday() {
      let monday = start_of_week(at("2026-06-12T14:00:00Z"), CalendarWeekStart::Monday);

      assert_eq!(monday, at("2026-06-08T00:00:00Z"));
    }

    #[test]
    fn it_rewinds_to_sunday() {
      let sunday = start_of_week(at("2026-06-12T14:00:00Z"), CalendarWeekStart::Sunday);

      assert_eq!(sunday, at("2026-06-07T00:00:00Z"));
    }
  }

  mod visible_weekdays {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_orders_a_monday_week() {
      assert_eq!(
        visible_weekdays(CalendarWeekStart::Monday, true),
        vec![1, 2, 3, 4, 5, 6, 0]
      );
    }

    #[test]
    fn it_drops_weekends_when_hidden() {
      assert_eq!(visible_weekdays(CalendarWeekStart::Sunday, false), vec![1, 2, 3, 4, 5]);
    }
  }

  mod week_dates {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_yields_seven_days_with_weekends() {
      let dates = week_dates(at("2026-06-12T00:00:00Z"), CalendarWeekStart::Monday, true);

      assert_eq!(dates.len(), 7);
      assert_eq!(dates[0], at("2026-06-08T00:00:00Z"));
    }

    #[test]
    fn it_drops_the_weekend_columns() {
      let dates = week_dates(at("2026-06-12T00:00:00Z"), CalendarWeekStart::Monday, false);

      assert_eq!(dates.len(), 5);
    }
  }

  mod month_matrix {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_spans_forty_two_days_from_the_week_start() {
      let matrix = month_matrix(2026, 5, CalendarWeekStart::Monday);

      assert_eq!(matrix.len(), 42);
      assert_eq!(matrix[0], at("2026-06-01T00:00:00Z"));
    }
  }

  mod pack_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lanes_overlapping_events_side_by_side() {
      let a = event("2026-06-12T19:00:00Z", 120);
      let b = event("2026-06-12T19:30:00Z", 60);
      let refs = vec![&a, &b];

      let packed = pack_day(&refs);

      assert_eq!(packed.len(), 2);
      assert!(packed.iter().all(|span| span.lanes == 2));
    }

    #[test]
    fn it_keeps_disjoint_events_in_one_lane() {
      let a = event("2026-06-12T09:00:00Z", 60);
      let b = event("2026-06-12T19:00:00Z", 60);
      let refs = vec![&a, &b];

      let packed = pack_day(&refs);

      assert!(packed.iter().all(|span| span.lanes == 1));
    }
  }

  mod events_on_day {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sorts_all_day_events_first() {
      let timed = event("2026-06-12T19:00:00Z", 60);
      let all_day = event("2026-06-12T00:00:00Z", 1440);
      let refs = vec![&timed, &all_day];

      let items = events_on_day(&refs, at("2026-06-12T00:00:00Z"));

      assert_eq!(items.len(), 2);
      assert!(items[0].is_all_day());
    }
  }
}
