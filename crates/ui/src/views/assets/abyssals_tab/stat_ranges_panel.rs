//! Stat-range filter panel — sliders and value labels for per-stat filter bounds.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Canvas, Space, button, column, container, row, text, text_input},
};
use pod_model::AbyssalStatViewModel;

use super::{
  AbyssalsState, Message, RangeSliderProgram, SliderEndpoint, filter_section, format_stat_value,
  module_type_picker::modal_source_pattern,
};
use crate::style::{
  color,
  typography::{body, mono},
};

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

/// Builder for the stat-range filter panel displayed in the filter sidebar.
pub struct Component<'a> {
  abyssals_state: &'a AbyssalsState,
  source_type_id: i32,
}

impl<'a> Component<'a> {
  /// Creates a new stat ranges panel for the given abyssals state and source type.
  pub fn new(abyssals_state: &'a AbyssalsState, source_type_id: i32) -> Self {
    Self {
      abyssals_state,
      source_type_id,
    }
  }

  /// Renders the stat ranges panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    stat_ranges_section(self.abyssals_state, self.source_type_id)
  }
}

/// Renders the placeholder shown when no module type is selected.
pub fn placeholder() -> Element<'static, Message> {
  stat_ranges_placeholder()
}
