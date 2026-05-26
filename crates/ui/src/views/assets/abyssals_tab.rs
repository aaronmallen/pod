//! Abyssals tab — mutated module grid with stat rows and resizable filter sidebar.

pub mod tier_badge;
pub mod type_icon_tile;
use std::collections::HashMap;

use iced::{
  Background, Border, Color, Element, Length, Padding, Point, Size, Theme, mouse,
  widget::{
    Canvas, Space, button,
    canvas::{self, Action, Frame, Geometry, Path, Stroke},
    column, container, image, mouse_area, row, scrollable, stack, text, text_input,
  },
};
use pod_model::{AbyssalCategory, AbyssalStatViewModel, AbyssalViewModel};

use super::State;
use crate::{
  components::{
    avatar::{self, AvatarKind},
    icon,
  },
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

const UNIT_SUFFIX_TABLE: &[(i32, &str)] = &[
  (71, " GJ"),
  (101, " m/s"),
  (105, " HP"),
  (108, " s"),
  (114, " kg"),
  (115, " tf"),
  (116, " MW"),
  (117, " km"),
  (121, " m\u{00b3}"),
  (124, "%"),
];

fn unit_suffix_for_id(unit_id: Option<i32>) -> &'static str {
  unit_id
    .and_then(|id| UNIT_SUFFIX_TABLE.iter().find(|&&(k, _)| k == id).map(|&(_, v)| v))
    .unwrap_or("")
}

pub fn dogma_unit_suffix(unit_id: Option<i32>) -> String {
  unit_suffix_for_id(unit_id).to_string()
}

/// Which endpoint of a stat slider range is being edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderEndpoint {
  Max,
  Min,
}

/// Messages produced by the abyssals tab.
#[derive(Clone, Debug)]
pub enum Message {
  /// The module-type picker modal was closed without a selection change.
  CloseTypeModal,
  /// The active filter was cleared.
  FilterReset,
  /// The module-type picker modal was opened.
  OpenTypeModal,
  /// Cursor moved during filter pane drag (cursor x position).
  PaneDrag(f32),
  /// Filter pane drag released.
  PaneDragEnd,
  /// Filter pane drag handle pressed.
  PaneDragStart,
  /// A slider value text field was committed (attribute_id, endpoint).
  SliderEditCommit(i32, SliderEndpoint),
  /// A slider value text field content changed (new text).
  SliderEditInput(String),
  /// A slider value label was clicked to begin editing (attribute_id, endpoint, current_value).
  SliderEditStart(i32, SliderEndpoint, f64),
  /// A per-stat maximum filter value changed (attribute_id, new_max).
  StatMaxFilterChanged(i32, f64),
  /// A per-stat minimum filter value changed (attribute_id, new_min).
  StatMinFilterChanged(i32, f64),
  /// A module source type was selected in the picker (None = all types).
  TypeSelected(Option<i32>),
}

fn stat_roll_direction(stat: &AbyssalStatViewModel) -> Option<bool> {
  let delta = stat.rolled_value - stat.base_value;
  if delta.abs() < 1e-9 {
    None
  } else if stat.high_is_good {
    Some(delta > 0.0)
  } else {
    Some(delta < 0.0)
  }
}

fn stat_direction_color(dir: Option<bool>) -> Color {
  match dir {
    Some(true) => color::text::SUCCESS,
    Some(false) => color::text::DANGER,
    None => color::text::TERTIARY,
  }
}

fn stat_delta_intensity(stat: &AbyssalStatViewModel, delta: f64) -> f32 {
  let range_span = (stat.max_mult - 1.0).abs().max(1e-9);
  let delta_pct = if stat.base_value.abs() > 1e-9 {
    (delta / stat.base_value).abs()
  } else {
    0.0
  };
  (delta_pct / range_span).clamp(0.0, 1.0) as f32
}

fn stat_intensity_bar(intensity: f32, fill_col: Color) -> Element<'static, Message> {
  let bg_col = color::border::SUBTLE;
  container(
    container(Space::new().width(Length::Fixed(intensity * 110.0)).height(4.0)).style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(fill_col, 0.9))),
      ..container::Style::default()
    }),
  )
  .width(110.0)
  .height(4.0)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_col)),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .clip(true)
  .into()
}

fn format_stat_value(value: f64, unit_suffix: &str) -> String {
  let formatted = format!("{:.2}", value);
  let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
  format!("{}{unit_suffix}", trimmed)
}

fn format_delta_line(delta: f64, base: f64, unit_suffix: &str) -> String {
  let sign = if delta >= 0.0 { "+" } else { "" };
  let abs_str = format_stat_value(delta.abs(), unit_suffix);
  let pct = if base.abs() > 1e-9 { delta / base * 100.0 } else { 0.0 };
  let pct_sign = if pct >= 0.0 { "+" } else { "" };
  format!("{}{} \u{00b7} {}{:.1}%", sign, abs_str, pct_sign, pct)
}

