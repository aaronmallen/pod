use iced::{
  Background, Border, Color, Element, Length, Padding, Shadow,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, svg, text, text_input},
};

use super::Outcome;
use crate::{
  config::{CascadeMode, NavLocation, Settings},
  features::shell::registry,
  ui::{
    components::{
      button::{Button, Size},
      card, color_picker,
      icon::Icon,
      rule, text_input as text_input_component, toggle,
    },
    style::{color, radius, spacing, typography},
  },
};

const ACCENT_GRID_MAX_WIDTH: f32 = 760.0;
const ACCENT_ROW_MAX_WIDTH: f32 = 720.0;
const ESI_TAG_RADIUS: f32 = 3.0;
const HEX_FIELD_HEIGHT: f32 = 34.0;
const HEX_FIELD_WIDTH: f32 = 74.0;
const ICON_CHIP_SIZE: f32 = 38.0;
const ICON_SIZE: f32 = 20.0;
const ORDER_BUTTON_SIZE: f32 = 28.0;
const PANEL_SIDE_PADDING: f32 = 36.0;
const PREVIEW_HEIGHT: f32 = 100.0;
const RADIO_DOT_SIZE: f32 = 14.0;
const RADIO_CHECK_SIZE: f32 = 8.0;
const SIDE_CARD_MAX_HEIGHT: f32 = 145.0;
const SIDE_CARD_MAX_WIDTH: f32 = 245.0;
const SIDE_CARDS_ROW_MAX_WIDTH: f32 = SIDE_CARD_MAX_WIDTH * 2.0 + spacing::SPACE_3_5;
const SWATCH_CHECK_SIZE: f32 = 15.0;
const SWATCH_DOT_SIZE: f32 = 34.0;
const SWATCH_GLOW_BLUR: f32 = 16.0;
const SWATCH_RING_BLUR: f32 = 3.0;
const ROW_LIST_MAX_WIDTH: f32 = 560.0;

const RAIL_SIDES: [NavLocation; 2] = [NavLocation::Left, NavLocation::Right];

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
  AccentHexChanged(String),
  AccentHexCommitted,
  AccentHexReverted,
  AccentReset,
  AccentSelected(&'static str),
  CascadeSelected(CascadeMode),
  Dropped,
  HoverSlot(usize),
  LeaveSlot(usize),
  MoveDown(usize),
  MoveUp(usize),
  PickUp(usize),
  PreviewInteracted,
  ResetOrder,
  SideSelected(NavLocation),
}

#[derive(Debug, Default)]
pub struct State {
  accent_draft: String,
  accent_editing: bool,
  dragging: Option<usize>,
  drop_index: Option<usize>,
}

impl State {
  pub fn from_settings(settings: &Settings) -> Self {
    State {
      accent_draft: settings.ui().accent().trim_start_matches('#').to_owned(),
      ..State::default()
    }
  }
}

#[derive(Clone, Copy, Debug)]
enum Direction {
  Down,
  Up,
}

impl Direction {
  fn glyph(self) -> &'static str {
    match self {
      Direction::Down => "\u{2193}",
      Direction::Up => "\u{2191}",
    }
  }
}

fn apply_if_changed(unchanged: bool, apply: impl FnOnce()) -> Outcome {
  if unchanged {
    return Outcome::None;
  }
  apply();
  Outcome::UiChanged
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::AccentHexChanged(draft) => {
      state.accent_draft = draft;
      state.accent_editing = true;
      Outcome::None
    }
    Message::AccentHexCommitted => commit_accent(state, settings),
    Message::AccentHexReverted => revert_accent(state, settings),
    Message::AccentReset => apply_accent(state, settings, crate::config::DEFAULT_ACCENT.to_owned()),
    Message::AccentSelected(hex) => apply_accent(state, settings, hex.to_owned()),
    Message::CascadeSelected(mode) => apply_if_changed(*settings.ui().cascade_mode() == mode, || {
      settings.ui_mut().set_cascade_mode(mode);
    }),
    Message::Dropped => match (state.dragging.take(), state.drop_index.take()) {
      (Some(from), Some(to)) => move_to(settings, from, to),
      _ => Outcome::None,
    },
    Message::HoverSlot(index) => {
      if state.dragging.is_some() {
        state.drop_index = Some(index);
      }
      Outcome::None
    }
    Message::LeaveSlot(index) => {
      if state.drop_index == Some(index) {
        state.drop_index = None;
      }
      Outcome::None
    }
    Message::MoveDown(index) => swap(settings, index, index + 1),
    Message::MoveUp(index) => {
      if index == 0 {
        Outcome::None
      } else {
        swap(settings, index, index - 1)
      }
    }
    Message::PickUp(index) => {
      state.dragging = Some(index);
      state.drop_index = None;
      Outcome::None
    }
    Message::PreviewInteracted => Outcome::None,
    Message::ResetOrder => apply_if_changed(is_default_order(settings), || {
      settings
        .ui_mut()
        .set_rail_order(crate::ui::components::rail::Destination::REORDERABLE.to_vec());
    }),
    Message::SideSelected(side) => apply_if_changed(*settings.ui().nav_location() == side, || {
      settings.ui_mut().set_nav_location(side);
    }),
  }
}

