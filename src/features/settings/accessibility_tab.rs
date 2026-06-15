use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, slider, text},
};

use super::Outcome;
use crate::{
  config::Settings,
  ui::{
    components::{rule, toggle},
    style::{color, control, radius, spacing, typography},
  },
};

const PANEL_SIDE_PADDING: f32 = 36.0;
const PRESET_HEIGHT: f32 = 78.0;
const READOUT_MAX_WIDTH: f32 = 640.0;
const SCALE_MAX: u8 = 150;
const SCALE_MIN: u8 = 85;

const PRESETS: [Preset; 5] = [
  Preset {
    label: "XS",
    pct: 85,
  },
  Preset {
    label: "S",
    pct: 92,
  },
  Preset {
    label: "M",
    pct: 100,
  },
  Preset {
    label: "L",
    pct: 125,
  },
  Preset {
    label: "XL",
    pct: 150,
  },
];

const PREVIEW_SAMPLE: &str = "The quick brown fox";
const TIER_COLUMN_WIDTH: f32 = 168.0;

const TIERS: [Tier; 3] = [
  Tier {
    hc: color::text::secondary_hc,
    name: "Secondary",
    off: color::text::secondary_off,
    target: "Lc 75",
    usage: "Body, labels, descriptions",
  },
  Tier {
    hc: color::text::tertiary_hc,
    name: "Tertiary",
    off: color::text::tertiary_off,
    target: "Lc 60",
    usage: "Meta, captions, hints",
  },
  Tier {
    hc: color::text::dim_hc,
    name: "Dim",
    off: color::text::dim_off,
    target: "Lc 45",
    usage: "Disabled, placeholder marks",
  },
];

#[derive(Clone, Copy, Debug)]
pub enum Message {
  HighContrastToggled(bool),
  ScaleChanged(u8),
}

#[derive(Debug, Default)]
pub struct State;

impl State {
  pub fn from_settings(_settings: &Settings) -> Self {
    State
  }
}

#[derive(Clone, Copy, Debug)]
struct Preset {
  label: &'static str,
  pct: u8,
}

#[derive(Clone, Copy)]
struct Tier {
  hc: fn() -> iced::Color,
  name: &'static str,
  off: fn() -> iced::Color,
  target: &'static str,
  usage: &'static str,
}

fn clamp_scale(scale: u8) -> u8 {
  scale.clamp(SCALE_MIN, SCALE_MAX)
}

fn preset_for(scale: u8) -> Option<Preset> {
  PRESETS.into_iter().find(|preset| preset.pct == scale)
}

pub fn update(_state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::HighContrastToggled(enabled) => {
      settings.accessibility_mut().set_high_contrast(enabled);
      Outcome::AccessibilityChanged
    }
    Message::ScaleChanged(scale) => {
      settings.accessibility_mut().set_scale(clamp_scale(scale));
      Outcome::AccessibilityChanged
    }
  }
}

pub fn badge(settings: &Settings) -> String {
  let scale = clamp_scale(*settings.accessibility().scale());
  let mut badge = if preset_for(scale).is_some() {
    format!("{scale}%")
  } else {
    format!("{scale}% \u{00b7} custom")
  };
  if *settings.accessibility().high_contrast() {
    badge.push_str(" \u{00b7} HC");
  }
  badge
}

pub fn view<'a>(_state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header();
  let body = panel_body(settings);

  Column::with_children(vec![header, body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header<'a>() -> Element<'a, Message> {
  let title = text("Accessibility")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(
    "Make Pod easier to read. Interface scale applies live across every window \u{2014} no restart \
      needed.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let band = container(identity).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn panel_body(settings: &Settings) -> Element<'_, Message> {
  let scale = clamp_scale(*settings.accessibility().scale());
  let high_contrast = *settings.accessibility().high_contrast();

  let section = section_head(
    "Interface scale",
    "Scales layout and text together. M (100%) is the default; the range spans 85% to 150%.",
    "Applies live \u{00b7} all windows",
  );
  let presets = scale_presets(scale);
  let readout = scale_readout(scale);
  let fine = fine_scale(scale);

  let contrast_head = section_head(
    "Contrast",
    "One alternate palette behind a single toggle. Targets: Lc 75 body, Lc 60 secondary.",
    if high_contrast { "On" } else { "Off" },
  );
  let contrast_toggle = contrast_toggle_row(high_contrast);
  let contrast_preview = contrast_preview(high_contrast);

  let inner = container(
    Column::with_children(vec![
      section,
      presets,
      readout,
      fine,
      contrast_head,
      contrast_toggle,
      contrast_preview,
    ])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::UNIT,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn section_head<'a>(label: &'a str, note: &'a str, chip: &'a str) -> Element<'a, Message> {
  let micro = text(label)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent::PLASMA));
  let detail = text(note)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![micro.into(), detail.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![identity.into(), live_chip(chip)])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5);

  let band = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: 0.0,
    bottom: spacing::SPACE_3,
    left: 0.0,
  });

  Column::with_children(vec![band.into(), rule::horizontal_alpha(0.18)])
    .width(Length::Fill)
    .into()
}