fn stat_row(stat: &AbyssalStatViewModel) -> Element<'_, Message> {
  let delta = stat.rolled_value - stat.base_value;
  let stat_color = stat_direction_color(stat_roll_direction(stat));
  let intensity = stat_delta_intensity(stat, delta);
  let border_color = if delta.abs() < 1e-9 {
    Color::TRANSPARENT
  } else {
    stat_color
  };

  let name_el = text(stat.display_name.clone())
    .font(body::REGULAR)
    .size(11.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let delta_line = format_delta_line(delta, stat.base_value, &stat.unit_suffix);
  let delta_el = text(delta_line)
    .font(mono::REGULAR)
    .size(10.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(stat_color),
    });

  let left_col = column([name_el.into(), delta_el.into()]).width(Length::Fill);

  let value_num = format_stat_value(stat.rolled_value, "");
  let unit_str = stat.unit_suffix.trim().to_string();
  let value_el = row([
    text(value_num)
      .font(mono::MEDIUM)
      .size(16.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(unit_str)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .align_y(iced::alignment::Vertical::Bottom);

  let bar_el = container(stat_intensity_bar(intensity, stat_color)).width(110.0);

  let content = row([
    left_col.into(),
    Space::new().width(14.0).into(),
    value_el.into(),
    Space::new().width(14.0).into(),
    bar_el.into(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .width(Length::Fill);

  container(row([
    container(Space::new())
      .width(2.0)
      .height(Length::Fill)
      .style(move |_| container::Style {
        background: Some(Background::Color(border_color)),
        ..container::Style::default()
      })
      .into(),
    Space::new().width(8.0).into(),
    content.into(),
  ]))
  .width(Length::Fill)
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 0.0,
    right: 0.0,
  })
  .into()
}

fn abyssal_card_header<'a>(
  item: &'a AbyssalViewModel,
  type_icons: &HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let price_label = item
    .muta_price_isk
    .map(format::fmt_isk)
    .unwrap_or_else(|| "\u{2014}".to_string());
  container(
    row([
      type_icon_tile::Component::new(&item.base_type_name, item.source_type_id, 42.0, 42.0)
        .icon(type_icons.get(&item.source_type_id).cloned())
        .render(),
      Space::new().width(12.0).into(),
      column([
        row([
          text(item.base_type_name.clone())
            .font(body::MEDIUM)
            .size(13.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::PRIMARY),
            })
            .into(),
          tier_badge::Component::new(&item.mutaplasmid_tier).render(),
          Space::new().width(Length::Fill).into(),
        ])
        .align_y(iced::alignment::Vertical::Center)
        .spacing(8.0)
        .into(),
        Space::new().height(2.0).into(),
        text(format!("{} Mutaplasmid", item.mutaplasmid_tier))
          .font(body::REGULAR)
          .size(11.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .width(Length::Fill)
      .into(),
      column([text(price_label)
        .font(mono::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::ACCENT),
        })
        .into()])
      .align_x(iced::alignment::Horizontal::Right)
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 12.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill)
  .into()
}

fn abyssal_card_stats(item: &AbyssalViewModel) -> Element<'_, Message> {
  let stat_rows: Vec<Element<'_, Message>> = item.stats.iter().map(stat_row).collect();
  container(column(stat_rows).spacing(2.0))
    .padding(Padding {
      top: 6.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .into()
}

fn abyssal_card_footer<'a>(
  item: &'a AbyssalViewModel,
  char_name: &'a str,
  portrait: Option<image::Handle>,
) -> Element<'a, Message> {
  let avatar = avatar::Component::new(
    char_name,
    (item.character_id.unsigned_abs() % 360) as u16,
    18.0,
    AvatarKind::Person,
  )
  .portrait(portrait)
  .render::<Message>();
  let mut row_items: Vec<Element<'_, Message>> = vec![
    avatar,
    Space::new().width(8.0).into(),
    text(char_name.to_string())
      .font(body::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ];
  if !item.location.is_empty() {
    row_items.push(Space::new().width(8.0).into());
    row_items.push(
      text("\u{00b7}")
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    );
    row_items.push(Space::new().width(8.0).into());
    row_items.push(
      text(item.location.clone())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(Length::Fill)
        .into(),
    );
  } else {
    row_items.push(Space::new().width(Length::Fill).into());
  }
  container(row(row_items).align_y(iced::alignment::Vertical::Center))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn abyssal_card<'a>(
  item: &'a AbyssalViewModel,
  char_name: &'a str,
  type_icons: &HashMap<i32, image::Handle>,
  portrait: Option<image::Handle>,
) -> Element<'a, Message> {
  let header = abyssal_card_header(item, type_icons);
  let stats_area = abyssal_card_stats(item);
  let footer = abyssal_card_footer(item, char_name, portrait);
  container(column([header, stats_area, footer]))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn item_passes_filter(
  item: &AbyssalViewModel,
  selected_source_type_id: Option<i32>,
  stat_range_filters: &HashMap<i32, (f64, f64)>,
) -> bool {
  if selected_source_type_id.is_some_and(|id| item.type_id != id) {
    return false;
  }
  for (attr_id, (min_val, max_val)) in stat_range_filters {
    if let Some(stat) = item.stats.iter().find(|s| s.attribute_id == *attr_id) {
      if stat.rolled_value < *min_val || stat.rolled_value > *max_val {
        return false;
      }
    }
  }
  true
}

fn stat_roll_contribution(s: &AbyssalStatViewModel) -> Option<f64> {
  let delta = s.rolled_value - s.base_value;
  if delta.abs() < 1e-9 {
    return None;
  }
  let pct = if s.base_value.abs() > 1e-9 {
    delta / s.base_value * 100.0
  } else {
    0.0
  };
  Some(if s.high_is_good { pct } else { -pct })
}

pub fn roll_score(item: &AbyssalViewModel) -> f64 {
  if item.stats.is_empty() {
    return 0.0;
  }
  let sum: f64 = item.stats.iter().filter_map(stat_roll_contribution).sum();
  sum / item.stats.len() as f64
}

fn empty_grid_message(msg: &str) -> Element<'static, Message> {
  container(
    text(msg.to_string())
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .center(Length::Fill)
  .into()
}

fn card_grid<'a>(state: &'a State) -> Element<'a, Message> {
  if state.abyssals.abyssals.is_empty() {
    return empty_grid_message("No abyssal modules synced yet.\nSync your characters to load abyssal data.");
  }

  let char_name_map: HashMap<i64, &str> = state.characters.iter().map(|c| (*c.id(), c.name().as_str())).collect();
  let type_icons = &state.abyssals.type_icons;

  let items: Vec<&AbyssalViewModel> = state
    .abyssals
    .abyssals
    .iter()
    .filter(|item| {
      item_passes_filter(
        item,
        state.abyssals.selected_source_type_id,
        &state.abyssals.stat_range_filters,
      )
    })
    .collect();

  if items.is_empty() {
    return empty_grid_message("No abyssal modules match the current filters.");
  }

  let cards: Vec<Element<'_, Message>> = items
    .iter()
    .map(|item| {
      let char_name = char_name_map.get(&item.character_id).copied().unwrap_or("");
      let portrait = state.abyssals.portrait_handles.get(&item.character_id).cloned();
      container(abyssal_card(item, char_name, type_icons, portrait))
        .padding(Padding {
          top: 0.0,
          bottom: 16.0,
          left: 0.0,
          right: 0.0,
        })
        .max_width(500.0)
        .width(Length::Fill)
        .into()
    })
    .collect();

  scrollable(
    container(column(cards).spacing(0.0))
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
  )
  .height(Length::Fill)
  .into()
}

fn section_divider() -> Element<'static, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn filter_section<'a>(label: &str, content: Element<'a, Message>) -> Element<'a, Message> {
  let label_el = text(label.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });

  column([
    container(column([label_el.into(), Space::new().height(10.0).into(), content]))
      .padding(Padding {
        top: 14.0,
        bottom: 14.0,
        left: 16.0,
        right: 16.0,
      })
      .width(Length::Fill)
      .into(),
    section_divider(),
  ])
  .width(Length::Fill)
  .into()
}