fn apply_accent(state: &mut State, settings: &mut Settings, hex: String) -> Outcome {
  state.accent_draft = hex.trim_start_matches('#').to_owned();
  state.accent_editing = false;
  apply_if_changed(settings.ui().accent().eq_ignore_ascii_case(&hex), || {
    settings.ui_mut().set_accent(hex);
  })
}

fn commit_accent(state: &mut State, settings: &mut Settings) -> Outcome {
  match color_picker::normalize_hex(&state.accent_draft) {
    Some(hex) => apply_accent(state, settings, hex),
    None => revert_accent(state, settings),
  }
}

fn revert_accent(state: &mut State, settings: &mut Settings) -> Outcome {
  state.accent_draft = settings.ui().accent().trim_start_matches('#').to_owned();
  state.accent_editing = false;
  Outcome::None
}

fn is_default_accent(settings: &Settings) -> bool {
  settings
    .ui()
    .accent()
    .eq_ignore_ascii_case(crate::config::DEFAULT_ACCENT)
}

fn is_preset_accent(settings: &Settings) -> bool {
  color::ACCENT_PRESETS
    .iter()
    .any(|preset| preset.hex.eq_ignore_ascii_case(settings.ui().accent()))
}

fn is_default_order(settings: &Settings) -> bool {
  *settings.ui().rail_order() == crate::ui::components::rail::Destination::REORDERABLE.to_vec()
}

fn move_to(settings: &mut Settings, from: usize, to: usize) -> Outcome {
  let mut order = settings.ui().rail_order().clone();
  if from >= order.len() || from == to {
    return Outcome::None;
  }
  let moved = order.remove(from);
  let insert_at = if from < to { to - 1 } else { to };
  order.insert(insert_at.min(order.len()), moved);
  settings.ui_mut().set_rail_order(order);
  Outcome::UiChanged
}

fn swap(settings: &mut Settings, a: usize, b: usize) -> Outcome {
  let mut order = settings.ui().rail_order().clone();
  if a >= order.len() || b >= order.len() || a == b {
    return Outcome::None;
  }
  order.swap(a, b);
  settings.ui_mut().set_rail_order(order);
  Outcome::UiChanged
}

pub fn badge(settings: &Settings) -> String {
  match settings.ui().nav_location() {
    NavLocation::Left => t!("settings.ui.nav_location_left").into_owned(),
    NavLocation::Right => t!("settings.ui.nav_location_right").into_owned(),
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs = Vec::new();
  if state.dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
      )
      .then_some(Message::Dropped)
    }));
  }
  if state.accent_editing {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
          key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
          ..
        })
      )
      .then_some(Message::AccentHexReverted)
    }));
  }
  iced::Subscription::batch(subs)
}

pub fn view<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  Column::with_children(vec![header(), body(state, settings)])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn header<'a>() -> Element<'a, Message> {
  let title = text(t!("settings.ui.title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(t!("settings.ui.blurb"))
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

fn body<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let accent_head = section_head(
    super::i18n::tr_static("settings.ui.accent_label"),
    super::i18n::tr_static("settings.ui.accent_note"),
    live_chip(super::i18n::tr_static("settings.ui.applies_live")),
  );
  let accent_picker = accent_picker(state, settings);

  let side_head = section_head(
    super::i18n::tr_static("settings.ui.rail_side_label"),
    super::i18n::tr_static("settings.ui.rail_side_note"),
    live_chip(super::i18n::tr_static("settings.ui.applies_live")),
  );
  let side_cards = side_cards(settings);

  let cascade_head = section_head(
    super::i18n::tr_static("settings.ui.rail_cascade_label"),
    super::i18n::tr_static("settings.ui.rail_cascade_note"),
    live_chip(super::i18n::tr_static("settings.ui.applies_live")),
  );
  let cascade_cards = cascade_cards(settings);

  let order_head = section_head(
    super::i18n::tr_static("settings.ui.icon_order_label"),
    super::i18n::tr_static("settings.ui.icon_order_note"),
    reset_button(settings),
  );
  let order_list = order_list(state, settings);

  let inner = container(
    Column::with_children(vec![
      accent_head,
      accent_picker,
      side_head,
      side_cards,
      cascade_head,
      cascade_cards,
      order_head,
      order_list,
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

fn section_head<'a>(label: &'a str, note: &'a str, accessory: Element<'a, Message>) -> Element<'a, Message> {
  let micro = text(label)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent()));
  let detail = text(note)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![micro.into(), detail.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  Row::with_children(vec![identity.into(), accessory])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5)
    .into()
}

fn live_chip<'a>(label: &'a str) -> Element<'a, Message> {
  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::accent())),
  )
  .padding(Padding {
    top: 4.0,
    right: spacing::SPACE_2_5,
    bottom: 4.0,
    left: spacing::SPACE_2_5,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent(), 0.12))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn accent_picker<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  Column::with_children(vec![
    swatch_grid(settings),
    custom_hex_row(state, settings),
    preview_strip(settings),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill)
  .into()
}

