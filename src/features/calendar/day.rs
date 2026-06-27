use chrono::{DateTime, Timelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, scrollable, text},
};

use super::{CalendarEvent, Message, State, grid, grid::Packed};
use crate::{
  config::CalendarDensity,
  ui::style::{color, radius, spacing, typography},
};

const GUTTER: f32 = 64.0;
const LANE_SPACING: f32 = 3.0;
const MOMENT_GLYPH: f32 = 20.0;

pub(super) fn view<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let tweaks = state.tweaks();
  let hour_height = hour_height(tweaks.density());
  let day = grid::start_of_day(state.cursor());
  let is_today = grid::day_key(day) == grid::day_key(now);
  let events = state.visible_events();
  let items = grid::events_on_day(&events, day);

  let all_day: Vec<&CalendarEvent> = items.iter().copied().filter(|event| event.is_all_day()).collect();
  let packed = grid::pack_day(&items);
  let (timed, moments) = grid::timed_and_moments(&packed);

  let mut layers: Vec<Element<'a, Message>> = vec![hour_grid(hour_height)];
  layers.push(event_canvas(state, hour_height, &timed, &moments));
  if is_today {
    layers.push(now_line(now, hour_height));
  }

  let canvas = Stack::with_children(layers)
    .width(Length::Fill)
    .height(Length::Fixed(hour_height * 24.0));

  let mut children: Vec<Element<'a, Message>> = Vec::new();
  if let Some(strip) = all_day_strip(state, &all_day) {
    children.push(strip);
  }
  children.push(scrollable(canvas).width(Length::Fill).height(Length::Fill).into());

  container(Column::with_children(children).width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn all_day_strip<'a>(state: &'a State, items: &[&'a CalendarEvent]) -> Option<Element<'a, Message>> {
  if items.is_empty() {
    return None;
  }

  let chips: Vec<Element<'a, Message>> = items.iter().map(|event| all_day_chip(state, event)).collect();

  let strip = Row::with_children(vec![
    container(
      text(t!("calendar.shell.all_day"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fixed(GUTTER))
    .into(),
    Row::with_children(chips).spacing(spacing::SPACE_2).wrap().into(),
  ])
  .spacing(spacing::SPACE_3)
  .padding(Padding {
    top: spacing::SPACE_3,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  });

  Some(
    container(strip)
      .width(Length::Fill)
      .style(|_| container::Style {
        border: Border {
          color: color::rule(),
          width: 1.0,
          radius: 0.0.into(),
        },
        ..container::Style::default()
      })
      .into(),
  )
}

fn all_day_chip<'a>(state: &'a State, event: &'a CalendarEvent) -> Element<'a, Message> {
  let tint = grid::color_for(state, event);
  let owner = event.owner_kind();

  button(
    Row::with_children(vec![
      owner.icon().color(tint).size(13.0).render::<Message>(),
      text(event.title.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::UNIT + 2.0,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::EventOpened(event.character_id, event.event_id))
  .style(move |_, status| block_style(tint, status))
  .into()
}

fn block_style(tint: iced::Color, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: Some(Background::Color(color::with_alpha(
      tint,
      if hovered { 0.26 } else { 0.16 },
    ))),
    border: Border {
      color: color::with_alpha(tint, 0.32),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn event_block<'a>(state: &'a State, packed: Packed<'a>, hour_height: f32) -> Element<'a, Message> {
  let event = packed.event;
  let tint = grid::color_for(state, event);
  let height = (((packed.end_minute - packed.start_minute) as f32 / 60.0) * hour_height - LANE_SPACING).max(28.0);
  let tall = height > 46.0;

  let mut lines: Vec<Element<'a, Message>> = vec![
    text(event.title.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if tall && let (Some(start), Some(end)) = (event.start(), event.end()) {
    lines.push(
      text(format!("{}\u{2013}{}", grid::hhmm(start), grid::hhmm(end)))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  let inner = container(Column::with_children(lines).spacing(2.0))
    .width(Length::Fill)
    .padding(Padding {
      top: if tall { spacing::UNIT + 2.0 } else { 3.0 },
      bottom: if tall { spacing::UNIT + 2.0 } else { 3.0 },
      left: spacing::SPACE_2,
      right: spacing::SPACE_2,
    });

  button(
    Row::with_children(vec![grid::accent_strip(tint), inner.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fixed(height))
  .padding(0.0)
  .on_press(Message::EventOpened(event.character_id, event.event_id))
  .style(move |_, status| block_style(tint, status))
  .into()
}

fn event_canvas<'a>(
  state: &'a State,
  hour_height: f32,
  timed: &[Packed<'a>],
  moments: &[Packed<'a>],
) -> Element<'a, Message> {
  let lanes = timed.iter().map(|span| span.lanes).max().unwrap_or(1).max(1);

  let lane_columns: Vec<Element<'a, Message>> = (0..lanes)
    .map(|lane| lane_column(state, hour_height, timed, lane))
    .collect();

  let timed_layer = Row::with_children(lane_columns)
    .spacing(LANE_SPACING)
    .width(Length::Fill)
    .height(Length::Fixed(hour_height * 24.0));

  let mut layers: Vec<Element<'a, Message>> = vec![
    container(timed_layer)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: GUTTER + spacing::SPACE_2,
        right: spacing::SPACE_3,
      })
      .into(),
  ];
  if !moments.is_empty() {
    layers.push(moment_layer(state, hour_height, moments));
  }

  Stack::with_children(layers)
    .width(Length::Fill)
    .height(Length::Fixed(hour_height * 24.0))
    .into()
}

fn hour_grid<'a>(hour_height: f32) -> Element<'a, Message> {
  let gutter_cells: Vec<Element<'a, Message>> = (0..24)
    .map(|hour| {
      container(if hour == 0 {
        Element::from(Space::new())
      } else {
        text(format!("{hour:02}:00"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(color::text::tertiary()))
          .into()
      })
      .width(Length::Fill)
      .height(Length::Fixed(hour_height))
      .align_x(Horizontal::Right)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: 0.0,
        right: spacing::SPACE_2_5,
      })
      .into()
    })
    .collect();

  let gutter = container(Column::with_children(gutter_cells))
    .width(Length::Fixed(GUTTER))
    .height(Length::Fixed(hour_height * 24.0));

  let lines: Vec<Element<'a, Message>> = (0..24)
    .map(|hour| {
      container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(hour_height))
        .style(move |_| container::Style {
          border: Border {
            color: color::rule(),
            width: if hour == 0 { 0.0 } else { 1.0 },
            radius: 0.0.into(),
          },
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  let canvas = container(Column::with_children(lines))
    .width(Length::Fill)
    .height(Length::Fixed(hour_height * 24.0))
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  Row::with_children(vec![gutter.into(), canvas.into()])
    .width(Length::Fill)
    .height(Length::Fixed(hour_height * 24.0))
    .into()
}

fn hour_height(density: CalendarDensity) -> f32 {
  match density {
    CalendarDensity::Comfortable => 58.0,
    CalendarDensity::Compact => 46.0,
  }
}

/// Builds one vertical lane of timed events using leading `Space` spacers to simulate absolute
/// top-offset positioning, since iced 0.14 has no absolute-position layout primitive. `filled`
/// tracks the running vertical cursor so consecutive events in the same lane stay contiguous.
fn lane_column<'a>(state: &'a State, hour_height: f32, timed: &[Packed<'a>], lane: usize) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::new();
  let mut filled = 0.0_f32;

  let mut in_lane: Vec<&Packed<'a>> = timed.iter().filter(|span| span.lane == lane).collect();
  in_lane.sort_by_key(|span| span.start_minute);

  for span in in_lane {
    let top = (span.start_minute as f32 / 60.0) * hour_height;
    if top > filled {
      children.push(Space::new().height(Length::Fixed(top - filled)).into());
    }
    let block = event_block(state, *span, hour_height);
    let height = (((span.end_minute - span.start_minute) as f32 / 60.0) * hour_height - LANE_SPACING).max(28.0);
    children.push(block);
    filled = top + height + LANE_SPACING;
  }

  Column::with_children(children)
    .width(Length::FillPortion(1))
    .height(Length::Fixed(hour_height * 24.0))
    .into()
}

fn moment_layer<'a>(state: &'a State, hour_height: f32, moments: &[Packed<'a>]) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::new();
  let mut filled = 0.0_f32;

  for span in moments {
    let top = (span.start_minute as f32 / 60.0) * hour_height - MOMENT_GLYPH / 2.0;
    if top > filled {
      children.push(Space::new().height(Length::Fixed(top - filled)).into());
    }
    children.push(moment_marker(state, span.event));
    filled = top + MOMENT_GLYPH;
  }

  container(Column::with_children(children))
    .width(Length::Fill)
    .height(Length::Fixed(hour_height * 24.0))
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: GUTTER + spacing::SPACE_2,
      right: spacing::SPACE_3,
    })
    .into()
}

fn moment_marker<'a>(state: &'a State, event: &'a CalendarEvent) -> Element<'a, Message> {
  let tint = grid::color_for(state, event);
  let owner = event.owner_kind();

  let glyph = container(owner.icon().color(tint).size(12.0).render::<Message>())
    .width(Length::Fixed(MOMENT_GLYPH))
    .height(Length::Fixed(MOMENT_GLYPH))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tint, 0.18))),
      border: Border {
        color: color::with_alpha(tint, 0.4),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    });

  let time = event.start().map(grid::hhmm).unwrap_or_default();

  button(
    Row::with_children(vec![
      glyph.into(),
      text(event.title.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Space::new().width(Length::Fill).into(),
      text(time)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(0.0)
  .width(Length::Fill)
  .on_press(Message::EventOpened(event.character_id, event.event_id))
  .style(|_, _| button::Style::default())
  .into()
}

fn now_line<'a>(now: DateTime<Utc>, hour_height: f32) -> Element<'a, Message> {
  let minutes = i64::from(now.hour()) * 60 + i64::from(now.minute());
  let top = (minutes as f32 / 60.0) * hour_height;

  let line = container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(2.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      ..container::Style::default()
    });

  Column::with_children(vec![
    Space::new().height(Length::Fixed(top.max(0.0))).into(),
    container(line)
      .width(Length::Fill)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: GUTTER,
        right: 0.0,
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod hour_height {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_compresses_in_compact_density() {
      assert!(hour_height(CalendarDensity::Compact) < hour_height(CalendarDensity::Comfortable));
    }

    #[test]
    fn it_uses_the_comfortable_default() {
      assert_eq!(hour_height(CalendarDensity::Comfortable), 58.0);
    }
  }
}