fn sidebar_reset_button(visible: bool) -> Element<'static, Message> {
  if !visible {
    return Space::new().width(0.0).into();
  }
  button(
    text("Reset")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 4.0,
    right: 4.0,
  })
  .on_press(Message::FilterReset)
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::text::TERTIARY,
    ..button::Style::default()
  })
  .into()
}

#[derive(Clone, Copy, PartialEq)]
enum DragTarget {
  Max,
  Min,
}

#[derive(Clone, Default)]
struct RangeSliderState {
  dragging: Option<DragTarget>,
}

struct RangeSliderProgram {
  attribute_id: i32,
  current_max: f64,
  current_min: f64,
  hi: f64,
  lo: f64,
}

impl canvas::Program<Message> for RangeSliderProgram {
  type State = RangeSliderState;

  fn update(
    &self,
    state: &mut Self::State,
    event: &canvas::Event,
    bounds: iced::Rectangle,
    cursor: mouse::Cursor,
  ) -> Option<Action<Message>> {
    let range = (self.hi - self.lo).max(1e-9);
    let thumb_r = 7.0f32;
    let inner_w = (bounds.width - thumb_r * 2.0).max(1.0);
    let val_at_x = |x: f32| -> f64 {
      let frac = ((x - thumb_r) / inner_w).clamp(0.0, 1.0) as f64;
      self.lo + frac * range
    };

    match event {
      canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
        let padded = iced::Rectangle {
          x: bounds.x - thumb_r,
          y: bounds.y,
          width: bounds.width + thumb_r * 2.0,
          height: bounds.height,
        };
        let Some(pos) = cursor.position_in(padded) else {
          return None;
        };
        let canvas_x = pos.x - thumb_r;
        let thumb_lo = thumb_r;
        let thumb_hi = thumb_r + inner_w;
        let x_min = (thumb_r + ((self.current_min - self.lo) / range) as f32 * inner_w).clamp(thumb_lo, thumb_hi);
        let x_max = (thumb_r + ((self.current_max - self.lo) / range) as f32 * inner_w).clamp(thumb_lo, thumb_hi);
        let target = if (canvas_x - x_min).abs() <= (canvas_x - x_max).abs() {
          DragTarget::Min
        } else {
          DragTarget::Max
        };
        state.dragging = Some(target);
        Some(Action::request_redraw())
      }
      canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
        if state.dragging.is_some() {
          state.dragging = None;
          return Some(Action::request_redraw());
        }
        None
      }
      canvas::Event::Mouse(mouse::Event::CursorMoved {
        ..
      }) => {
        let Some(target) = state.dragging else { return None };
        let raw = cursor.position().unwrap_or_default();
        let canvas_x = raw.x - bounds.x;
        let new_val = val_at_x(canvas_x);
        let attr_id = self.attribute_id;
        let message = match target {
          DragTarget::Min => Message::StatMinFilterChanged(attr_id, new_val.clamp(self.lo, self.current_max)),
          DragTarget::Max => Message::StatMaxFilterChanged(attr_id, new_val.clamp(self.current_min, self.hi)),
        };
        Some(Action::publish(message))
      }
      _ => None,
    }
  }

  fn draw(
    &self,
    state: &Self::State,
    renderer: &iced::Renderer,
    _theme: &iced::Theme,
    bounds: iced::Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<Geometry<iced::Renderer>> {
    let mut frame = Frame::new(renderer, bounds.size());
    let w = frame.width();
    let h = frame.height();
    let cy = h / 2.0;
    let rail_h = 3.0f32;
    let thumb_r = 7.0f32;
    let range = (self.hi - self.lo).max(1e-9);
    let inner_w = (w - thumb_r * 2.0).max(0.0);
    let x_for = |v: f64| thumb_r + ((v - self.lo) / range).clamp(0.0, 1.0) as f32 * inner_w;
    let x_min = x_for(self.current_min);
    let x_max = x_for(self.current_max);

    let is_filtered = (self.current_min - self.lo).abs() > 1e-9 || (self.current_max - self.hi).abs() > 1e-9;

    let bg_rail = Path::rectangle(
      Point::new(thumb_r, cy - rail_h / 2.0),
      Size::new((w - thumb_r * 2.0).max(0.0), rail_h),
    );
    frame.fill(&bg_rail, color::border::SUBTLE);

    let segment_col = if is_filtered {
      color::text::ACCENT
    } else {
      color::with_alpha(color::text::PRIMARY, 0.22)
    };
    let active_w = (x_max - x_min).max(0.0);
    if active_w > 0.0 {
      let active_rail = Path::rectangle(Point::new(x_min, cy - rail_h / 2.0), Size::new(active_w, rail_h));
      frame.fill(&active_rail, segment_col);
    }

    for (x, dragging) in [
      (x_min, state.dragging == Some(DragTarget::Min)),
      (x_max, state.dragging == Some(DragTarget::Max)),
    ] {
      let r = if dragging { 8.0f32 } else { thumb_r };
      if dragging {
        let glow = Path::circle(Point::new(x, cy), r + 4.0);
        frame.fill(&glow, color::with_alpha(color::text::ACCENT, 0.22));
      }
      let thumb = Path::circle(Point::new(x, cy), r);
      frame.fill(&thumb, color::surface::BASE);
      let border_col = if dragging {
        color::text::ACCENT
      } else {
        color::text::PRIMARY
      };
      frame.stroke(&thumb, Stroke::default().with_color(border_col).with_width(2.0));
    }

    vec![frame.into_geometry()]
  }

  fn mouse_interaction(
    &self,
    state: &Self::State,
    bounds: iced::Rectangle,
    cursor: mouse::Cursor,
  ) -> mouse::Interaction {
    if state.dragging.is_some() || cursor.position_in(bounds).is_some() {
      mouse::Interaction::ResizingHorizontally
    } else {
      mouse::Interaction::default()
    }
  }
}

fn slider_value_label<'a>(
  attr_id: i32,
  endpoint: SliderEndpoint,
  value: f64,
  unit: &str,
  editing: Option<(&(i32, SliderEndpoint), &str)>,
) -> Element<'a, Message> {
  let is_editing = editing.is_some_and(|(k, _)| k.0 == attr_id && k.1 == endpoint);
  if is_editing {
    let current_text = editing.map(|(_, t)| t).unwrap_or("").to_string();
    return text_input("", &current_text)
      .on_input(Message::SliderEditInput)
      .on_submit(Message::SliderEditCommit(attr_id, endpoint))
      .font(mono::REGULAR)
      .size(10.0)
      .width(56.0)
      .style(|_, _| text_input::Style {
        background: Background::Color(color::with_alpha(color::text::ACCENT, 0.08)),
        border: Border {
          color: color::text::ACCENT,
          radius: 3.0.into(),
          width: 1.0,
        },
        icon: color::text::ACCENT,
        placeholder: color::text::TERTIARY,
        value: color::text::ACCENT,
        selection: color::with_alpha(color::text::ACCENT, 0.25),
      })
      .into();
  }
  let label = format!("{}{}", format_stat_value(value, ""), unit.trim());
  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::ACCENT),
      }),
  )
  .padding(Padding::ZERO)
  .on_press(Message::SliderEditStart(attr_id, endpoint, value))
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::text::ACCENT,
    ..button::Style::default()
  })
  .into()
}