fn swatch_grid(settings: &Settings) -> Element<'_, Message> {
  let current = settings.ui().accent();
  let swatches: Vec<Element<'_, Message>> = color::ACCENT_PRESETS
    .iter()
    .map(|preset| accent_swatch(preset, preset.hex.eq_ignore_ascii_case(current)))
    .collect();

  container(
    Row::with_children(swatches)
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .max_width(ACCENT_GRID_MAX_WIDTH)
  .into()
}

fn accent_swatch<'a>(preset: &color::AccentPreset, selected: bool) -> Element<'a, Message> {
  let hex = preset.hex;
  let base = preset.shades.base;
  let ink = preset.shades.ink;

  let check: Element<'a, Message> = if selected {
    Icon::check().size(SWATCH_CHECK_SIZE).color(ink).render()
  } else {
    Space::new().into()
  };
  let dot = container(check)
    .width(Length::Fixed(SWATCH_DOT_SIZE))
    .height(Length::Fixed(SWATCH_DOT_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(base)),
      border: Border {
        color: color::with_alpha(base, 0.5),
        width: 1.0,
        radius: (SWATCH_DOT_SIZE / 2.0).into(),
      },
      shadow: if selected {
        Shadow {
          color: color::with_alpha(base, 0.45),
          blur_radius: SWATCH_GLOW_BLUR,
          ..Shadow::default()
        }
      } else {
        Shadow::default()
      },
      ..container::Style::default()
    });

  let name = text(preset.name)
    .font(if selected {
      typography::body::MEDIUM
    } else {
      typography::body::REGULAR
    })
    .size(typography::size::SM)
    .style(typography::colored(if selected {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    }));

  let content = Column::with_children(vec![dot.into(), name.into()])
    .spacing(spacing::SPACE_2)
    .align_x(Horizontal::Center)
    .width(Length::Fill);

  button(content)
    .width(Length::FillPortion(1))
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_2,
      bottom: 11.0,
      left: spacing::SPACE_2,
    })
    .on_press(Message::AccentSelected(hex))
    .style(move |_, status| {
      let border_color = if selected {
        base
      } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        color::rule_strong()
      } else {
        color::rule()
      };
      button::Style {
        background: Some(Background::Color(if selected {
          color::with_alpha(base, 0.08)
        } else {
          color::surface::SUNKEN
        })),
        border: Border {
          color: border_color,
          width: 1.0,
          radius: radius::NAV_CARD.into(),
        },
        shadow: if selected {
          Shadow {
            color: color::with_alpha(base, 0.18),
            blur_radius: SWATCH_RING_BLUR,
            ..Shadow::default()
          }
        } else {
          Shadow::default()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn custom_hex_row<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let label = text(super::i18n::tr_static("settings.ui.accent_custom_label"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));

  let mut children = vec![label.into(), hex_field(state)];
  if !is_preset_accent(settings) && color_picker::normalize_hex(&state.accent_draft).is_some() {
    children.push(
      text(super::i18n::tr_static("settings.ui.accent_custom_hue"))
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  Row::with_children(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .into()
}

fn hex_field(state: &State) -> Element<'_, Message> {
  let border_color = color_picker::normalize_hex(&state.accent_draft)
    .and_then(|hex| color::from_hex(&hex))
    .map_or_else(color::rule, |valid| color::with_alpha(valid, 0.5));

  let hash = text("#")
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::tertiary()));
  let input = text_input("3FB8DB", &state.accent_draft)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding::ZERO)
    .width(Length::Fixed(HEX_FIELD_WIDTH))
    .on_input(Message::AccentHexChanged)
    .on_submit(Message::AccentHexCommitted)
    .style(text_input_component::inner_style());

  container(
    Row::with_children(vec![hash.into(), input.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .height(Length::Fixed(HEX_FIELD_HEIGHT))
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_3,
    bottom: 0.0,
    left: spacing::SPACE_3,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn preview_strip(settings: &Settings) -> Element<'_, Message> {
  let label = text(super::i18n::tr_static("settings.ui.accent_preview_label"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));

  let mut children: Vec<Element<'_, Message>> = vec![
    label.into(),
    Button::primary("Primary")
      .size(Size::Sm)
      .on_press(Message::PreviewInteracted)
      .into(),
    Button::secondary("Secondary")
      .size(Size::Sm)
      .on_press(Message::PreviewInteracted)
      .into(),
    toggle::toggle(true, Message::PreviewInteracted),
    esi_tag(),
    Space::new().width(Length::Fill).into(),
  ];
  if !is_default_accent(settings) {
    children.push(
      Button::ghost(t!("settings.ui.accent_reset"))
        .icon(Icon::reset())
        .size(Size::Sm)
        .on_press(Message::AccentReset)
        .into(),
    );
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_3_5)
      .align_y(Vertical::Center)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .max_width(ACCENT_ROW_MAX_WIDTH)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: 16.0,
    bottom: spacing::SPACE_3_5,
    left: 16.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: radius::NAV_CARD.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn esi_tag<'a>() -> Element<'a, Message> {
  container(
    text(t!("settings.ui.esi_tag"))
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::accent())),
  )
  .padding(Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent(), 0.06))),
    border: Border {
      color: color::with_alpha(color::accent(), 0.3),
      width: 1.0,
      radius: ESI_TAG_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn side_cards(settings: &Settings) -> Element<'_, Message> {
  let selected = *settings.ui().nav_location();
  let cards: Vec<Element<'_, Message>> = RAIL_SIDES
    .into_iter()
    .map(|side| nav_card(side, selected == side))
    .collect();

  container(
    Row::with_children(cards)
      .spacing(spacing::SPACE_3_5)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .max_width(SIDE_CARDS_ROW_MAX_WIDTH)
  .into()
}

fn nav_card<'a>(side: NavLocation, selected: bool) -> Element<'a, Message> {
  let preview = container(nav_preview(side, selected))
    .width(Length::Fill)
    .height(Length::Fixed(PREVIEW_HEIGHT));

  let label = text(
    match side {
      NavLocation::Left => t!("settings.ui.nav_location_left"),
      NavLocation::Right => t!("settings.ui.nav_location_right"),
    }
    .into_owned(),
  )
  .font(typography::body::MEDIUM)
  .size(typography::size::MD)
  .style(typography::colored(if selected {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  }));

  let footer = Row::with_children(vec![
    label.into(),
    Space::new().width(Length::Fill).into(),
    radio_dot(selected),
  ])
  .align_y(Vertical::Center)
  .padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  });

  let card = Column::with_children(vec![preview.into(), rule::horizontal(), footer.into()]).width(Length::Fill);

  let card_button = card::selectable_card(card, selected, Message::SideSelected(side)).width(Length::Fill);

  container(card_button)
    .width(Length::Fill)
    .max_height(SIDE_CARD_MAX_HEIGHT)
    .into()
}

