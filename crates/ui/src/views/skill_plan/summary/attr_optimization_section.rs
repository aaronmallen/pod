//! Attribute optimisation section — implant-set picker, current/proposed
//! attribute columns, savings callout, and remap availability status.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::{super::Message, fmt_time_long};
use crate::{
  plan_math::{BaseAttrs, EffectiveAttrs, ImplantBonus, ImplantSet, RemapResult},
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::skills::skill_data::AttrKey,
};

fn read_attr(attrs: &BaseAttrs, key: AttrKey) -> i32 {
  match key {
    AttrKey::Charisma => attrs.charisma,
    AttrKey::Intelligence => attrs.intelligence,
    AttrKey::Memory => attrs.memory,
    key => read_attr_ext(attrs, key),
  }
}

fn read_attr_ext(attrs: &BaseAttrs, key: AttrKey) -> i32 {
  match key {
    AttrKey::Perception => attrs.perception,
    _ => attrs.willpower,
  }
}

fn attr_base(attrs: &BaseAttrs, key: AttrKey) -> i32 {
  read_attr(attrs, key)
}

fn attr_implant(implant: &ImplantBonus, key: AttrKey) -> i32 {
  read_attr(implant, key)
}

fn ghost_btn_bg(status: button::Status) -> Background {
  Background::Color(match status {
    button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_SUBTLE,
    _ => Color::TRANSPARENT,
  })
}

fn ghost_btn_border_color(status: button::Status) -> Color {
  match status {
    button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_MUTED,
    _ => color::border::SUBTLE,
  }
}

fn ghost_button(label: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: Some(ghost_btn_bg(status)),
    border: Border {
      color: ghost_btn_border_color(status),
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .into()
}

fn implant_btn_border_color(active: bool) -> Color {
  if active {
    color::accent::PLASMA_MUTED
  } else {
    color::border::SUBTLE
  }
}

fn implant_btn_text_color(active: bool) -> Color {
  if active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  }
}

fn implant_set_btn_bg(active: bool, bg: Color, status: button::Status) -> Background {
  Background::Color(if active { bg } else { implant_btn_hover_bg(status) })
}

fn implant_set_button(set: ImplantSet, label: &'static str, active: bool) -> Element<'static, Message> {
  let bg = implant_btn_bg(active);
  let border_color = implant_btn_border_color(active);
  let text_color = implant_btn_text_color(active);

  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::ImplantSetChanged(set))
  .style(move |_, status| button::Style {
    background: Some(implant_set_btn_bg(active, bg, status)),
    border: Border {
      color: border_color,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn implant_btn_bg(active: bool) -> Color {
  if active {
    color::accent::PLASMA_SUBTLE
  } else {
    Color::TRANSPARENT
  }
}

fn implant_btn_hover_bg(status: button::Status) -> Color {
  match status {
    button::Status::Hovered => color::with_alpha(color::accent::PLASMA, 0.05),
    _ => Color::TRANSPARENT,
  }
}

fn implant_set_picker(current: ImplantSet, clone_data_missing: bool) -> Element<'static, Message> {
  let sets = [
    (ImplantSet::None, "None"),
    (ImplantSet::Plus3, "+3"),
    (ImplantSet::Plus4, "+4"),
    (ImplantSet::Plus5, "+5"),
    (ImplantSet::Current, "Current"),
  ];

  let buttons: Vec<Element<'_, Message>> = sets
    .iter()
    .map(|(set, label)| implant_set_button(*set, label, current == *set))
    .collect();

  let picker_row = row(buttons).spacing(4.0);

  if current == ImplantSet::Current && clone_data_missing {
    column([
      picker_row.into(),
      Space::new().height(4.0).into(),
      text("(clone not synced)")
        .font(crate::style::typography::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    ])
    .width(Length::Fill)
    .into()
  } else {
    picker_row.into()
  }
}

fn optimization_header_row() -> Element<'static, Message> {
  row([
    text("ATTRIBUTE OPTIMIZATION")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .width(Length::Fill)
      .into(),
    ghost_button("Optimize", Message::OptimizerRequested),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn attr_base_val_color(highlight: bool) -> Color {
  if highlight {
    color::accent::PLASMA
  } else {
    color::text::PRIMARY
  }
}

fn attr_implant_cell(implant_val: i32) -> Element<'static, Message> {
  if implant_val > 0 {
    text(format!("+{implant_val}"))
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SUCCESS),
      })
      .width(Length::Fixed(28.0))
      .into()
  } else {
    Space::new().width(Length::Fixed(28.0)).into()
  }
}

fn attr_value_row(key: AttrKey, base_val: i32, implant_val: i32, highlight: bool) -> Element<'static, Message> {
  let base_color = attr_base_val_color(highlight);
  row([
    text(key.short())
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .width(Length::Fixed(30.0))
      .into(),
    text(key.label())
      .font(body::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .width(Length::Fill)
      .into(),
    text(base_val.to_string())
      .font(mono::MEDIUM)
      .size(12.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(base_color),
      })
      .into(),
    attr_implant_cell(implant_val),
  ])
  .align_y(Vertical::Center)
  .spacing(4.0)
  .into()
}

fn computing_text() -> Element<'static, Message> {
  container(
    text("Computing optimal remap\u{2026}")
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding::new(12.0))
  .width(Length::Fill)
  .into()
}