fn stat_slider_row<'a>(
  stat: &AbyssalStatViewModel,
  filter_range: Option<(f64, f64)>,
  bounds: (f64, f64),
  editing: Option<(&'a (i32, SliderEndpoint), &'a str)>,
) -> Element<'a, Message> {
  let (lo, hi) = bounds;
  let (filter_min, filter_max) = filter_range
    .map(|(min, max)| (min.max(lo), max.min(hi)))
    .unwrap_or((lo, hi));
  let is_active = filter_range.is_some();
  let unit = stat.unit_suffix.clone();

  let readout_color = if is_active {
    color::text::ACCENT
  } else {
    color::text::TERTIARY
  };
  let sep_col = readout_color;

  let min_el = slider_value_label(stat.attribute_id, SliderEndpoint::Min, filter_min, &unit, editing);
  let max_el = slider_value_label(stat.attribute_id, SliderEndpoint::Max, filter_max, &unit, editing);

  let readout_row: Element<'_, Message> = row([
    min_el,
    text(" \u{2013} ")
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(sep_col),
      })
      .into(),
    max_el,
    text(format!(" {}", unit.trim()))
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(readout_color),
      })
      .into(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();

  let label_row: Element<'_, Message> = row([
    text(stat.display_name.clone())
      .font(body::MEDIUM)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    readout_row,
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();

  let track = Canvas::new(RangeSliderProgram {
    attribute_id: stat.attribute_id,
    current_max: filter_max,
    current_min: filter_min,
    hi,
    lo,
  })
  .width(Length::Fill)
  .height(22.0);

  let bounds_row: Element<'_, Message> = row([
    text(format_stat_value(lo, &unit))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    text(format!(
      "base {}{}",
      format_stat_value(stat.base_value, if unit.trim() == "%" { &unit } else { "" }),
      if unit.trim() == "%" { "" } else { unit.trim() }
    ))
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into(),
    Space::new().width(Length::Fill).into(),
    text(format_stat_value(hi, &unit))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .into();

  column([
    label_row,
    Space::new().height(6.0).into(),
    track.into(),
    Space::new().height(3.0).into(),
    bounds_row,
  ])
  .spacing(0.0)
  .into()
}

fn module_type_filter_button(has_filter: bool) -> Element<'static, Message> {
  let (bg, border_col, label_col) = if has_filter {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.08))),
      color::with_alpha(color::text::ACCENT, 0.35),
      color::text::PRIMARY,
    )
  } else {
    (None, color::border::SUBTLE, color::text::SECONDARY)
  };
  let icon_col = if has_filter {
    color::text::ACCENT
  } else {
    color::text::TERTIARY
  };
  let label_text = if has_filter {
    "Edit module filter"
  } else {
    "Filter by module type"
  };
  let mut row_items: Vec<Element<'static, Message>> = vec![
    icon::Component::filter().size(14.0).color(icon_col).render::<Message>(),
    Space::new().width(10.0).into(),
    text(label_text)
      .font(body::REGULAR)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(label_col),
      })
      .width(Length::Fill)
      .into(),
  ];
  if has_filter {
    row_items.push(
      container(
        text("1")
          .font(mono::MEDIUM)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::surface::BASE),
          }),
      )
      .padding(Padding {
        top: 2.0,
        bottom: 2.0,
        left: 7.0,
        right: 7.0,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::text::ACCENT)),
        border: Border {
          radius: 999.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    );
  }
  button(row(row_items).align_y(iced::alignment::Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 12.0,
      right: 12.0,
    })
    .on_press(Message::OpenTypeModal)
    .style(move |_, _| button::Style {
      background: bg,
      border: Border {
        color: border_col,
        radius: 6.0.into(),
        width: 1.0,
      },
      text_color: label_col,
      ..button::Style::default()
    })
    .into()
}