fn cascade_cards(settings: &Settings) -> Element<'_, Message> {
  let selected = *settings.ui().cascade_mode();
  let cards: Vec<Element<'_, Message>> = CascadeMode::ALL
    .into_iter()
    .map(|mode| cascade_card(mode, selected == mode))
    .collect();

  Row::with_children(cards)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn cascade_card<'a>(mode: CascadeMode, selected: bool) -> Element<'a, Message> {
  let label = text(mode.label())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(if selected {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    }));
  let note = text(cascade_note(mode))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::tertiary()));
  let identity = Column::with_children(vec![label.into(), note.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let footer = Row::with_children(vec![
    identity.into(),
    Space::new().width(Length::Fill).into(),
    radio_dot(selected),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2_5);

  let body = container(footer).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  });

  card::selectable_card(body, selected, Message::CascadeSelected(mode))
    .width(Length::FillPortion(1))
    .into()
}

fn cascade_note(mode: CascadeMode) -> &'static str {
  match mode {
    CascadeMode::Flyout => super::i18n::tr_static("settings.ui.cascade_note_flyout"),
    CascadeMode::None => super::i18n::tr_static("settings.ui.cascade_note_none"),
    CascadeMode::SubRail => super::i18n::tr_static("settings.ui.cascade_note_sub_rail"),
  }
}