fn live_chip(label: &str) -> Element<'_, Message> {
  let dot = container(Space::new())
    .width(Length::Fixed(6.0))
    .height(Length::Fixed(6.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::status::ONLINE)),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let label = text(label.to_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));

  let row = Row::with_children(vec![dot.into(), label.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  container(row)
    .padding(Padding {
      top: spacing::UNIT + 1.0,
      right: spacing::SPACE_3,
      bottom: spacing::UNIT + 1.0,
      left: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn contrast_toggle_row(high_contrast: bool) -> Element<'static, Message> {
  let heading = text("High contrast")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let title = Row::with_children(vec![heading.into(), apca_tag()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5);
  let blurb = text(
    "Swaps the three secondary text tiers \u{2014} secondary, tertiary, and dim \u{2014} from \
      reduced-opacity overlays to solid, tuned values, and firms up surface borders. Primary text \
      and the dark theme are unchanged.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  let switch = toggle::toggle(high_contrast, Message::HighContrastToggled(!high_contrast));

  let row = Row::with_children(vec![identity.into(), switch])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill);

  container(row).width(Length::Fill).into()
}

fn apca_tag() -> Element<'static, Message> {
  let label = text("APCA")
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS)
    .style(typography::colored(color::accent::PLASMA));

  container(label)
    .padding(Padding {
      top: 2.0,
      right: spacing::SPACE_2 - 2.0,
      bottom: 2.0,
      left: spacing::SPACE_2 - 2.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, 0.3),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn contrast_preview(high_contrast: bool) -> Element<'static, Message> {
  let mut rows: Vec<Element<'static, Message>> = Vec::with_capacity(TIERS.len() + 2);
  rows.push(preview_header_row());
  for tier in TIERS {
    rows.push(preview_tier_row(tier, high_contrast));
  }
  rows.push(preview_swatch_row(high_contrast));

  let edge = if high_contrast {
    color::rule_strong()
  } else {
    color::rule()
  };

  container(Column::with_children(rows).width(Length::Fill))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: edge,
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn preview_header_row() -> Element<'static, Message> {
  let heading = |label: &'static str, align: Horizontal| {
    container(
      text(label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fill)
    .align_x(align)
  };

  let tier = container(
    text("Tier")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fixed(TIER_COLUMN_WIDTH));

  let row = Row::with_children(vec![
    tier.into(),
    heading("Today", Horizontal::Left).into(),
    heading("High contrast", Horizontal::Left).into(),
    heading("APCA", Horizontal::Right).into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_3)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn preview_tier_row(tier: Tier, high_contrast: bool) -> Element<'static, Message> {
  let name = text(tier.name)
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let usage = text(tier.usage)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let label = container(Column::with_children(vec![name.into(), usage.into()]).spacing(spacing::UNIT))
    .width(Length::Fixed(TIER_COLUMN_WIDTH));

  let today = container(
    text(PREVIEW_SAMPLE)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored((tier.off)())),
  )
  .width(Length::Fill);
  let hc = container(
    text(PREVIEW_SAMPLE)
      .font(if high_contrast {
        typography::body::MEDIUM
      } else {
        typography::body::REGULAR
      })
      .size(typography::size::MD)
      .style(typography::colored((tier.hc)())),
  )
  .width(Length::Fill);

  let target_color = if high_contrast {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let target = container(
    text(tier.target)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(target_color)),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Right);

  let row = Row::with_children(vec![label.into(), today.into(), hc.into(), target.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  let edge = if high_contrast {
    color::rule()
  } else {
    color::with_alpha(color::text::PRIMARY, 0.1)
  };

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      border: Border {
        color: edge,
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn preview_swatch_row(high_contrast: bool) -> Element<'static, Message> {
  let caption = text("Surface edges")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));

  let row = Row::with_children(vec![
    caption.into(),
    border_swatch("rule", color::rule_off_alpha(), color::rule_hc_alpha(), high_contrast),
    border_swatch(
      "ruleStrong",
      color::rule_strong_off_alpha(),
      color::rule_strong_hc_alpha(),
      high_contrast,
    ),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_6)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn border_swatch(label: &'static str, off_alpha: f32, hc_alpha: f32, high_contrast: bool) -> Element<'static, Message> {
  let alpha = if high_contrast { hc_alpha } else { off_alpha };
  let swatch = container(Space::new())
    .width(Length::Fixed(34.0))
    .height(Length::Fixed(18.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, alpha),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    });

  let caption = text(format!("{label}  {off_alpha:.2}\u{2192}{hc_alpha:.2}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));

  Row::with_children(vec![swatch.into(), caption.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5)
    .into()
}

fn scale_presets(scale: u8) -> Element<'static, Message> {
  let mut cells: Vec<Element<'static, Message>> = Vec::with_capacity(PRESETS.len());
  for preset in PRESETS {
    cells.push(preset_cell(preset, scale == preset.pct));
  }

  Row::with_children(cells)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn preset_cell(preset: Preset, active: bool) -> Element<'static, Message> {
  let label_color = if active {
    color::text::PRIMARY
  } else {
    color::with_alpha(color::text::PRIMARY, 0.82)
  };
  let pct_color = if active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };

  let label = text(preset.label)
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(label_color));
  let pct = text(format!("{}%", preset.pct))
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(pct_color));

  let mut stack: Vec<Element<'static, Message>> = vec![label.into(), pct.into()];
  if preset.pct == 100 {
    let default_color = if active {
      color::accent::PLASMA
    } else {
      color::text::tertiary()
    };
    stack.push(
      text("Default")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(default_color))
        .into(),
    );
  }

  let content = Column::with_children(stack)
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Center);

  let cell = container(content)
    .width(Length::Fill)
    .height(Length::Fixed(PRESET_HEIGHT))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(if active {
        color::with_alpha(color::accent::PLASMA, 0.1)
      } else {
        color::surface::SUNKEN
      })),
      border: Border {
        color: if active {
          color::accent::PLASMA
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  button(cell)
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::ScaleChanged(preset.pct))
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

fn scale_readout(scale: u8) -> Element<'static, Message> {
  let caption = match preset_for(scale) {
    Some(preset) => format!("{} preset", preset.label),
    None => "custom (between steps)".to_owned(),
  };

  let now = text("Now:")
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::tertiary()));
  let value = text(format!("{scale}%"))
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let separator = text("\u{00b7}")
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::tertiary()));
  let detail = text(caption)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));

  let row = Row::with_children(vec![now.into(), value.into(), separator.into(), detail.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  container(row).max_width(READOUT_MAX_WIDTH).into()
}

fn fine_scale(scale: u8) -> Element<'static, Message> {
  let preset = preset_for(scale);

  let heading = text("Fine scale")
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let hint = text("Land between presets \u{00b7} 1% steps")
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![heading.into(), hint.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let value = text(format!("{scale}%"))
    .font(typography::mono::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let caption_color = if preset.is_some() {
    color::accent::PLASMA
  } else {
    color::status::WARNING
  };
  let caption = text(match preset {
    Some(preset) => format!("{} preset", preset.label),
    None => "Custom".to_owned(),
  })
  .font(typography::mono::REGULAR)
  .size(typography::size::XS)
  .style(typography::colored(caption_color));
  let readout = Column::with_children(vec![value.into(), caption.into()])
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Right);

  let top = Row::with_children(vec![labels.into(), readout.into()])
    .align_y(Vertical::Bottom)
    .spacing(spacing::SPACE_3_5);

  let track = slider(SCALE_MIN..=SCALE_MAX, scale, Message::ScaleChanged)
    .step(1u8)
    .height(6.0)
    .style(control::slider_track);

  let column = Column::with_children(vec![top.into(), track.into(), scale_ticks(scale)])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  container(column)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_6,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_6,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

// Milestone markers under the fine-scale slider: a tick + label at each preset,
// spaced proportionally to its position in the 85-150% range so the markers line
// up beneath the slider handle's travel. Each marker is clickable and jumps the
// scale to that preset, mirroring the segmented preset row above.
fn scale_ticks(scale: u8) -> Element<'static, Message> {
  let mut children: Vec<Element<'static, Message>> = Vec::with_capacity(PRESETS.len() * 2 - 1);
  let mut previous = SCALE_MIN;
  for (index, preset) in PRESETS.into_iter().enumerate() {
    if index > 0 {
      let gap = u16::from(preset.pct - previous);
      children.push(Space::new().width(Length::FillPortion(gap)).into());
    }
    children.push(tick_mark(preset, scale == preset.pct));
    previous = preset.pct;
  }

  Row::with_children(children).width(Length::Fill).into()
}

fn tick_mark(preset: Preset, active: bool) -> Element<'static, Message> {
  let bar = container(Space::new())
    .width(Length::Fixed(2.0))
    .height(Length::Fixed(6.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(if active {
        color::accent::PLASMA
      } else {
        color::rule_strong()
      })),
      ..container::Style::default()
    });
  let label = text(preset.label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(if active {
      color::accent::PLASMA
    } else {
      color::text::dim()
    }));

  let content = Column::with_children(vec![bar.into(), label.into()])
    .spacing(spacing::UNIT + 1.0)
    .align_x(Horizontal::Center);

  button(content)
    .padding(0)
    .on_press(Message::ScaleChanged(preset.pct))
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_a_bare_percentage_for_a_preset_value() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(125);

      assert_eq!(badge(&settings), "125%");
    }

    #[test]
    fn it_marks_a_non_preset_value_as_custom() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(112);

      assert_eq!(badge(&settings), "112% \u{00b7} custom");
    }

    #[test]
    fn it_reports_the_default_scale_as_a_preset() {
      assert_eq!(badge(&Settings::default()), "100%");
    }

    #[test]
    fn it_appends_an_hc_suffix_when_high_contrast_is_on() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_high_contrast(true);

      assert_eq!(badge(&settings), "100% \u{00b7} HC");
    }

    #[test]
    fn it_appends_the_hc_suffix_after_a_custom_scale() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(112);
      settings.accessibility_mut().set_high_contrast(true);

      assert_eq!(badge(&settings), "112% \u{00b7} custom \u{00b7} HC");
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn selecting_a_preset_sets_the_scale_and_signals_a_live_change() {
      let mut state = State;
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::ScaleChanged(125), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert_eq!(*settings.accessibility().scale(), 125);
    }

    #[test]
    fn dragging_to_a_custom_value_sets_the_same_scale_the_badge_reads() {
      let mut state = State;
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::ScaleChanged(112), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert_eq!(*settings.accessibility().scale(), 112);
      assert_eq!(badge(&settings), "112% \u{00b7} custom");
    }

    #[test]
    fn it_clamps_a_scale_below_the_minimum() {
      let mut state = State;
      let mut settings = Settings::default();

      update(&mut state, Message::ScaleChanged(10), &mut settings);

      assert_eq!(*settings.accessibility().scale(), SCALE_MIN);
    }

    #[test]
    fn it_clamps_a_scale_above_the_maximum() {
      let mut state = State;
      let mut settings = Settings::default();

      update(&mut state, Message::ScaleChanged(240), &mut settings);

      assert_eq!(*settings.accessibility().scale(), SCALE_MAX);
    }

    #[test]
    fn toggling_high_contrast_on_flips_the_flag_and_signals_a_live_change() {
      let mut state = State;
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::HighContrastToggled(true), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert!(*settings.accessibility().high_contrast());
    }

    #[test]
    fn toggling_high_contrast_off_clears_the_flag_and_signals_a_live_change() {
      let mut state = State;
      let mut settings = Settings::default();
      settings.accessibility_mut().set_high_contrast(true);

      let outcome = update(&mut state, Message::HighContrastToggled(false), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert!(!*settings.accessibility().high_contrast());
    }
  }

  mod preset_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_matches_each_preset_percentage() {
      for preset in PRESETS {
        assert_eq!(super::preset_for(preset.pct).map(|p| p.label), Some(preset.label));
      }
    }

    #[test]
    fn it_returns_none_for_an_off_preset_value() {
      assert!(super::preset_for(112).is_none());
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_accessibility_panel() {
      let settings = Settings::default();
      let state = State;

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_with_a_custom_scale() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(112);
      let state = State;

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_contrast_preview_with_high_contrast_on() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_high_contrast(true);
      let state = State;

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