fn selected_type_chip(type_name: &str) -> Element<'static, Message> {
  let name_owned = type_name.to_string();
  container(
    row([
      text(name_owned)
        .font(body::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      Space::new().width(4.0).into(),
      button(
        text("\u{00d7}")
          .font(mono::REGULAR)
          .size(12.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .padding(Padding {
        top: 1.0,
        bottom: 1.0,
        left: 2.0,
        right: 2.0,
      })
      .on_press(Message::TypeSelected(None))
      .style(|_, _| button::Style {
        background: None,
        border: Border::default(),
        text_color: color::text::SECONDARY,
        ..button::Style::default()
      })
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 4.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

struct ModalEntry {
  label: &'static str,
  type_id: i32,
}

enum ModalRow {
  Family {
    name: &'static str,
    variants: &'static [ModalEntry],
  },
  Single {
    label: &'static str,
    type_id: i32,
  },
}

struct ModalSection {
  rows: &'static [ModalRow],
  title: &'static str,
}

static MODAL_LAYOUT: &[&[ModalSection]] = &[
  &[
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Stasis Webifier",
          type_id: 47702,
        },
        ModalRow::Single {
          label: "Warp Scrambler",
          type_id: 47732,
        },
        ModalRow::Single {
          label: "Warp Disruptor",
          type_id: 47736,
        },
        ModalRow::Single {
          label: "Heavy Warp Scrambler",
          type_id: 56303,
        },
        ModalRow::Single {
          label: "Heavy Warp Disruptor",
          type_id: 56304,
        },
      ],
      title: "Electronic Warfare",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Magnetic Field Stabilizer",
          type_id: 49722,
        },
        ModalRow::Single {
          label: "Heat Sink",
          type_id: 49726,
        },
        ModalRow::Single {
          label: "Gyrostabilizer",
          type_id: 49730,
        },
        ModalRow::Single {
          label: "Entropic Radiation Sink",
          type_id: 49734,
        },
        ModalRow::Single {
          label: "Ballistic Control System",
          type_id: 49738,
        },
        ModalRow::Single {
          label: "Drone Damage Amplifier",
          type_id: 60482,
        },
        ModalRow::Single {
          label: "Siege Module",
          type_id: 56313,
        },
        ModalRow::Single {
          label: "Vorton Tuning System",
          type_id: 78621,
        },
        ModalRow::Single {
          label: "Fighter Support Unit",
          type_id: 60483,
        },
      ],
      title: "Weapon Upgrades",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Mining Laser",
          type_id: 90460,
        },
        ModalRow::Single {
          label: "Deep Core Mining Laser",
          type_id: 90483,
        },
        ModalRow::Single {
          label: "Modulated Deep Core Miner",
          type_id: 90474,
        },
      ],
      title: "Mining Lasers",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Strip Miner",
          type_id: 90493,
        },
        ModalRow::Single {
          label: "Deep Core Strip Miner",
          type_id: 90498,
        },
        ModalRow::Single {
          label: "Modulated Strip Miner",
          type_id: 90467,
        },
        ModalRow::Single {
          label: "Modulated Deep Core Strip Miner",
          type_id: 90487,
        },
      ],
      title: "Strip Miners",
    },
  ],
  &[
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Shield Booster",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47781,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47785,
            },
            ModalEntry {
              label: "Large",
              type_id: 47789,
            },
            ModalEntry {
              label: "X-Large",
              type_id: 47793,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56309,
            },
          ],
        },
        ModalRow::Family {
          name: "Ancillary Shield Booster",
          variants: &[
            ModalEntry {
              label: "Medium",
              type_id: 47836,
            },
            ModalEntry {
              label: "Large",
              type_id: 47838,
            },
            ModalEntry {
              label: "X-Large",
              type_id: 47840,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56310,
            },
          ],
        },
        ModalRow::Family {
          name: "Shield Extender",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47800,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47804,
            },
            ModalEntry {
              label: "Large",
              type_id: 47808,
            },
          ],
        },
      ],
      title: "Shield",
    },
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Armor Repairer",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47769,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47773,
            },
            ModalEntry {
              label: "Large",
              type_id: 47777,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56307,
            },
          ],
        },
        ModalRow::Family {
          name: "Ancillary Armor Repairer",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47842,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47844,
            },
            ModalEntry {
              label: "Large",
              type_id: 47846,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56308,
            },
          ],
        },
        ModalRow::Family {
          name: "Armor Plates",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47812,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47817,
            },
            ModalEntry {
              label: "Large",
              type_id: 47820,
            },
          ],
        },
      ],
      title: "Armor",
    },
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Afterburner",
          variants: &[
            ModalEntry {
              label: "1MN",
              type_id: 47749,
            },
            ModalEntry {
              label: "10MN",
              type_id: 47753,
            },
            ModalEntry {
              label: "100MN",
              type_id: 47757,
            },
            ModalEntry {
              label: "10000MN",
              type_id: 56305,
            },
          ],
        },
        ModalRow::Family {
          name: "Microwarpdrive",
          variants: &[
            ModalEntry {
              label: "5MN",
              type_id: 47740,
            },
            ModalEntry {
              label: "50MN",
              type_id: 47408,
            },
            ModalEntry {
              label: "500MN",
              type_id: 47745,
            },
            ModalEntry {
              label: "50000MN",
              type_id: 56306,
            },
          ],
        },
      ],
      title: "Propulsion",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Ice Mining Laser",
          type_id: 90502,
        },
        ModalRow::Single {
          label: "Ice Harvester",
          type_id: 90524,
        },
      ],
      title: "Ice Mining",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Gas Cloud Scoop",
          type_id: 90529,
        },
        ModalRow::Single {
          label: "Gas Cloud Harvester",
          type_id: 90593,
        },
      ],
      title: "Gas Harvesting",
    },
  ],
  &[
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Energy Neutralizer",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47824,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47828,
            },
            ModalEntry {
              label: "Heavy",
              type_id: 47832,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56312,
            },
          ],
        },
        ModalRow::Family {
          name: "Energy Nosferatu",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 48419,
            },
            ModalEntry {
              label: "Medium",
              type_id: 48423,
            },
            ModalEntry {
              label: "Heavy",
              type_id: 48427,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56311,
            },
          ],
        },
        ModalRow::Family {
          name: "Cap Battery",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 48431,
            },
            ModalEntry {
              label: "Medium",
              type_id: 48435,
            },
            ModalEntry {
              label: "Large",
              type_id: 48439,
            },
          ],
        },
      ],
      title: "Engineering",
    },
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Damage Control",
          variants: &[
            ModalEntry {
              label: "Regular",
              type_id: 52227,
            },
            ModalEntry {
              label: "Assault",
              type_id: 52230,
            },
          ],
        },
        ModalRow::Family {
          name: "EMP Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84442,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84438,
            },
            ModalEntry {
              label: "Large",
              type_id: 84434,
            },
          ],
        },
        ModalRow::Family {
          name: "Graviton Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84444,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84440,
            },
            ModalEntry {
              label: "Large",
              type_id: 84436,
            },
          ],
        },
        ModalRow::Family {
          name: "Plasma Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84443,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84439,
            },
            ModalEntry {
              label: "Large",
              type_id: 84435,
            },
          ],
        },
        ModalRow::Family {
          name: "Proton Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84445,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84441,
            },
            ModalEntry {
              label: "Large",
              type_id: 84437,
            },
          ],
        },
        ModalRow::Family {
          name: "Combat Drone",
          variants: &[
            ModalEntry {
              label: "Light",
              type_id: 60478,
            },
            ModalEntry {
              label: "Medium",
              type_id: 60479,
            },
            ModalEntry {
              label: "Heavy",
              type_id: 60480,
            },
            ModalEntry {
              label: "Sentry",
              type_id: 60481,
            },
          ],
        },
      ],
      title: "Miscellaneous",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Mining Drone",
          type_id: 90614,
        },
        ModalRow::Single {
          label: "Ice Harvesting Drone",
          type_id: 90618,
        },
        ModalRow::Single {
          label: "'Excavator' Mining Drone",
          type_id: 90621,
        },
        ModalRow::Single {
          label: "'Excavator' Ice Harvesting Drone",
          type_id: 90622,
        },
      ],
      title: "Mining Drones",
    },
  ],
];

fn modal_selected_label(type_id: i32) -> Option<String> {
  for col in MODAL_LAYOUT {
    for section in *col {
      for row in section.rows {
        match row {
          ModalRow::Single {
            label,
            type_id: tid,
          } if *tid == type_id => {
            return Some((*label).to_string());
          }
          ModalRow::Family {
            name,
            variants,
            ..
          } => {
            for e in *variants {
              if e.type_id == type_id {
                return Some(format!("{} ({})", name, e.label));
              }
            }
          }
          _ => {}
        }
      }
    }
  }
  None
}