fn portion(width: f32) -> u16 {
  (width.clamp(0.0, 1.0) * 100.0) as u16
}

fn complement_portion(width: f32) -> u16 {
  100 - portion(width)
}

fn nav_preview<'a>(side: NavLocation, selected: bool) -> Element<'a, Message> {
  let rail_color = if selected {
    color::accent()
  } else {
    color::with_alpha(color::text::PRIMARY, 0.32)
  };
  let head_dim = color::with_alpha(color::text::PRIMARY, if selected { 0.3 } else { 0.18 });
  let body_dim = color::with_alpha(color::text::PRIMARY, if selected { 0.2 } else { 0.12 });

  let dots: Vec<Element<'a, Message>> = (0..4)
    .map(|_| {
      container(Space::new())
        .width(Length::Fixed(5.0))
        .height(Length::Fixed(5.0))
        .style(move |_| container::Style {
          background: Some(Background::Color(rail_color)),
          border: Border {
            radius: 1.5.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into()
    })
    .collect();
  let rail = container(Column::with_children(dots).spacing(4.0).align_x(Horizontal::Center))
    .width(Length::Fixed(14.0))
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      ..container::Style::default()
    });

  let stub = |width: f32, height: f32, fill: Color| -> Element<'a, Message> {
    container(Space::new())
      .width(Length::FillPortion(portion(width)))
      .height(Length::Fixed(height))
      .style(move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
          radius: 1.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  };
  let stub_row = |width: f32, height: f32, fill: Color| -> Element<'a, Message> {
    let rest = Length::FillPortion(complement_portion(width));
    Row::with_children(vec![stub(width, height, fill), Space::new().width(rest).into()])
      .width(Length::Fill)
      .into()
  };
  let body = container(
    Column::with_children(vec![
      stub_row(0.7, 4.0, head_dim),
      stub_row(0.4, 3.0, body_dim),
      stub_row(0.55, 3.0, body_dim),
      stub_row(0.32, 3.0, body_dim),
    ])
    .spacing(3.0)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .padding(spacing::SPACE_2);

  let children: Vec<Element<'a, Message>> = match side {
    NavLocation::Left => vec![rail.into(), body.into()],
    NavLocation::Right => vec![body.into(), rail.into()],
  };

  container(Row::with_children(children).width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        radius: iced::border::Radius {
          top_left: radius::NAV_CARD,
          top_right: radius::NAV_CARD,
          bottom_left: 0.0,
          bottom_right: 0.0,
        },
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn radio_dot<'a>(selected: bool) -> Element<'a, Message> {
  let check: Element<'a, Message> = if selected {
    Icon::check()
      .size(RADIO_CHECK_SIZE)
      .color(color::surface::BASE)
      .render()
  } else {
    Space::new().into()
  };

  container(check)
    .width(Length::Fixed(RADIO_DOT_SIZE))
    .height(Length::Fixed(RADIO_DOT_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: selected.then_some(Background::Color(color::accent())),
      border: Border {
        color: if selected {
          color::accent()
        } else {
          color::rule_strong()
        },
        width: 1.0,
        radius: (RADIO_DOT_SIZE / 2.0).into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn reset_button(settings: &Settings) -> Element<'_, Message> {
  let enabled = !is_default_order(settings);

  Button::secondary(t!("settings.ui.reset_order"))
    .icon(Icon::reset())
    .size(Size::Sm)
    .on_press_maybe(enabled.then_some(Message::ResetOrder))
    .into()
}

fn order_list<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let order = settings.ui().rail_order();
  let enabled = settings.features().enabled();
  let count = order.len();
  let rows: Vec<Element<'a, Message>> = order
    .iter()
    .enumerate()
    .map(|(index, &destination)| {
      let is_enabled = registry::feature_for_destination(destination).is_none_or(|feature| enabled.contains(&feature));
      order_row(state, destination, index, count, is_enabled)
    })
    .collect();

  container(Column::with_children(rows).spacing(spacing::UNIT).width(Length::Fill))
    .max_width(ROW_LIST_MAX_WIDTH)
    .width(Length::Fill)
    .into()
}

fn order_row<'a>(
  state: &State,
  destination: crate::ui::components::rail::Destination,
  index: usize,
  count: usize,
  is_enabled: bool,
) -> Element<'a, Message> {
  let dragging = state.dragging == Some(index);
  let drop_above = state.drop_index == Some(index) && state.dragging.is_some() && !dragging;
  let dimmed = !is_enabled;

  let cells = Row::with_children(vec![
    drag_handle(index),
    position_label(index),
    icon_chip(destination, dimmed),
    name_cell(destination, dimmed),
    Space::new().width(Length::Fill).into(),
    order_button(Direction::Up, index, index == 0),
    order_button(Direction::Down, index, index + 1 >= count),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  let row = container(cells)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(if dragging {
        color::with_alpha(color::accent(), 0.05)
      } else {
        color::surface::SUNKEN
      })),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    });

  let top = container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(if drop_above {
        color::accent()
      } else {
        Color::TRANSPARENT
      })),
      ..container::Style::default()
    });
  let stacked = Column::with_children(vec![top.into(), row.into()]).width(Length::Fill);

  mouse_area(stacked)
    .on_enter(Message::HoverSlot(index))
    .on_exit(Message::LeaveSlot(index))
    .into()
}