fn attr_panel_header(title: &'static str, highlight: bool) -> Element<'static, Message> {
  text(title)
    .font(mono::REGULAR)
    .size(9.0)
    .style(move |_| iced::widget::text::Style {
      color: Some(if highlight {
        color::accent::PLASMA
      } else {
        color::text::TERTIARY
      }),
    })
    .into()
}

fn attr_panel_container<'a>(highlight: bool, items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  let bg = if highlight {
    color::accent::PLASMA_SUBTLE
  } else {
    color::surface::SUNKEN
  };
  let border_color = if highlight {
    color::accent::PLASMA_MUTED
  } else {
    color::border::SUBTLE
  };

  container(column(items).spacing(2.0).width(Length::Fill))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 10.0,
      right: 10.0,
    })
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: border_color,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn attr_column_panel<'a>(
  title: &'static str,
  base: &'a BaseAttrs,
  implant: &'a ImplantBonus,
  highlight: bool,
) -> Element<'a, Message> {
  let header = attr_panel_header(title, highlight);
  let rows: Vec<Element<'_, Message>> = AttrKey::ALL
    .iter()
    .map(|&key| attr_value_row(key, attr_base(base, key), attr_implant(implant, key), highlight))
    .collect();

  let items: Vec<Element<'_, Message>> = std::iter::once(header)
    .chain(std::iter::once(Space::new().height(6.0).into()))
    .chain(rows)
    .collect();

  attr_panel_container(highlight, items)
}

fn single_attr_column<'a>(
  base: &'a BaseAttrs,
  effective: &'a EffectiveAttrs,
  implant: &'a ImplantBonus,
) -> Element<'a, Message> {
  let rows: Vec<Element<'_, Message>> = AttrKey::ALL
    .iter()
    .map(|&key| {
      let base_val = attr_base(base, key);
      let imp_val = attr_implant(implant, key);
      attr_value_row(key, base_val, imp_val, false)
    })
    .collect();

  let _ = effective;
  container(column(rows).spacing(2.0).width(Length::Fill))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 12.0,
      right: 12.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn dual_attr_columns<'a>(
  base: &'a BaseAttrs,
  effective: &'a EffectiveAttrs,
  implant: &'a ImplantBonus,
  result: &'a RemapResult,
) -> Element<'a, Message> {
  let _ = effective;
  let current_col = attr_column_panel("CURRENT", base, implant, false);
  let proposed_col = attr_column_panel("PROPOSED", &result.base, implant, true);

  row([current_col, Space::new().width(8.0).into(), proposed_col])
    .width(Length::Fill)
    .into()
}

fn savings_callout_style(result: &RemapResult) -> (Color, Color, Color, String) {
  if result.is_current {
    (
      color::with_alpha(color::text::SUCCESS, 0.08),
      color::with_alpha(color::text::SUCCESS, 0.30),
      color::text::SUCCESS,
      "Already optimal".to_string(),
    )
  } else {
    let saved = (result.current_sec - result.total_sec).max(0.0);
    (
      color::accent::PLASMA_SUBTLE,
      color::accent::PLASMA_MUTED,
      color::accent::PLASMA,
      format!("\u{2212}{}", fmt_time_long(saved)),
    )
  }
}