static MODAL_SOURCE_PATTERNS: &[(i32, &str)] = &[
  (47702, "Stasis Webifier"),
  (47732, "Warp Scrambler"),
  (47736, "Warp Disruptor"),
  (56303, "Heavy Warp Scrambler"),
  (56304, "Heavy Warp Disruptor"),
  (49722, "Magnetic Field Stabilizer"),
  (49726, "Heat Sink"),
  (49730, "Gyrostabilizer"),
  (49734, "Entropic Radiation Sink"),
  (49738, "Ballistic Control System"),
  (60482, "Drone Damage Amplifier"),
  (56313, "Siege Module"),
  (78621, "Vorton Tuning System"),
  (60483, "Fighter Support Unit"),
  (90460, "Mining Laser"),
  (90483, "Deep Core Mining Laser"),
  (90474, "Modulated Deep Core Miner"),
  (90493, "Strip Miner"),
  (90498, "Deep Core Strip Miner"),
  (90467, "Modulated Strip Miner"),
  (90487, "Modulated Deep Core Strip Miner"),
  (47781, "Small Shield Booster"),
  (47785, "Medium Shield Booster"),
  (47789, "Large Shield Booster"),
  (47793, "X-Large Shield Booster"),
  (56309, "Capital Shield Booster"),
  (47836, "Medium Ancillary Shield Booster"),
  (47838, "Large Ancillary Shield Booster"),
  (47840, "X-Large Ancillary Shield Booster"),
  (56310, "Capital Ancillary Shield Booster"),
  (47800, "Small Shield Extender"),
  (47804, "Medium Shield Extender"),
  (47808, "Large Shield Extender"),
  (47769, "Small Armor Repairer"),
  (47773, "Medium Armor Repairer"),
  (47777, "Large Armor Repairer"),
  (56307, "Capital Armor Repairer"),
  (47842, "Small Ancillary Armor Repairer"),
  (47844, "Medium Ancillary Armor Repairer"),
  (47846, "Large Ancillary Armor Repairer"),
  (56308, "Capital Ancillary Armor Repairer"),
  (47749, "1MN Afterburner"),
  (47753, "10MN Afterburner"),
  (47757, "100MN Afterburner"),
  (56305, "10000MN Afterburner"),
  (47740, "5MN Microwarpdrive"),
  (47408, "50MN Microwarpdrive"),
  (47745, "500MN Microwarpdrive"),
  (56306, "50000MN Microwarpdrive"),
  (90502, "Ice Mining Laser"),
  (90524, "Ice Harvester"),
  (90529, "Gas Cloud Scoop"),
  (90593, "Gas Cloud Harvester"),
  (47824, "Small Energy Neutralizer"),
  (47828, "Medium Energy Neutralizer"),
  (47832, "Heavy Energy Neutralizer"),
  (56312, "Capital Energy Neutralizer"),
  (48419, "Small Energy Nosferatu"),
  (48423, "Medium Energy Nosferatu"),
  (48427, "Heavy Energy Nosferatu"),
  (56311, "Capital Energy Nosferatu"),
  (48431, "Small Cap Battery"),
  (48435, "Medium Cap Battery"),
  (48439, "Large Cap Battery"),
  (52227, "Damage Control"),
  (52230, "Assault Damage Control"),
  (84442, "Small EMP Smartbomb"),
  (84438, "Medium EMP Smartbomb"),
  (84434, "Large EMP Smartbomb"),
  (84444, "Small Graviton Smartbomb"),
  (84440, "Medium Graviton Smartbomb"),
  (84436, "Large Graviton Smartbomb"),
  (84443, "Small Plasma Smartbomb"),
  (84439, "Medium Plasma Smartbomb"),
  (84435, "Large Plasma Smartbomb"),
  (84445, "Small Proton Smartbomb"),
  (84441, "Medium Proton Smartbomb"),
  (84437, "Large Proton Smartbomb"),
  (90614, "Mining Drone"),
  (90618, "Ice Harvesting Drone"),
  (90621, "'Excavator' Mining Drone"),
  (90622, "'Excavator' Ice Harvesting Drone"),
];

fn modal_source_pattern(type_id: i32) -> Option<&'static str> {
  MODAL_SOURCE_PATTERNS
    .iter()
    .find(|&&(tid, _)| tid == type_id)
    .map(|&(_, p)| p)
}

fn modal_type_chip(label: &str, type_id: i32, selected: bool) -> Element<'static, Message> {
  let (bg, border_col, text_col) = if selected {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.14))),
      color::text::ACCENT,
      color::text::ACCENT,
    )
  } else {
    (
      Some(Background::Color(color::surface::BASE)),
      color::border::SUBTLE,
      color::text::SECONDARY,
    )
  };
  let label = label.to_string();
  button(
    text(label)
      .font(body::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_col),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::TypeSelected(Some(type_id)))
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      color: border_col,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: text_col,
    ..button::Style::default()
  })
  .into()
}

fn modal_single_row(label: &'static str, type_id: i32, selected: bool) -> Element<'static, Message> {
  let (bg, text_col, border_col) = if selected {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.10))),
      color::text::ACCENT,
      color::text::ACCENT,
    )
  } else {
    (None, color::text::PRIMARY, Color::TRANSPARENT)
  };
  button(
    text(label)
      .font(body::REGULAR)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_col),
      })
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(Message::TypeSelected(Some(type_id)))
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      color: border_col,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: text_col,
    ..button::Style::default()
  })
  .into()
}