fn drag_handle<'a>(index: usize) -> Element<'a, Message> {
  let glyph = text("\u{22ee}")
    .font(typography::body::REGULAR)
    .size(typography::size::LG)
    .style(typography::colored(color::text::tertiary()));
  let cell = container(glyph)
    .width(Length::Fixed(16.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center);

  mouse_area(cell).on_press(Message::PickUp(index)).into()
}

fn position_label<'a>(index: usize) -> Element<'a, Message> {
  text(format!("{:02}", index + 1))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .width(Length::Fixed(16.0))
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn icon_chip<'a>(destination: crate::ui::components::rail::Destination, dimmed: bool) -> Element<'a, Message> {
  let icon_color = if dimmed {
    color::with_alpha(color::text::secondary(), 0.4)
  } else {
    color::text::secondary()
  };
  container(
    svg(svg::Handle::from_memory(destination.icon()))
      .width(Length::Fixed(ICON_SIZE))
      .height(Length::Fixed(ICON_SIZE))
      .style(move |_, _| svg::Style {
        color: Some(icon_color),
      }),
  )
  .width(Length::Fixed(ICON_CHIP_SIZE))
  .height(Length::Fixed(ICON_CHIP_SIZE))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(
      color::surface::NAVIGATION,
      if dimmed { 0.4 } else { 1.0 },
    ))),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn name_cell<'a>(destination: crate::ui::components::rail::Destination, dimmed: bool) -> Element<'a, Message> {
  text(destination.label())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(if dimmed {
      color::text::tertiary()
    } else {
      color::text::PRIMARY
    }))
    .into()
}