fn savings_callout(result: &RemapResult, effective: &EffectiveAttrs) -> Element<'static, Message> {
  let _ = effective;
  let (bg_color, border_color, label_color, msg_str) = savings_callout_style(result);

  container(
    text(msg_str)
      .font(mono::MEDIUM)
      .size(13.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(label_color),
      }),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 14.0,
    right: 14.0,
  })
  .width(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_color)),
    border: Border {
      color: border_color,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn remap_status_info(cooldown_days: i32, remap_available: bool) -> (Color, String) {
  if cooldown_days > 0 {
    (
      color::status::CAUTION,
      format!("Remap on cooldown for {cooldown_days} days"),
    )
  } else if remap_available {
    (color::status::ONLINE, "Remap available now".to_string())
  } else {
    (color::text::TERTIARY, "No remap available".to_string())
  }
}

fn remap_status_row(cooldown_days: i32, remap_available: bool) -> Element<'static, Message> {
  let (dot_color, status_text) = remap_status_info(cooldown_days, remap_available);

  row([
    container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(6.0).into(),
    text(status_text)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(dot_color),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

/// Builder for the attribute optimisation section.
pub struct AttrOptimizationSection<'a> {
  /// Base character attributes.
  pub base_attrs: &'a BaseAttrs,
  /// Bonus remap count (reserved for future use).
  pub bonus_remaps: u32,
  /// Whether active-clone data is missing.
  pub clone_data_missing: bool,
  /// Effective character attributes after implants.
  pub effective_attrs: &'a EffectiveAttrs,
  /// Currently equipped implant bonus values.
  pub implant: &'a ImplantBonus,
  /// Active implant set selection.
  pub implant_set: ImplantSet,
  /// Whether a remap is currently available.
  pub remap_available: bool,
  /// Days until remap cooldown expires (0 = available).
  pub remap_cooldown_days: i32,
  /// Computed optimizer result, if available.
  pub optimizer_result: Option<&'a RemapResult>,
  /// Whether the optimizer is currently running.
  pub optimizer_running: bool,
  /// Whether to show the remap comparison columns.
  pub show_remap: bool,
}

impl<'a> AttrOptimizationSection<'a> {
  /// Create a new `AttrOptimizationSection`.
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    base_attrs: &'a BaseAttrs,
    effective_attrs: &'a EffectiveAttrs,
    implant: &'a ImplantBonus,
    implant_set: ImplantSet,
    optimizer_result: Option<&'a RemapResult>,
    optimizer_running: bool,
    show_remap: bool,
    remap_cooldown_days: i32,
    remap_available: bool,
    bonus_remaps: u32,
    clone_data_missing: bool,
  ) -> Self {
    Self {
      base_attrs,
      bonus_remaps,
      clone_data_missing,
      effective_attrs,
      implant,
      implant_set,
      optimizer_result,
      optimizer_running,
      remap_available,
      remap_cooldown_days,
      show_remap,
    }
  }

  /// Render the section into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let mut items: Vec<Element<'_, Message>> = vec![
      optimization_header_row(),
      Space::new().height(spacing::SPACE_3).into(),
      implant_set_picker(self.implant_set, self.clone_data_missing),
      Space::new().height(spacing::SPACE_3).into(),
    ];

    if !self.show_remap {
      items.push(single_attr_column(self.base_attrs, self.effective_attrs, self.implant));
    } else if self.optimizer_running {
      items.push(computing_text());
    } else if let Some(result) = self.optimizer_result {
      items.push(dual_attr_columns(
        self.base_attrs,
        self.effective_attrs,
        self.implant,
        result,
      ));
      items.push(Space::new().height(spacing::SPACE_3).into());
      items.push(savings_callout(result, self.effective_attrs));
      items.push(Space::new().height(spacing::SPACE_3).into());
      items.push(remap_status_row(self.remap_cooldown_days, self.remap_available));
    } else {
      items.push(single_attr_column(self.base_attrs, self.effective_attrs, self.implant));
    }

    container(column(items).width(Length::Fill))
      .padding(Padding {
        top: spacing::SPACE_3,
        bottom: spacing::SPACE_4,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}