fn modal_family_row(
  name: &'static str,
  variants: &'static [ModalEntry],
  selected_id: Option<i32>,
) -> Element<'static, Message> {
  let some_selected = variants.iter().any(|e| selected_id == Some(e.type_id));
  let (bg, border_col) = if some_selected {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.06))),
      color::with_alpha(color::text::ACCENT, 0.30),
    )
  } else {
    (None, Color::TRANSPARENT)
  };
  let name_el = text(name)
    .font(body::MEDIUM)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });
  let chips: Vec<Element<'static, Message>> = variants
    .iter()
    .map(|e| modal_type_chip(e.label, e.type_id, selected_id == Some(e.type_id)))
    .collect();
  container(column([
    name_el.into(),
    Space::new().height(8.0).into(),
    row(chips).spacing(4.0).wrap().into(),
  ]))
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .style(move |_| container::Style {
    background: bg,
    border: Border {
      color: border_col,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn modal_section_el(section: &'static ModalSection, selected_id: Option<i32>) -> Element<'static, Message> {
  let title = column([
    text(section.title)
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::ACCENT),
      })
      .into(),
    Space::new().height(6.0).into(),
    section_divider(),
    Space::new().height(8.0).into(),
  ])
  .width(Length::Fill);

  let rows: Vec<Element<'static, Message>> = section
    .rows
    .iter()
    .map(|row| match row {
      ModalRow::Single {
        label,
        type_id,
      } => modal_single_row(label, *type_id, selected_id == Some(*type_id)),
      ModalRow::Family {
        name,
        variants,
        ..
      } => modal_family_row(name, variants, selected_id),
    })
    .collect();

  column([title.into(), column(rows).spacing(2.0).width(Length::Fill).into()])
    .width(Length::Fill)
    .into()
}

fn abyssals_modal(state: &State) -> Element<'_, Message> {
  let selected_id = state.abyssals.selected_source_type_id;

  let build_col = |sections: &'static [ModalSection]| -> Vec<Element<'static, Message>> {
    sections
      .iter()
      .map(|s| {
        container(modal_section_el(s, selected_id))
          .padding(Padding {
            bottom: 24.0,
            ..Padding::ZERO
          })
          .into()
      })
      .collect()
  };

  let col0 = build_col(MODAL_LAYOUT[0]);
  let col1 = build_col(MODAL_LAYOUT[1]);
  let col2 = build_col(MODAL_LAYOUT[2]);

  let subtitle = selected_id
    .and_then(modal_selected_label)
    .unwrap_or_else(|| "Pick a module type".to_string());

  let mut header_row_items: Vec<Element<'_, Message>> = vec![
    column([
      text("Filter by module type")
        .font(body::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(2.0).into(),
      text(subtitle)
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .width(Length::Fill)
    .into(),
  ];

  if selected_id.is_some() {
    header_row_items.push(
      button(
        text("Clear")
          .font(body::REGULAR)
          .size(11.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .padding(Padding {
        top: 5.0,
        bottom: 5.0,
        left: 10.0,
        right: 10.0,
      })
      .on_press(Message::TypeSelected(None))
      .style(|_, _| button::Style {
        background: None,
        border: Border {
          color: color::border::SUBTLE,
          radius: 5.0.into(),
          width: 1.0,
        },
        text_color: color::text::SECONDARY,
        ..button::Style::default()
      })
      .into(),
    );
    header_row_items.push(Space::new().width(8.0).into());
  }

  header_row_items.push(
    button(
      text("\u{00d7}")
        .font(mono::REGULAR)
        .size(18.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(28.0)
    .height(28.0)
    .on_press(Message::CloseTypeModal)
    .style(|_, _| button::Style {
      background: None,
      border: Border::default(),
      text_color: color::text::SECONDARY,
      ..button::Style::default()
    })
    .into(),
  );

  let panel_header = container(row(header_row_items).align_y(iced::alignment::Vertical::Center))
    .padding(Padding {
      top: 16.0,
      bottom: 16.0,
      left: 24.0,
      right: 24.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  let panel_body = scrollable(
    container(row([
      column(col0).width(Length::Fill).into(),
      Space::new().width(44.0).into(),
      column(col1).width(Length::Fill).into(),
      Space::new().width(44.0).into(),
      column(col2).width(Length::Fill).into(),
    ]))
    .padding(Padding {
      top: 24.0,
      bottom: 28.0,
      left: 32.0,
      right: 32.0,
    })
    .width(Length::Fill),
  )
  .height(Length::Fill);

  let panel_footer = container(
    row([
      text("esc \u{00b7} close")
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(Length::Fill)
        .into(),
      button(
        text("Done")
          .font(body::MEDIUM)
          .size(12.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::surface::BASE),
          }),
      )
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 18.0,
        right: 18.0,
      })
      .on_press(Message::CloseTypeModal)
      .style(|_, _| button::Style {
        background: Some(Background::Color(color::text::ACCENT)),
        border: Border {
          radius: 6.0.into(),
          ..Border::default()
        },
        text_color: color::surface::BASE,
        ..button::Style::default()
      })
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 24.0,
    right: 24.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  });

  let panel = container(column([
    panel_header.into(),
    section_divider(),
    panel_body.into(),
    section_divider(),
    panel_footer.into(),
  ]))
  .max_width(1180.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::border::DEFAULT,
      radius: 12.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  container(panel)
    .center(Length::Fill)
    .padding(32.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARKER)),
      ..container::Style::default()
    })
    .into()
}

fn stat_ranges_section<'a>(abyssals_state: &'a AbyssalsState, source_type_id: i32) -> Element<'a, Message> {
  let editing_key = abyssals_state.slider_editing.as_ref();
  let editing_text = abyssals_state.slider_edit_text.as_str();
  let template_stats: Vec<&AbyssalStatViewModel> = abyssals_state
    .abyssals
    .iter()
    .find(|i| i.type_id == source_type_id)
    .map(|i| i.stats.iter().collect())
    .unwrap_or_else(|| {
      modal_source_pattern(source_type_id)
        .and_then(|pattern| {
          abyssals_state
            .categories
            .iter()
            .flat_map(|c| c.source_types.iter())
            .find(|t| !t.stat_templates.is_empty() && t.name.starts_with(pattern))
            .map(|t| t.stat_templates.iter().collect())
        })
        .unwrap_or_default()
    });

  if template_stats.is_empty() {
    return Space::new().into();
  }

  let mut sliders: Vec<Element<'_, Message>> = vec![];

  for stat in &template_stats {
    let lo_raw = stat.base_value * stat.min_mult;
    let hi_raw = stat.base_value * stat.max_mult;
    let lo = lo_raw.min(hi_raw);
    let hi = lo_raw.max(hi_raw);
    if (hi - lo).abs() < 1e-9 {
      continue;
    }
    let filter = abyssals_state.stat_range_filters.get(&stat.attribute_id).copied();
    let editing = editing_key.map(|k| (k, editing_text));
    sliders.push(stat_slider_row(stat, filter, (lo, hi), editing));
    sliders.push(Space::new().height(14.0).into());
  }

  if sliders.is_empty() {
    return Space::new().into();
  }

  filter_section("Stat ranges", column(sliders).width(Length::Fill).into())
}

fn stat_ranges_placeholder() -> Element<'static, Message> {
  container(
    text("Pick a module type to filter by its rolled stats.")
      .font(body::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding {
    top: 20.0,
    bottom: 20.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill)
  .into()
}

fn filter_pane_drag_handle() -> Element<'static, Message> {
  mouse_area(
    container(Space::new().width(4.0).height(Length::Fill))
      .width(4.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      }),
  )
  .on_press(Message::PaneDragStart)
  .interaction(iced::mouse::Interaction::ResizingHorizontally)
  .into()
}

