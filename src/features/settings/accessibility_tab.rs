use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, slider, text},
};

use super::Outcome;
use crate::{
  config::Settings,
  ui::{
    components::rule,
    style::{color, radius, spacing, typography},
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

#[derive(Clone, Copy, Debug)]
pub enum Message {
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

fn clamp_scale(scale: u8) -> u8 {
  scale.clamp(SCALE_MIN, SCALE_MAX)
}

fn preset_for(scale: u8) -> Option<Preset> {
  PRESETS.into_iter().find(|preset| preset.pct == scale)
}

pub fn update(_state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::ScaleChanged(scale) => {
      settings.accessibility_mut().set_scale(clamp_scale(scale));
      Outcome::AccessibilityChanged
    }
  }
}

pub fn badge(settings: &Settings) -> String {
  let scale = clamp_scale(*settings.accessibility().scale());
  if preset_for(scale).is_some() {
    format!("{scale}%")
  } else {
    format!("{scale}% \u{00b7} custom")
  }
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

  let section = section_head(
    "Interface scale",
    "Scales layout and text together. M (100%) is the default; the range spans 85% to 150%.",
  );
  let presets = scale_presets(scale);
  let readout = scale_readout(scale);
  let fine = fine_scale(scale);

  let inner = container(
    Column::with_children(vec![section, presets, readout, fine])
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

fn section_head<'a>(label: &'a str, note: &'a str) -> Element<'a, Message> {
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

  let row = Row::with_children(vec![identity.into(), live_chip()])
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

fn live_chip<'a>() -> Element<'a, Message> {
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
  let label = text("Applies live \u{00b7} all windows")
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
    .style(|_, _| slider::Style {
      rail: slider::Rail {
        backgrounds: (
          Background::Color(color::accent::PLASMA),
          Background::Color(color::with_alpha(color::text::PRIMARY, 0.12)),
        ),
        width: 6.0,
        border: Border {
          radius: radius::SUBTLE.into(),
          width: 0.0,
          color: iced::Color::TRANSPARENT,
        },
      },
      handle: slider::Handle {
        shape: slider::HandleShape::Circle {
          radius: 10.0,
        },
        background: Background::Color(color::accent::PLASMA),
        border_color: color::surface::BASE,
        border_width: 3.0,
      },
    });

  let column = Column::with_children(vec![top.into(), track.into()])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  container(column)
    .width(Length::Fill)
    .max_width(READOUT_MAX_WIDTH)
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
  }
}
