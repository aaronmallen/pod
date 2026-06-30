use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, slider, text},
};

use super::Outcome;
use crate::{
  config::Settings,
  services::i18n::Language,
  ui::{
    components::{
      button::{Button, Size},
      icon::Icon,
      rule, toggle,
    },
    style::{color, control, radius, spacing, typography},
  },
};

const LANGUAGE_GRID_COLUMNS: usize = 3;
const PANEL_SIDE_PADDING: f32 = 36.0;
const PRESET_HEIGHT: f32 = 78.0;
const READOUT_MAX_WIDTH: f32 = 640.0;
const SCALE_MAX: u8 = 150;
const SCALE_MIN: u8 = 85;

const PRESETS: [Preset; 5] = [
  Preset {
    label_key: "settings.accessibility.preset_xs",
    pct: 85,
  },
  Preset {
    label_key: "settings.accessibility.preset_s",
    pct: 92,
  },
  Preset {
    label_key: "settings.accessibility.preset_m",
    pct: 100,
  },
  Preset {
    label_key: "settings.accessibility.preset_l",
    pct: 125,
  },
  Preset {
    label_key: "settings.accessibility.preset_xl",
    pct: 150,
  },
];

const TIER_COLUMN_WIDTH: f32 = 168.0;

const TIERS: [Tier; 3] = [
  Tier {
    hc: color::text::secondary_hc,
    name_key: "settings.accessibility.tier_secondary_name",
    off: color::text::secondary_off,
    target: "Lc 75",
    usage_key: "settings.accessibility.tier_secondary_usage",
  },
  Tier {
    hc: color::text::tertiary_hc,
    name_key: "settings.accessibility.tier_tertiary_name",
    off: color::text::tertiary_off,
    target: "Lc 60",
    usage_key: "settings.accessibility.tier_tertiary_usage",
  },
  Tier {
    hc: color::text::dim_hc,
    name_key: "settings.accessibility.tier_dim_name",
    off: color::text::dim_off,
    target: "Lc 45",
    usage_key: "settings.accessibility.tier_dim_usage",
  },
];

#[derive(Clone, Copy, Debug)]
pub enum Message {
  HighContrastToggled(bool),
  LanguageChangeCanceled,
  LanguageChanged(Language),
  LanguageRestartConfirmed,
  ScaleChanged(u8),
}

#[derive(Debug, Default)]
pub struct State {
  pending_language: Option<Language>,
}

impl State {
  pub fn from_settings(_settings: &Settings) -> Self {
    State::default()
  }
}

#[derive(Clone, Copy, Debug)]
struct Preset {
  label_key: &'static str,
  pct: u8,
}

#[derive(Clone, Copy)]
struct Tier {
  hc: fn() -> iced::Color,
  name_key: &'static str,
  off: fn() -> iced::Color,
  target: &'static str,
  usage_key: &'static str,
}

fn clamp_scale(scale: u8) -> u8 {
  scale.clamp(SCALE_MIN, SCALE_MAX)
}

fn preset_for(scale: u8) -> Option<Preset> {
  PRESETS.into_iter().find(|preset| preset.pct == scale)
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::HighContrastToggled(enabled) => {
      settings.accessibility_mut().set_high_contrast(enabled);
      Outcome::AccessibilityChanged
    }
    Message::LanguageChangeCanceled => {
      state.pending_language = None;
      Outcome::None
    }
    Message::LanguageChanged(language) => {
      state.pending_language = (language != settings.accessibility().language()).then_some(language);
      Outcome::None
    }
    // The restart-gated apply (ADR-0041): persist the pending language and emit the outcome the app
    // consumes to re-seed and relaunch. Nothing to do without a pending selection.
    Message::LanguageRestartConfirmed => match state.pending_language.take() {
      Some(language) => {
        settings.accessibility_mut().set_language(language);
        Outcome::LanguageChanged(language)
      }
      None => Outcome::None,
    },
    Message::ScaleChanged(scale) => {
      settings.accessibility_mut().set_scale(clamp_scale(scale));
      Outcome::AccessibilityChanged
    }
  }
}

pub fn badge(settings: &Settings) -> String {
  let scale = clamp_scale(*settings.accessibility().scale());
  let is_preset = preset_for(scale).is_some();
  let high_contrast = *settings.accessibility().high_contrast();
  let key = match (is_preset, high_contrast) {
    (true, false) => "settings.accessibility.badge_preset",
    (true, true) => "settings.accessibility.badge_preset_hc",
    (false, false) => "settings.accessibility.badge_custom",
    (false, true) => "settings.accessibility.badge_custom_hc",
  };
  t!(key, scale => scale).into_owned()
}