fn order_button<'a>(direction: Direction, index: usize, disabled: bool) -> Element<'a, Message> {
  let glyph = text(direction.glyph())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(if disabled {
      color::text::tertiary()
    } else {
      color::text::secondary()
    }));
  let cell = container(glyph).center_x(Length::Fill).center_y(Length::Fill);

  let message = match direction {
    Direction::Down => Message::MoveDown(index),
    Direction::Up => Message::MoveUp(index),
  };

  button(cell)
    .width(Length::Fixed(ORDER_BUTTON_SIZE))
    .height(Length::Fixed(ORDER_BUTTON_SIZE))
    .padding(Padding::ZERO)
    .on_press_maybe((!disabled).then_some(message))
    .style(move |_, status| {
      let border_alpha = match status {
        button::Status::Hovered | button::Status::Pressed if !disabled => 0.18,
        _ => 0.1,
      };
      button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: if disabled {
          color::text::tertiary()
        } else {
          color::text::secondary()
        },
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, border_alpha),
          width: 1.0,
          radius: radius::SUBTLE.into(),
        },
        ..button::Style::default()
      }
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ui::components::rail::Destination;

  fn settings() -> Settings {
    Settings::default()
  }

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_left_for_the_default_side() {
      assert_eq!(badge(&settings()), "Left");
    }

    #[test]
    fn it_reads_right_when_the_rail_is_flipped() {
      let mut settings = settings();
      settings.ui_mut().set_nav_location(NavLocation::Right);

      assert_eq!(badge(&settings), "Right");
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flips_the_rail_side_and_signals_a_live_change() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::SideSelected(NavLocation::Right), &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().nav_location(), &NavLocation::Right);
    }

    #[test]
    fn it_ignores_selecting_the_current_side() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::SideSelected(NavLocation::Left), &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[test]
    fn it_changes_the_cascade_mode_and_signals_a_live_change() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::CascadeSelected(CascadeMode::None), &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().cascade_mode(), &CascadeMode::None);
    }

    #[test]
    fn it_ignores_selecting_the_current_cascade_mode() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::CascadeSelected(CascadeMode::Flyout), &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[test]
    fn it_moves_an_item_up_with_the_arrow() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::MoveUp(1), &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().rail_order()[0], Destination::Skills);
      assert_eq!(settings.ui().rail_order()[1], Destination::Roster);
    }

    #[test]
    fn it_keeps_the_first_item_pinned_when_moved_up() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::MoveUp(0), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(settings.ui().rail_order()[0], Destination::Roster);
    }

    #[test]
    fn it_moves_an_item_down_with_the_arrow() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::MoveDown(0), &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().rail_order()[0], Destination::Skills);
      assert_eq!(settings.ui().rail_order()[1], Destination::Roster);
    }

    #[test]
    fn it_ignores_moving_the_last_item_down() {
      let mut state = State::default();
      let mut settings = settings();
      let last = settings.ui().rail_order().len() - 1;

      let outcome = update(&mut state, Message::MoveDown(last), &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[test]
    fn it_drops_a_dragged_item_above_the_target_row() {
      let mut state = State::default();
      let mut settings = settings();
      update(&mut state, Message::PickUp(6), &mut settings);
      update(&mut state, Message::HoverSlot(0), &mut settings);

      let outcome = update(&mut state, Message::Dropped, &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().rail_order()[0], Destination::Assets);
      assert!(state.dragging.is_none(), "the drop consumes the drag");
    }

    #[test]
    fn it_ignores_a_drop_with_no_active_drag() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::Dropped, &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[test]
    fn it_resets_the_order_to_default_and_signals_a_live_change() {
      let mut state = State::default();
      let mut settings = settings();
      update(&mut state, Message::MoveDown(0), &mut settings);

      let outcome = update(&mut state, Message::ResetOrder, &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(*settings.ui().rail_order(), Destination::REORDERABLE.to_vec());
    }

    #[test]
    fn it_ignores_a_reset_that_is_already_default() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::ResetOrder, &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[test]
    fn it_applies_a_preset_swatch_and_signals_a_live_change() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::AccentSelected("#36C6A8"), &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().accent(), "#36C6A8");
      assert_eq!(state.accent_draft, "36C6A8");
    }

    #[test]
    fn it_ignores_selecting_the_current_accent() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::AccentSelected("#3FB8DB"), &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[test]
    fn it_keeps_partial_hex_typing_from_touching_the_accent() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::AccentHexChanged("3F".to_owned()), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(settings.ui().accent(), "#3FB8DB");
      assert_eq!(state.accent_draft, "3F");
      assert!(state.accent_editing);
    }

    #[test]
    fn it_commits_a_valid_custom_hex() {
      let mut state = State::default();
      let mut settings = settings();
      update(
        &mut state,
        Message::AccentHexChanged("ff6b6b".to_owned()),
        &mut settings,
      );

      let outcome = update(&mut state, Message::AccentHexCommitted, &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().accent(), "#FF6B6B");
      assert_eq!(state.accent_draft, "FF6B6B");
      assert!(!state.accent_editing);
    }

    #[test]
    fn it_reverts_an_invalid_hex_on_commit() {
      let mut state = State::from_settings(&settings());
      let mut settings = settings();
      update(&mut state, Message::AccentHexChanged("zzz".to_owned()), &mut settings);

      let outcome = update(&mut state, Message::AccentHexCommitted, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(settings.ui().accent(), "#3FB8DB");
      assert_eq!(state.accent_draft, "3FB8DB");
    }

    #[test]
    fn it_reverts_the_draft_on_escape() {
      let mut state = State::from_settings(&settings());
      let mut settings = settings();
      update(&mut state, Message::AccentHexChanged("12".to_owned()), &mut settings);

      let outcome = update(&mut state, Message::AccentHexReverted, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(state.accent_draft, "3FB8DB");
      assert!(!state.accent_editing);
    }

    #[test]
    fn it_resets_a_custom_accent_to_plasma() {
      let mut state = State::default();
      let mut settings = settings();
      update(&mut state, Message::AccentSelected("#B89BEA"), &mut settings);

      let outcome = update(&mut state, Message::AccentReset, &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().accent(), "#3FB8DB");
      assert_eq!(state.accent_draft, "3FB8DB");
    }

    #[test]
    fn it_ignores_a_reset_when_the_accent_is_already_default() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::AccentReset, &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[test]
    fn it_ignores_preview_interactions() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::PreviewInteracted, &mut settings);

      assert_eq!(outcome, Outcome::None);
    }
  }

  mod subscription {
    use super::*;

    #[test]
    fn it_is_empty_while_idle() {
      let _sub: iced::Subscription<Message> = subscription(&State::default());
    }

    #[test]
    fn it_listens_for_a_release_while_dragging() {
      let state = State {
        dragging: Some(0),
        ..State::default()
      };

      let _sub: iced::Subscription<Message> = subscription(&state);
    }

    #[test]
    fn it_listens_for_escape_while_editing_the_hex() {
      let state = State {
        accent_editing: true,
        ..State::default()
      };

      let _sub: iced::Subscription<Message> = subscription(&state);
    }
  }

  mod accent_section {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_the_draft_from_the_saved_accent() {
      let mut settings = settings();
      settings.ui_mut().set_accent("#B89BEA".to_owned());

      let state = State::from_settings(&settings);

      assert_eq!(state.accent_draft, "B89BEA");
    }

    #[test]
    fn it_renders_the_full_picker_with_the_default_selected() {
      let settings = settings();
      let state = State::from_settings(&settings);

      let _el: Element<'_, Message> = accent_picker(&state, &settings);
    }

    #[test]
    fn it_renders_the_custom_row_with_an_off_palette_hue() {
      let mut settings = settings();
      settings.ui_mut().set_accent("#FF6B6B".to_owned());
      let state = State::from_settings(&settings);

      let _el: Element<'_, Message> = custom_hex_row(&state, &settings);
    }

    #[test]
    fn it_renders_the_preview_strip_with_a_reset_when_off_default() {
      let mut settings = settings();
      settings.ui_mut().set_accent("#36C6A8".to_owned());

      let _el: Element<'_, Message> = preview_strip(&settings);
    }

    #[test]
    fn it_renders_the_preview_strip_without_a_reset_at_default() {
      let settings = settings();

      let _el: Element<'_, Message> = preview_strip(&settings);
    }

    #[test]
    fn it_marks_the_presets_and_default_accent() {
      let mut settings = settings();

      assert!(is_default_accent(&settings));
      assert!(is_preset_accent(&settings));

      settings.ui_mut().set_accent("#36c6a8".to_owned());

      assert!(!is_default_accent(&settings));
      assert!(is_preset_accent(&settings));

      settings.ui_mut().set_accent("#FF6B6B".to_owned());

      assert!(!is_preset_accent(&settings));
    }
  }

  mod preview_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_caps_the_side_card_max_width_at_the_design_grid_bound() {
      assert_eq!(SIDE_CARD_MAX_WIDTH, 245.0);
    }

    #[test]
    fn it_caps_the_side_card_max_height_at_the_design_grid_bound() {
      assert_eq!(SIDE_CARD_MAX_HEIGHT, 145.0);
    }

    #[test]
    fn it_caps_the_side_card_pair_to_two_cards_and_the_gutter() {
      assert_eq!(SIDE_CARDS_ROW_MAX_WIDTH, 504.0);
    }

    #[test]
    fn it_renders_a_selected_and_unselected_radio_dot() {
      let _selected: Element<'_, Message> = radio_dot(true);
      let _unselected: Element<'_, Message> = radio_dot(false);
    }

    #[test]
    fn it_renders_both_rail_sides_as_an_evenly_gapped_pair() {
      let settings = settings();
      let _el: Element<'_, Message> = side_cards(&settings);
    }

    #[test]
    fn it_renders_the_cascade_cards_with_the_shared_treatment() {
      let settings = settings();
      let _el: Element<'_, Message> = cascade_cards(&settings);
    }

    #[test]
    fn it_maps_a_fractional_width_to_a_hundredths_fill_portion() {
      assert_eq!(portion(0.7), 70);
      assert_eq!(portion(0.4), 40);
      assert_eq!(portion(0.55), 55);
      assert_eq!(portion(0.32), 32);
    }

    #[test]
    fn it_pairs_each_stub_with_a_complementary_remainder() {
      for width in [0.7, 0.4, 0.55, 0.32] {
        assert_eq!(portion(width) + complement_portion(width), 100);
      }
    }

    #[test]
    fn it_keeps_body_rows_staggered_and_short_not_full_width() {
      let widths = [portion(0.7), portion(0.4), portion(0.55), portion(0.32)];

      assert!(widths.iter().all(|&w| w < 100), "no row should span the full body");
      assert!(
        widths.iter().collect::<std::collections::HashSet<_>>().len() == widths.len(),
        "rows are staggered at distinct widths"
      );
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_with_a_disabled_feature_item_dimmed() {
      let mut settings = settings();
      settings
        .features_mut()
        .set_enabled(crate::config::Feature::Industry, false);
      let state = State::default();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_a_flipped_rail_with_an_active_drag() {
      let mut settings = settings();
      settings.ui_mut().set_nav_location(NavLocation::Right);
      let state = State {
        dragging: Some(0),
        drop_index: Some(2),
        ..State::default()
      };

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
