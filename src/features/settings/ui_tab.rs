use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, svg, text},
};

use super::Outcome;
use crate::{
  config::{CascadeMode, NavLocation, Settings},
  features::registry,
  ui::{
    components::rule,
    style::{color, radius, spacing, typography},
  },
};

const ICON_CHIP_SIZE: f32 = 38.0;
const ICON_SIZE: f32 = 20.0;
const ORDER_BUTTON_SIZE: f32 = 28.0;
const PANEL_SIDE_PADDING: f32 = 36.0;
const PREVIEW_HEIGHT: f32 = 100.0;
const SIDE_CARD_MAX_WIDTH: f32 = 240.0;
const ROW_LIST_MAX_WIDTH: f32 = 560.0;

const RAIL_SIDES: [NavLocation; 2] = [NavLocation::Left, NavLocation::Right];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
  CascadeSelected(CascadeMode),
  Dropped,
  HoverSlot(usize),
  LeaveSlot(usize),
  MoveDown(usize),
  MoveUp(usize),
  PickUp(usize),
  ResetOrder,
  SideSelected(NavLocation),
}

#[derive(Debug, Default)]
pub struct State {
  dragging: Option<usize>,
  drop_index: Option<usize>,
}

impl State {
  pub fn from_settings(_settings: &Settings) -> Self {
    State::default()
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

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::CascadeSelected(mode) => {
      if *settings.ui().cascade_mode() == mode {
        Outcome::None
      } else {
        settings.ui_mut().set_cascade_mode(mode);
        Outcome::UiChanged
      }
    }
    Message::Dropped => {
      let from = state.dragging.take();
      let to = state.drop_index.take();
      match (from, to) {
        (Some(from), Some(to)) => move_to(settings, from, to),
        _ => Outcome::None,
      }
    }
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
    Message::ResetOrder => {
      if is_default_order(settings) {
        Outcome::None
      } else {
        settings
          .ui_mut()
          .set_rail_order(crate::ui::components::rail::Destination::REORDERABLE.to_vec());
        Outcome::UiChanged
      }
    }
    Message::SideSelected(side) => {
      if *settings.ui().nav_location() == side {
        Outcome::None
      } else {
        settings.ui_mut().set_nav_location(side);
        Outcome::UiChanged
      }
    }
  }
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
    NavLocation::Left => "Left".to_owned(),
    NavLocation::Right => "Right".to_owned(),
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if state.dragging.is_none() {
    return iced::Subscription::none();
  }
  iced::event::listen_with(|event, _status, _id| {
    matches!(
      event,
      iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
    )
    .then_some(Message::Dropped)
  })
}

pub fn view<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  Column::with_children(vec![header(), body(state, settings)])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn header<'a>() -> Element<'a, Message> {
  let title = text("User Interface")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(
    "Choose which side the navigation rail sits on and the order its icons appear in. Changes apply \
      live across every Pod view.",
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

fn body<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let side_head = section_head(
    "Rail side",
    "Which edge of the workspace the navigation rail is docked to.",
    live_chip("Applies live \u{00b7} all views"),
  );
  let side_cards = side_cards(settings);

  let cascade_head = section_head(
    "Rail cascade",
    "How a view\u{2019}s sub-sections surface from the rail. Flyout pops them on hover; off keeps a plain rail.",
    live_chip("Applies live \u{00b7} all views"),
  );
  let cascade_cards = cascade_cards(settings);

  let order_head = section_head(
    "Icon order",
    "Drag a row \u{2014} or use the arrows \u{2014} to reorder the rail. Settings stays pinned at the end.",
    reset_button(settings),
  );
  let order_list = order_list(state, settings);

  let inner = container(
    Column::with_children(vec![
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
    .style(typography::colored(color::accent::PLASMA));
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
      .style(typography::colored(color::accent::PLASMA)),
  )
  .padding(Padding {
    top: 4.0,
    right: spacing::SPACE_2_5,
    bottom: 4.0,
    left: spacing::SPACE_2_5,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn side_cards(settings: &Settings) -> Element<'_, Message> {
  let selected = *settings.ui().nav_location();
  let mut cards: Vec<Element<'_, Message>> = RAIL_SIDES
    .into_iter()
    .map(|side| nav_card(side, selected == side))
    .collect();
  cards.push(Space::new().width(Length::Fill).into());

  Row::with_children(cards)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn nav_card<'a>(side: NavLocation, selected: bool) -> Element<'a, Message> {
  let preview = container(nav_preview(side, selected))
    .width(Length::Fill)
    .height(Length::Fixed(PREVIEW_HEIGHT));

  let label = text(match side {
    NavLocation::Left => "Left",
    NavLocation::Right => "Right",
  })
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

  let card_button = button(card)
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::SideSelected(side))
    .style(move |_, _| button::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: if selected {
          color::accent::PLASMA
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    });

  container(card_button)
    .width(Length::Fill)
    .max_width(SIDE_CARD_MAX_WIDTH)
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

  button(container(footer).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  }))
  .padding(0)
  .width(Length::FillPortion(1))
  .on_press(Message::CascadeSelected(mode))
  .style(move |_, _| button::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: if selected {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::text::PRIMARY, 0.1)
      },
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn cascade_note(mode: CascadeMode) -> &'static str {
  match mode {
    CascadeMode::Flyout => "Hover pops sub-sections out",
    CascadeMode::None => "Plain rail, no cascade",
    CascadeMode::SubRail => "A pinned second column",
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
    color::accent::PLASMA
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
      ..container::Style::default()
    })
    .into()
}

fn radio_dot<'a>(selected: bool) -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(14.0))
    .height(Length::Fixed(14.0))
    .style(move |_| container::Style {
      background: selected.then_some(Background::Color(color::accent::PLASMA)),
      border: Border {
        color: if selected {
          color::accent::PLASMA
        } else {
          color::rule_strong()
        },
        width: 1.0,
        radius: 7.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn reset_button(settings: &Settings) -> Element<'_, Message> {
  let enabled = !is_default_order(settings);
  let label = text("Reset order")
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(if enabled {
      color::text::secondary()
    } else {
      color::text::tertiary()
    }));

  button(label)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
    })
    .on_press_maybe(enabled.then_some(Message::ResetOrder))
    .style(move |_, status| {
      let border_alpha = match status {
        button::Status::Hovered | button::Status::Pressed if enabled => 0.18,
        _ => 0.1,
      };
      button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: if enabled {
          color::text::secondary()
        } else {
          color::text::tertiary()
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
        color::with_alpha(color::accent::PLASMA, 0.05)
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
        color::accent::PLASMA
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
      assert_eq!(settings.ui().rail_order()[1], Destination::Characters);
    }

    #[test]
    fn it_keeps_the_first_item_pinned_when_moved_up() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::MoveUp(0), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(settings.ui().rail_order()[0], Destination::Characters);
    }

    #[test]
    fn it_moves_an_item_down_with_the_arrow() {
      let mut state = State::default();
      let mut settings = settings();

      let outcome = update(&mut state, Message::MoveDown(0), &mut settings);

      assert_eq!(outcome, Outcome::UiChanged);
      assert_eq!(settings.ui().rail_order()[0], Destination::Skills);
      assert_eq!(settings.ui().rail_order()[1], Destination::Characters);
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
  }

  mod preview_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_caps_the_side_card_max_width_at_the_design_grid_bound() {
      assert_eq!(SIDE_CARD_MAX_WIDTH, 240.0);
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
      };

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