pub fn view<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header();
  let body = panel_body(state, settings);

  Column::with_children(vec![header, body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header<'a>() -> Element<'a, Message> {
  let title = text(t!("settings.accessibility.title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(t!("settings.accessibility.blurb"))
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

fn panel_body(state: &State, settings: &Settings) -> Element<'static, Message> {
  let scale = clamp_scale(*settings.accessibility().scale());
  let high_contrast = *settings.accessibility().high_contrast();
  let running_language = settings.accessibility().language();
  let selected_language = state.pending_language.unwrap_or(running_language);

  let language_head = language_section_head(state.pending_language, selected_language);
  let language_grid = language_grid(selected_language);
  let language_footer = match state.pending_language {
    Some(pending) => language_confirm_row(pending),
    None => language_resting_note(running_language),
  };

  let section = section_head(
    super::i18n::tr_static("settings.accessibility.scale_section_label"),
    super::i18n::tr_static("settings.accessibility.scale_section_note"),
    super::i18n::tr_static("settings.accessibility.scale_section_chip"),
  );
  let presets = scale_presets(scale);
  let readout = scale_readout(scale);
  let fine = fine_scale(scale);

  let contrast_head = section_head(
    super::i18n::tr_static("settings.accessibility.contrast_section_label"),
    super::i18n::tr_static("settings.accessibility.contrast_section_note"),
    if high_contrast {
      super::i18n::tr_static("settings.accessibility.contrast_chip_on")
    } else {
      super::i18n::tr_static("settings.accessibility.contrast_chip_off")
    },
  );
  let contrast_toggle = contrast_toggle_row(high_contrast);
  let contrast_preview = contrast_preview(high_contrast);

  let inner = container(
    Column::with_children(vec![
      language_head,
      language_grid,
      language_footer,
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

fn live_chip(label: &str) -> Element<'static, Message> {
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

fn language_section_head(pending: Option<Language>, selected: Language) -> Element<'static, Message> {
  let micro = text(super::i18n::tr_static("settings.accessibility.language_section_label"))
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent::PLASMA));
  let detail = text(super::i18n::tr_static("settings.accessibility.language_section_note"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![micro.into(), detail.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let right = if pending.is_some() {
    restart_chip()
  } else {
    live_chip(&format!("{} \u{00b7} {}", selected.native_label(), selected.esi_code()))
  };
  let row = Row::with_children(vec![identity.into(), right])
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

fn restart_chip() -> Element<'static, Message> {
  let dot = container(Space::new())
    .width(Length::Fixed(6.0))
    .height(Length::Fixed(6.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::status::WARNING)),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let label = text(super::i18n::tr_static("settings.accessibility.language_section_chip"))
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS)
    .style(typography::colored(color::status::WARNING));

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
      background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.07))),
      border: Border {
        color: color::with_alpha(color::status::WARNING, 0.34),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

// A three-column grid of selectable language cards (one per ESI language). Iced has no grid widget,
// so the nine languages are chunked into rows of three, the trailing row padded with spacers to keep
// the columns aligned. Mirrors the wizard's language step for a consistent card idiom.
fn language_grid(selected: Language) -> Element<'static, Message> {
  let rows: Vec<Element<'static, Message>> = Language::ALL
    .chunks(LANGUAGE_GRID_COLUMNS)
    .map(|chunk| {
      let mut cells: Vec<Element<'static, Message>> = chunk
        .iter()
        .map(|&language| language_card(language, language == selected))
        .collect();
      while cells.len() < LANGUAGE_GRID_COLUMNS {
        cells.push(Space::new().width(Length::Fill).into());
      }
      Row::with_children(cells)
        .spacing(spacing::SPACE_3)
        .width(Length::Fill)
        .into()
    })
    .collect();

  Column::with_children(rows)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn language_card(language: Language, selected: bool) -> Element<'static, Message> {
  let native_color = if selected {
    color::text::PRIMARY
  } else {
    color::with_alpha(color::text::PRIMARY, 0.86)
  };
  let native = text(language.native_label())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(native_color));
  let code = language_code_tag(language.esi_code(), selected);
  let top = Row::with_children(vec![native.into(), Space::new().width(Length::Fill).into(), code])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  let label = text(language.label())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let mut bottom_children: Vec<Element<'static, Message>> = vec![label.into(), Space::new().width(Length::Fill).into()];
  if selected {
    bottom_children.push(language_check());
  }
  let bottom = Row::with_children(bottom_children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  let content = Column::with_children(vec![top.into(), bottom.into()])
    .spacing(spacing::UNIT + 1.0)
    .width(Length::Fill);

  let cell = container(content).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3_5 - 1.0,
    left: spacing::SPACE_3_5,
  });

  let (background, border) = if selected {
    (color::with_alpha(color::accent::PLASMA, 0.1), color::accent::PLASMA)
  } else {
    (color::surface::SUNKEN, color::rule())
  };
  let shadow = if selected {
    iced::Shadow {
      color: color::with_alpha(color::accent::PLASMA, 0.12),
      offset: iced::Vector::ZERO,
      blur_radius: 3.0,
    }
  } else {
    iced::Shadow::default()
  };

  button(cell)
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::LanguageChanged(language))
    .style(move |_, status| {
      let border_color = match (selected, status) {
        (true, _) => border,
        (false, button::Status::Hovered) => color::rule_strong(),
        (false, _) => border,
      };
      button::Style {
        background: Some(Background::Color(background)),
        border: Border {
          color: border_color,
          width: 1.0,
          radius: radius::NAV_CARD.into(),
        },
        text_color: color::text::PRIMARY,
        shadow,
        ..button::Style::default()
      }
    })
    .into()
}

fn language_check() -> Element<'static, Message> {
  container(
    Icon::check()
      .size(10.0)
      .color(color::on_fill(color::accent::PLASMA))
      .render(),
  )
  .width(Length::Fixed(16.0))
  .height(Length::Fixed(16.0))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::accent::PLASMA)),
    border: Border {
      radius: 8.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn language_code_tag(code: &'static str, selected: bool) -> Element<'static, Message> {
  let (text_color, border_color) = if selected {
    (color::accent::PLASMA, color::with_alpha(color::accent::PLASMA, 0.28))
  } else {
    (color::text::tertiary(), color::rule())
  };
  let background = if selected {
    color::with_alpha(color::accent::PLASMA, 0.1)
  } else {
    color::with_alpha(color::text::PRIMARY, 0.04)
  };
  let label = text(code)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS)
    .style(typography::colored(text_color));

  container(label)
    .padding(Padding {
      top: 2.0,
      right: spacing::SPACE_2 - 2.0,
      bottom: 2.0,
      left: spacing::SPACE_2 - 2.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn language_resting_note(running: Language) -> Element<'static, Message> {
  let icon = Icon::market().size(15.0).color(color::text::secondary()).render();
  let note = text(
    t!(
      "settings.accessibility.language_resting_note",
      code => running.esi_code()
    )
    .into_owned(),
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));

  let row = Row::with_children(vec![icon, note.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Top);

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
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn language_confirm_row(pending: Language) -> Element<'static, Message> {
  let marker = Icon::clock().size(16.0).color(color::status::WARNING).render();
  let heading = text(
    t!(
      "settings.accessibility.language_dirty_heading",
      native => pending.native_label()
    )
    .into_owned(),
  )
  .font(typography::body::MEDIUM)
  .size(typography::size::SM)
  .style(typography::colored(color::text::PRIMARY));
  let blurb = text(
    t!(
      "settings.accessibility.language_dirty_blurb",
      code => pending.esi_code()
    )
    .into_owned(),
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![heading.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let actions = Row::with_children(vec![language_cancel_button(), language_apply_button()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5);

  let row = Row::with_children(vec![marker, identity.into(), actions.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5)
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
      background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.06))),
      border: Border {
        color: color::with_alpha(color::status::WARNING, 0.32),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn language_apply_button() -> Element<'static, Message> {
  Button::primary(super::i18n::tr_static("settings.accessibility.language_apply"))
    .size(Size::Sm)
    .on_press(Message::LanguageRestartConfirmed)
    .into()
}

fn language_cancel_button() -> Element<'static, Message> {
  Button::secondary(super::i18n::tr_static("settings.accessibility.language_cancel"))
    .size(Size::Sm)
    .on_press(Message::LanguageChangeCanceled)
    .into()
}

fn contrast_toggle_row(high_contrast: bool) -> Element<'static, Message> {
  let heading = text(t!("settings.accessibility.high_contrast_heading").into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let title = Row::with_children(vec![heading.into(), apca_tag()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5);
  let blurb = text(t!("settings.accessibility.high_contrast_blurb").into_owned())
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
  let label = text(t!("settings.accessibility.apca_tag").into_owned())
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
    text(t!("settings.accessibility.preview_col_tier").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fixed(TIER_COLUMN_WIDTH));

  let row = Row::with_children(vec![
    tier.into(),
    heading(
      super::i18n::tr_static("settings.accessibility.preview_col_today"),
      Horizontal::Left,
    )
    .into(),
    heading(
      super::i18n::tr_static("settings.accessibility.preview_col_high_contrast"),
      Horizontal::Left,
    )
    .into(),
    heading(
      super::i18n::tr_static("settings.accessibility.preview_col_apca"),
      Horizontal::Right,
    )
    .into(),
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
  let name = text(super::i18n::tr_static(tier.name_key))
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let usage = text(super::i18n::tr_static(tier.usage_key))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let label = container(Column::with_children(vec![name.into(), usage.into()]).spacing(spacing::UNIT))
    .width(Length::Fixed(TIER_COLUMN_WIDTH));

  let today = container(
    text(super::i18n::tr_static("settings.accessibility.preview_sample"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored((tier.off)())),
  )
  .width(Length::Fill);
  let hc = container(
    text(super::i18n::tr_static("settings.accessibility.preview_sample"))
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
  let caption = text(t!("settings.accessibility.preview_surface_edges").into_owned())
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

  let label = text(super::i18n::tr_static(preset.label_key))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(label_color));
  let pct = text(t!("settings.accessibility.percent", scale => preset.pct).into_owned())
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
      text(t!("settings.accessibility.preset_default").into_owned())
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
    Some(preset) => {
      t!("settings.accessibility.readout_preset", label => super::i18n::tr_static(preset.label_key)).into_owned()
    }
    None => t!("settings.accessibility.readout_custom").into_owned(),
  };

  let now = text(t!("settings.accessibility.readout_now").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::tertiary()));
  let value = text(t!("settings.accessibility.percent", scale => scale).into_owned())
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

  let heading = text(t!("settings.accessibility.fine_scale_heading").into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let hint = text(t!("settings.accessibility.fine_scale_hint").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![heading.into(), hint.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let value = text(t!("settings.accessibility.percent", scale => scale).into_owned())
    .font(typography::mono::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let caption_color = if preset.is_some() {
    color::accent::PLASMA
  } else {
    color::status::WARNING
  };
  let caption = text(match preset {
    Some(preset) => {
      t!("settings.accessibility.readout_preset", label => super::i18n::tr_static(preset.label_key)).into_owned()
    }
    None => t!("settings.accessibility.fine_scale_custom").into_owned(),
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
  let label = text(super::i18n::tr_static(preset.label_key))
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
    fn it_appends_an_hc_suffix_when_high_contrast_is_on() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let mut settings = Settings::default();
      settings.accessibility_mut().set_high_contrast(true);

      assert_eq!(badge(&settings), "100% \u{00b7} HC");
    }

    #[test]
    fn it_appends_the_hc_suffix_after_a_custom_scale() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(112);
      settings.accessibility_mut().set_high_contrast(true);

      assert_eq!(badge(&settings), "112% \u{00b7} custom \u{00b7} HC");
    }

    #[test]
    fn it_marks_a_non_preset_value_as_custom() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(112);

      assert_eq!(badge(&settings), "112% \u{00b7} custom");
    }

    #[test]
    fn it_reports_a_bare_percentage_for_a_preset_value() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(125);

      assert_eq!(badge(&settings), "125%");
    }

    #[test]
    fn it_reports_the_default_scale_as_a_preset() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);

      assert_eq!(badge(&Settings::default()), "100%");
    }
  }

  mod preset_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_matches_each_preset_percentage() {
      for preset in PRESETS {
        assert_eq!(
          super::preset_for(preset.pct).map(|p| p.label_key),
          Some(preset.label_key)
        );
      }
    }

    #[test]
    fn it_returns_none_for_an_off_preset_value() {
      assert!(super::preset_for(112).is_none());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn dragging_to_a_custom_value_sets_the_same_scale_the_badge_reads() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::ScaleChanged(112), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert_eq!(*settings.accessibility().scale(), 112);
      assert_eq!(badge(&settings), "112% \u{00b7} custom");
    }

    #[test]
    fn it_clamps_a_scale_above_the_maximum() {
      let mut state = State::default();
      let mut settings = Settings::default();

      update(&mut state, Message::ScaleChanged(240), &mut settings);

      assert_eq!(*settings.accessibility().scale(), SCALE_MAX);
    }

    #[test]
    fn it_clamps_a_scale_below_the_minimum() {
      let mut state = State::default();
      let mut settings = Settings::default();

      update(&mut state, Message::ScaleChanged(10), &mut settings);

      assert_eq!(*settings.accessibility().scale(), SCALE_MIN);
    }

    #[test]
    fn selecting_a_preset_sets_the_scale_and_signals_a_live_change() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::ScaleChanged(125), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert_eq!(*settings.accessibility().scale(), 125);
    }

    #[test]
    fn toggling_high_contrast_off_clears_the_flag_and_signals_a_live_change() {
      let mut state = State::default();
      let mut settings = Settings::default();
      settings.accessibility_mut().set_high_contrast(true);

      let outcome = update(&mut state, Message::HighContrastToggled(false), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert!(!*settings.accessibility().high_contrast());
    }

    #[test]
    fn toggling_high_contrast_on_flips_the_flag_and_signals_a_live_change() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::HighContrastToggled(true), &mut settings);

      assert_eq!(outcome, Outcome::AccessibilityChanged);
      assert!(*settings.accessibility().high_contrast());
    }

    #[test]
    fn confirming_a_restart_persists_the_pending_language_and_signals_the_change() {
      let mut state = State::default();
      let mut settings = Settings::default();
      update(&mut state, Message::LanguageChanged(Language::De), &mut settings);

      let outcome = update(&mut state, Message::LanguageRestartConfirmed, &mut settings);

      assert_eq!(outcome, Outcome::LanguageChanged(Language::De));
      assert_eq!(settings.accessibility().language(), Language::De);
      assert!(state.pending_language.is_none());
    }

    #[test]
    fn canceling_a_pending_switch_reverts_to_the_running_language_without_persisting() {
      let mut state = State::default();
      let mut settings = Settings::default();
      let running = settings.accessibility().language();
      update(&mut state, Message::LanguageChanged(Language::De), &mut settings);

      let outcome = update(&mut state, Message::LanguageChangeCanceled, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.pending_language.is_none());
      assert_eq!(settings.accessibility().language(), running);
    }

    #[test]
    fn canceling_without_a_pending_switch_is_a_no_op() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::LanguageChangeCanceled, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.pending_language.is_none());
    }

    #[test]
    fn confirming_a_restart_without_a_pending_language_is_a_no_op() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::LanguageRestartConfirmed, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(settings.accessibility().language(), Language::default());
    }

    #[test]
    fn picking_a_different_language_parks_it_pending_without_persisting() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::LanguageChanged(Language::Ja), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(state.pending_language, Some(Language::Ja));
      assert_eq!(settings.accessibility().language(), Language::default());
    }

    #[test]
    fn picking_the_running_language_is_a_no_op() {
      let mut state = State::default();
      let mut settings = Settings::default();
      let running = settings.accessibility().language();

      let outcome = update(&mut state, Message::LanguageChanged(running), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.pending_language.is_none());
    }

    #[test]
    fn re_picking_the_running_language_clears_a_prior_pending_selection() {
      let mut state = State::default();
      let mut settings = Settings::default();
      let running = settings.accessibility().language();
      update(&mut state, Message::LanguageChanged(Language::Fr), &mut settings);

      update(&mut state, Message::LanguageChanged(running), &mut settings);

      assert!(state.pending_language.is_none());
    }
  }

  mod language_grid {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_offers_a_card_for_every_one_of_the_nine_languages() {
      let offered: HashSet<&'static str> = Language::ALL.iter().map(|language| language.native_label()).collect();

      assert_eq!(offered.len(), 9);
      assert_eq!(Language::ALL.len(), 9);
    }

    #[test]
    fn it_builds_the_grid_for_a_selected_language() {
      let _el: Element<'static, Message> = language_grid(Language::Ja);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_accessibility_panel() {
      let settings = Settings::default();
      let state = State::default();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_contrast_preview_with_high_contrast_on() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_high_contrast(true);
      let state = State::default();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_with_a_custom_scale() {
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(112);
      let state = State::default();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_restart_confirmation_when_a_language_is_pending() {
      let mut settings = Settings::default();
      let mut state = State::default();
      update(&mut state, Message::LanguageChanged(Language::De), &mut settings);

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