fn filter_sidebar<'a>(state: &'a State) -> Element<'a, Message> {
  let abyssals_state = &state.abyssals;
  let has_filter = abyssals_state.selected_source_type_id.is_some() || !abyssals_state.stat_range_filters.is_empty();

  let header = container(
    row([
      text("FILTERS")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(Length::Fill)
        .into(),
      sidebar_reset_button(has_filter),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 10.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill);

  let header_with_border = column([header.into(), section_divider()]).width(Length::Fill);

  let filter_btn = module_type_filter_button(abyssals_state.selected_source_type_id.is_some());
  let mut module_type_items: Vec<Element<'_, Message>> = vec![filter_btn];
  if let Some(type_id) = abyssals_state.selected_source_type_id {
    let type_name = modal_selected_label(type_id).unwrap_or_else(|| "Unknown".to_string());
    module_type_items.push(Space::new().height(10.0).into());
    module_type_items.push(selected_type_chip(&type_name));
  }
  let module_section = filter_section("Module Type", column(module_type_items).width(Length::Fill).into());

  let stat_el = match abyssals_state.selected_source_type_id {
    Some(src_id) => stat_ranges_section(abyssals_state, src_id),
    None => stat_ranges_placeholder(),
  };

  let body = scrollable(column([module_section, stat_el]).width(Length::Fill)).height(Length::Fill);

  let pane_width = abyssals_state.filter_pane_width.clamp(160.0, 450.0);

  container(column([header_with_border.into(), body.into()]))
    .width(Length::Fixed(pane_width))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

/// State for the abyssals tab sub-view.
#[derive(Clone, Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct AbyssalsState {
  /// All loaded abyssal view models.
  pub abyssals: Vec<AbyssalViewModel>,
  /// All abyssal source-type categories from the SDE, sorted by definition order.
  pub categories: Vec<AbyssalCategory>,
  /// Whether the filter pane drag is in progress.
  pub filter_pane_dragging: bool,
  /// Last recorded x position during filter pane drag.
  pub filter_pane_last_drag_x: f32,
  /// Current width of the filter sidebar pane in pixels.
  pub filter_pane_width: f32,
  /// Whether the module-type picker modal is open.
  pub modal_open: bool,
  /// Cached portrait image handles keyed by character_id, pre-built to avoid
  /// per-render allocation which causes flickering.
  pub portrait_handles: HashMap<i64, image::Handle>,
  /// Selected source type ID for the stat-range filter (None = all types).
  pub selected_source_type_id: Option<i32>,
  /// Per-attribute-id filter range (min_val, max_val).
  pub stat_range_filters: HashMap<i32, (f64, f64)>,
  /// Which slider value label is currently being edited, if any.
  pub slider_editing: Option<(i32, SliderEndpoint)>,
  /// Current text in the active slider value edit field.
  pub slider_edit_text: String,
  /// Loaded EVE icons keyed by source_type_id.
  pub type_icons: HashMap<i32, image::Handle>,
}

impl Default for AbyssalsState {
  fn default() -> Self {
    Self {
      abyssals: Vec::new(),
      categories: Vec::new(),
      filter_pane_dragging: false,
      filter_pane_last_drag_x: 0.0,
      filter_pane_width: 220.0,
      modal_open: false,
      portrait_handles: HashMap::new(),
      selected_source_type_id: None,
      slider_edit_text: String::new(),
      slider_editing: None,
      stat_range_filters: HashMap::new(),
      type_icons: HashMap::new(),
    }
  }
}

/// Builder for the abyssals tab.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new abyssals tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the abyssals tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let sidebar_el = filter_sidebar(state);
    let drag_handle = filter_pane_drag_handle();
    let grid_el = card_grid(state);

    let base = row([sidebar_el, drag_handle, grid_el])
      .width(Length::Fill)
      .height(Length::Fill);

    if state.abyssals.modal_open {
      stack([base.into(), abyssals_modal(state)])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
      base.into()
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod roll_score {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_item(stats: Vec<AbyssalStatViewModel>) -> AbyssalViewModel {
      AbyssalViewModel {
        base_type_name: "Test Module".to_string(),
        character_id: 1,
        item_id: 100,
        location: "Jita".to_string(),
        muta_price_isk: None,
        mutaplasmid_color_hue: 220,
        mutaplasmid_tier: "Decayed".to_string(),
        source_type_id: 1,
        stats,
        type_id: 47804,
      }
    }

    fn make_stat(display_name: &str, base: f64, rolled: f64, high_is_good: bool) -> AbyssalStatViewModel {
      AbyssalStatViewModel {
        attribute_id: 1,
        base_value: base,
        display_name: display_name.to_string(),
        high_is_good,
        icon_id: None,
        max_mult: 1.5,
        min_mult: 0.7,
        rolled_value: rolled,
        unit_suffix: "".to_string(),
      }
    }

    #[test]
    fn it_returns_zero_for_empty_stats() {
      let item = make_item(vec![]);

      assert_eq!(roll_score(&item), 0.0);
    }

    #[test]
    fn it_returns_positive_for_good_rolls() {
      let stats = vec![
        make_stat("Damage", 100.0, 110.0, true),
        make_stat("CPU Use", 50.0, 45.0, false),
      ];
      let item = make_item(stats);

      let score = roll_score(&item);

      assert!(score > 0.0);
    }

    #[test]
    fn it_returns_negative_for_bad_rolls() {
      let stats = vec![
        make_stat("Damage", 100.0, 90.0, true),
        make_stat("CPU Use", 50.0, 55.0, false),
      ];
      let item = make_item(stats);

      let score = roll_score(&item);

      assert!(score < 0.0);
    }
  }

  mod format_stat_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_percentage_trimming_trailing_zero() {
      assert_eq!(format_stat_value(35.5, "%"), "35.5%");
    }

    #[test]
    fn it_formats_large_whole_values_without_decimals() {
      assert_eq!(format_stat_value(50_000.0, " kg"), "50000 kg");
    }

    #[test]
    fn it_formats_medium_whole_values_without_decimals() {
      assert_eq!(format_stat_value(1_500.0, " HP"), "1500 HP");
    }

    #[test]
    fn it_formats_values_trimming_trailing_zero() {
      assert_eq!(format_stat_value(25.5, " tf"), "25.5 tf");
    }

    #[test]
    fn it_formats_values_with_two_significant_decimals() {
      assert_eq!(format_stat_value(4.75, " GJ"), "4.75 GJ");
    }
  }
}
