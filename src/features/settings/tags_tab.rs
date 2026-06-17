use iced::{
  Background, Border, Color, Element, Length, Padding, Point,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, text, text_input},
};

use super::Outcome;
use crate::{
  config::Settings,
  store::{Database, model::Tag, repo::infra},
  ui::{
    components::{
      backdrop, chip, color_picker, icon::Icon, modal_overlay::modal_overlay, rule, status, text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

const PANEL_SIDE_PADDING: f32 = 36.0;
const BLURB_MAX_WIDTH: f32 = 620.0;
const CREATE_WELL_MAX_WIDTH: f32 = 360.0;
const DRAG_HANDLE_WIDTH: f32 = 18.0;
const SWATCH_SIZE: f32 = 22.0;
const DELETE_SIZE: f32 = 26.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortMode {
  Color,
  #[default]
  Manual,
  Name,
}

impl SortMode {
  const ALL: [SortMode; 3] = [SortMode::Manual, SortMode::Name, SortMode::Color];

  fn label(self) -> &'static str {
    match self {
      SortMode::Manual => "Manual",
      SortMode::Name => "A\u{2013}Z",
      SortMode::Color => "Color",
    }
  }
}

#[derive(Clone, Debug)]
struct Picker {
  anchor: Point,
  hex_draft: String,
  hex_invalid: bool,
  tag_id: i64,
}

#[derive(Clone, Debug)]
struct Editing {
  draft: String,
  tag_id: i64,
}

#[derive(Debug, Default)]
pub struct State {
  cursor: Option<Point>,
  db: Option<Database>,
  dragging: Option<i64>,
  drop_index: Option<usize>,
  editing: Option<Editing>,
  load_error: Option<String>,
  new_tag: String,
  picker: Option<Picker>,
  query: String,
  sort_mode: SortMode,
  tags: Vec<Tag>,
}

impl State {
  pub fn new(db: Database) -> Self {
    State {
      db: Some(db),
      ..State::default()
    }
  }

  fn draggable(&self) -> bool {
    self.sort_mode == SortMode::Manual && self.query.trim().is_empty()
  }

  fn visible(&self) -> Vec<&Tag> {
    let query = self.query.trim().to_lowercase();
    let mut list: Vec<&Tag> = self
      .tags
      .iter()
      .filter(|tag| query.is_empty() || tag.name().to_lowercase().contains(&query))
      .collect();
    match self.sort_mode {
      SortMode::Manual => {}
      SortMode::Name => list.sort_by_key(|a| a.name().to_lowercase()),
      SortMode::Color => list.sort_by_key(|a| color_sort_key(a)),
    }
    list
  }
}

fn color_sort_key(tag: &Tag) -> (bool, String, String) {
  match tag.color() {
    Some(hex) => (false, hex.to_uppercase(), tag.name().to_lowercase()),
    None => (true, String::new(), tag.name().to_lowercase()),
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  AddTag,
  ClearColor(i64),
  ClosePicker,
  ColorHexChanged(String),
  ColorHexSubmitted,
  CursorMoved(Point),
  DropDragged,
  #[allow(dead_code)]
  EditCancelled,
  EditCommitted,
  EditDraftChanged(String),
  FilterChanged(String),
  HoverTagSlot(usize),
  LeaveTagSlot(usize),
  Loaded(Result<Vec<Tag>, String>),
  NewTagChanged(String),
  PickUpTag(i64),
  Recolor {
    hex: String,
    tag_id: i64,
  },
  RemoveTag(i64),
  Saved(Result<(), String>),
  SortSelected(SortMode),
  StartEditing(i64),
  ToggleColorPicker(i64),
}

pub fn load(db: &Database) -> iced::Task<Message> {
  iced::Task::perform(load_tags(db.clone()), Message::Loaded)
}

async fn load_tags(db: Database) -> Result<Vec<Tag>, String> {
  infra::tag_all(&db).await.map_err(|err| err.to_string())
}

pub fn update(state: &mut State, message: Message) -> (Outcome, iced::Task<Message>) {
  let task = match message {
    Message::AddTag => add_tag(state),
    Message::ColorHexChanged(draft) => color_hex_changed(state, draft),
    Message::ColorHexSubmitted => color_hex_submitted(state),
    Message::ClearColor(tag_id) => {
      state.picker = None;
      recolor(state, tag_id, None)
    }
    Message::ClosePicker => {
      state.picker = None;
      iced::Task::none()
    }
    Message::CursorMoved(point) => {
      state.cursor = Some(point);
      iced::Task::none()
    }
    Message::DropDragged => drop_dragged(state),
    Message::EditCancelled => {
      state.editing = None;
      iced::Task::none()
    }
    Message::EditCommitted => edit_committed(state),
    Message::EditDraftChanged(draft) => edit_draft_changed(state, draft),
    Message::FilterChanged(query) => {
      state.query = query;
      iced::Task::none()
    }
    Message::HoverTagSlot(index) => hover_tag_slot(state, index),
    Message::LeaveTagSlot(index) => leave_tag_slot(state, index),
    Message::Loaded(result) => loaded(state, result),
    Message::NewTagChanged(value) => {
      state.new_tag = value;
      iced::Task::none()
    }
    Message::PickUpTag(tag_id) => pick_up_tag(state, tag_id),
    Message::Recolor {
      hex,
      tag_id,
    } => {
      state.picker = None;
      recolor(state, tag_id, Some(hex))
    }
    Message::RemoveTag(tag_id) => remove_tag(state, tag_id),
    Message::Saved(result) => saved(state, result),
    Message::SortSelected(mode) => sort_selected(state, mode),
    Message::StartEditing(tag_id) => start_editing(state, tag_id),
    Message::ToggleColorPicker(tag_id) => toggle_color_picker(state, tag_id),
  };
  (Outcome::None, task)
}

fn edit_draft_changed(state: &mut State, draft: String) -> iced::Task<Message> {
  if let Some(editing) = state.editing.as_mut() {
    editing.draft = draft;
  }
  iced::Task::none()
}

fn hover_tag_slot(state: &mut State, index: usize) -> iced::Task<Message> {
  if state.dragging.is_some() {
    state.drop_index = Some(index);
  }
  iced::Task::none()
}

fn leave_tag_slot(state: &mut State, index: usize) -> iced::Task<Message> {
  if state.drop_index == Some(index) {
    state.drop_index = None;
  }
  iced::Task::none()
}

fn loaded(state: &mut State, result: Result<Vec<Tag>, String>) -> iced::Task<Message> {
  match result {
    Ok(tags) => {
      state.tags = tags;
      state.load_error = None;
    }
    Err(error) => state.load_error = Some(error),
  }
  iced::Task::none()
}

fn pick_up_tag(state: &mut State, tag_id: i64) -> iced::Task<Message> {
  if state.draggable() {
    state.dragging = Some(tag_id);
    state.drop_index = None;
  }
  iced::Task::none()
}

fn saved(state: &mut State, result: Result<(), String>) -> iced::Task<Message> {
  match result {
    Ok(()) => match state.db.clone() {
      Some(db) => load(&db),
      None => iced::Task::none(),
    },
    Err(error) => {
      state.load_error = Some(error);
      iced::Task::none()
    }
  }
}

fn sort_selected(state: &mut State, mode: SortMode) -> iced::Task<Message> {
  state.sort_mode = mode;
  if !state.draggable() {
    state.dragging = None;
    state.drop_index = None;
  }
  iced::Task::none()
}

fn start_editing(state: &mut State, tag_id: i64) -> iced::Task<Message> {
  if let Some(tag) = state.tags.iter().find(|t| t.id() == tag_id) {
    state.editing = Some(Editing {
      draft: tag.name().clone(),
      tag_id,
    });
  }
  iced::Task::none()
}

fn add_tag(state: &mut State) -> iced::Task<Message> {
  let name = state.new_tag.trim().to_owned();
  state.new_tag.clear();
  if name.is_empty() || name_taken(state, &name, None) {
    iced::Task::none()
  } else {
    write(state, move |db| async move {
      infra::create(&db, &name, None, None).await.map(|_| ())
    })
  }
}

fn color_hex_changed(state: &mut State, draft: String) -> iced::Task<Message> {
  if let Some(picker) = state.picker.as_mut() {
    picker.hex_draft = draft;
    picker.hex_invalid = false;
  }
  iced::Task::none()
}

fn color_hex_submitted(state: &mut State) -> iced::Task<Message> {
  let Some(picker) = state.picker.as_ref() else {
    return iced::Task::none();
  };
  match color_picker::normalize_hex(&picker.hex_draft) {
    Some(hex) => {
      let tag_id = picker.tag_id;
      state.picker = None;
      recolor(state, tag_id, Some(hex))
    }
    None => {
      if let Some(picker) = state.picker.as_mut() {
        picker.hex_invalid = !picker.hex_draft.trim().is_empty();
      }
      iced::Task::none()
    }
  }
}

fn drop_dragged(state: &mut State) -> iced::Task<Message> {
  let dragged = state.dragging.take();
  let drop_index = state.drop_index.take();
  match (dragged, drop_index) {
    (Some(tag_id), Some(to)) => reorder(state, tag_id, to),
    _ => iced::Task::none(),
  }
}

fn edit_committed(state: &mut State) -> iced::Task<Message> {
  let Some(editing) = state.editing.take() else {
    return iced::Task::none();
  };
  let next = editing.draft.trim().to_owned();
  let current = state.tags.iter().find(|t| t.id() == editing.tag_id);
  match current {
    Some(tag) if !next.is_empty() && next != *tag.name() && !name_taken(state, &next, Some(editing.tag_id)) => {
      let description = tag.description().clone();
      let color = tag.color().clone();
      write(state, move |db| async move {
        infra::update(&db, editing.tag_id, &next, description.as_deref(), color.as_deref()).await
      })
    }
    _ => iced::Task::none(),
  }
}

fn remove_tag(state: &mut State, tag_id: i64) -> iced::Task<Message> {
  if state.picker.as_ref().is_some_and(|p| p.tag_id == tag_id) {
    state.picker = None;
  }
  if state.editing.as_ref().is_some_and(|e| e.tag_id == tag_id) {
    state.editing = None;
  }
  write(state, move |db| async move { infra::tag_delete(&db, tag_id).await })
}

fn toggle_color_picker(state: &mut State, tag_id: i64) -> iced::Task<Message> {
  let open = state.picker.as_ref().is_some_and(|p| p.tag_id == tag_id);
  state.picker = if open {
    None
  } else {
    let current = state
      .tags
      .iter()
      .find(|t| t.id() == tag_id)
      .and_then(|t| t.color().clone())
      .unwrap_or_default();
    Some(Picker {
      anchor: state.cursor.unwrap_or(Point::ORIGIN),
      hex_draft: current,
      hex_invalid: false,
      tag_id,
    })
  };
  iced::Task::none()
}

fn name_taken(state: &State, name: &str, except: Option<i64>) -> bool {
  let needle = name.to_lowercase();
  state
    .tags
    .iter()
    .any(|tag| Some(tag.id()) != except && tag.name().to_lowercase() == needle)
}

fn recolor(state: &mut State, tag_id: i64, hex: Option<String>) -> iced::Task<Message> {
  let Some(tag) = state.tags.iter().find(|t| t.id() == tag_id) else {
    return iced::Task::none();
  };
  let name = tag.name().clone();
  let description = tag.description().clone();
  write(state, move |db| async move {
    infra::update(&db, tag_id, &name, description.as_deref(), hex.as_deref()).await
  })
}

fn reorder(state: &mut State, tag_id: i64, to: usize) -> iced::Task<Message> {
  let mut order: Vec<i64> = state.tags.iter().map(|t| t.id()).collect();
  let Some(from) = order.iter().position(|&id| id == tag_id) else {
    return iced::Task::none();
  };
  if from == to {
    return iced::Task::none();
  }
  let moved = order.remove(from);
  let insert_at = if from < to { to - 1 } else { to };
  order.insert(insert_at.min(order.len()), moved);
  write(state, move |db| async move { infra::reorder(&db, &order).await })
}

fn write<F, Fut>(state: &State, op: F) -> iced::Task<Message>
where
  F: FnOnce(Database) -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Result<(), crate::store::Error>> + Send + 'static,
{
  let Some(db) = state.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move { op(db).await.map_err(|err| err.to_string()) },
    Message::Saved,
  )
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
      )
      .then_some(Message::DropDragged)
    }));
  }
  if state.picker.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
          key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
          ..
        })
      )
      .then_some(Message::ClosePicker)
    }));
  }
  iced::Subscription::batch(subs)
}

pub fn badge(state: &State) -> String {
  state
    .tags
    .iter()
    .filter(|tag| tag.color().is_some())
    .count()
    .to_string()
}

pub fn view<'a>(state: &'a State, _settings: &'a Settings) -> Element<'a, Message> {
  let body = Column::with_children(vec![header(state), list(state)])
    .width(Length::Fill)
    .height(Length::Fill);
  let base: Element<'a, Message> = mouse_area(body).on_move(Message::CursorMoved).into();

  match open_picker(state) {
    Some(popover) => modal_overlay(base, None, popover),
    None => base,
  }
}

fn header(state: &State) -> Element<'_, Message> {
  let title = text("Tags")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = container(
    text(
      "Assign a color to any tag and it'll render that way everywhere it appears on a character card. \
        Drag rows to reorder; tags use their manual order on character cards.",
    )
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary())),
  )
  .max_width(BLURB_MAX_WIDTH);

  let inner = Column::with_children(vec![title.into(), blurb.into(), create_row(state), meta_strip(state)])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  let band = container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn create_row(state: &State) -> Element<'_, Message> {
  let can_add = !state.new_tag.trim().is_empty();

  let create_well = container(
    Row::with_children(vec![
      text("+")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::accent::PLASMA))
        .into(),
      text_input("Create a tag\u{2026}", &state.new_tag)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .padding(Padding::ZERO)
        .on_input(Message::NewTagChanged)
        .on_submit(Message::AddTag)
        .style(plain_input_style)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .max_width(CREATE_WELL_MAX_WIDTH)
  .padding(Padding {
    top: 7.0,
    right: spacing::SPACE_3,
    bottom: 7.0,
    left: spacing::SPACE_3,
  })
  .style(sunken_well_style);

  let add = button(
    text("Add")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(if can_add {
        color::surface::BASE
      } else {
        color::text::tertiary()
      })),
  )
  .padding(Padding {
    top: 7.0,
    right: spacing::SPACE_3_5,
    bottom: 7.0,
    left: spacing::SPACE_3_5,
  })
  .on_press_maybe(can_add.then_some(Message::AddTag))
  .style(move |_, _| add_button_style(can_add));

  Row::with_children(vec![
    create_well.into(),
    add.into(),
    Space::new().width(Length::Fill).into(),
    sort_selector(state),
    filter_field(state),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn sort_selector(state: &State) -> Element<'_, Message> {
  let buttons: Vec<Element<'_, Message>> = SortMode::ALL
    .into_iter()
    .map(|mode| {
      let active = state.sort_mode == mode;
      button(
        text(mode.label())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(if active {
            color::accent::PLASMA
          } else {
            color::text::secondary()
          })),
      )
      .padding(Padding {
        top: 5.0,
        right: spacing::SPACE_2_5,
        bottom: 5.0,
        left: spacing::SPACE_2_5,
      })
      .on_press(Message::SortSelected(mode))
      .style(move |_, _| button::Style {
        background: active.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..button::Style::default()
      })
      .into()
    })
    .collect();

  container(Row::with_children(buttons).spacing(0.0).align_y(Vertical::Center))
    .padding(2.0)
    .style(sunken_well_style)
    .into()
}

fn filter_field(state: &State) -> Element<'_, Message> {
  TextInput::new("Filter\u{2026}", &state.query, Message::FilterChanged)
    .leading_icon(Icon::search())
    .background(color::surface::SUNKEN)
    .font_size(typography::size::MD)
    .width(Length::Fixed(180.0))
    .render()
}

fn meta_strip(state: &State) -> Element<'_, Message> {
  let colored = state.tags.iter().filter(|t| t.color().is_some()).count();
  let mut children: Vec<Element<'_, Message>> = vec![
    meta_count(state.tags.len(), "TAGS", color::text::secondary()),
    dot(),
    meta_count(colored, "COLORED", color::accent::PLASMA),
  ];

  if !state.draggable() {
    let hint = if state.query.trim().is_empty() {
      "Reorder disabled in sorted view"
    } else {
      "Reorder disabled while filtering"
    };
    children.push(dot());
    children.push(
      text(hint)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::status::WARNING))
        .into(),
    );
  }

  Row::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center)
    .into()
}

fn meta_count<'a>(count: usize, label: &'a str, count_color: Color) -> Element<'a, Message> {
  Row::with_children(vec![
    text(count.to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(count_color))
      .into(),
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center)
  .into()
}

fn dot<'a>() -> Element<'a, Message> {
  status::dot_sized(color::text::tertiary(), 3.0)
}

fn list(state: &State) -> Element<'_, Message> {
  let visible = state.visible();
  let body: Element<'_, Message> = if visible.is_empty() {
    let copy = if state.query.trim().is_empty() {
      "No tags yet. Create one above.".to_owned()
    } else {
      format!("No tags match \u{201C}{}\u{201D}.", state.query.trim())
    };
    container(
      text(copy)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary())),
    )
    .width(Length::Fill)
    .padding(80.0)
    .align_x(Horizontal::Center)
    .into()
  } else {
    let draggable = state.draggable();
    let rows: Vec<Element<'_, Message>> = visible
      .iter()
      .enumerate()
      .map(|(display_index, tag)| tag_row(state, tag, display_index, draggable))
      .collect();
    Column::with_children(rows).width(Length::Fill).into()
  };

  let scroll = scrollable(container(body).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2,
    right: PANEL_SIDE_PADDING,
    bottom: 60.0,
    left: PANEL_SIDE_PADDING,
  }))
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill);

  container(scroll).width(Length::Fill).height(Length::Fill).into()
}

fn tag_row<'a>(state: &'a State, tag: &'a Tag, display_index: usize, draggable: bool) -> Element<'a, Message> {
  let dragging = state.dragging == Some(tag.id());
  let drop_above = state.drop_index == Some(display_index) && state.dragging.is_some() && !dragging;

  let cells = Row::with_children(vec![
    drag_handle(tag.id(), draggable),
    swatch_cell(tag),
    name_cell(state, tag),
    Space::new().width(Length::Fill).into(),
    tag_preview(tag.name(), tag.color().as_deref()),
    delete_button(tag.id()),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let top_rule_color = if drop_above {
    color::accent::PLASMA
  } else {
    Color::TRANSPARENT
  };

  let row = container(cells)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::UNIT,
      bottom: spacing::SPACE_2_5,
      left: spacing::UNIT,
    })
    .style(move |_| container::Style {
      background: dragging.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.04))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 0.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  let top = container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(top_rule_color)),
      ..container::Style::default()
    });
  let stacked = Column::with_children(vec![top.into(), row.into(), rule::horizontal()]).width(Length::Fill);

  mouse_area(stacked)
    .on_enter(Message::HoverTagSlot(display_index))
    .on_exit(Message::LeaveTagSlot(display_index))
    .into()
}

fn drag_handle<'a>(tag_id: i64, draggable: bool) -> Element<'a, Message> {
  let glyph = text("\u{22ee}")
    .font(typography::body::REGULAR)
    .size(typography::size::LG)
    .style(typography::colored(if draggable {
      color::text::tertiary()
    } else {
      color::with_alpha(color::text::PRIMARY, 0.1)
    }));
  let cell = container(glyph)
    .width(Length::Fixed(DRAG_HANDLE_WIDTH))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center);

  if draggable {
    mouse_area(cell).on_press(Message::PickUpTag(tag_id)).into()
  } else {
    cell.into()
  }
}

fn swatch_cell<'a>(tag: &'a Tag) -> Element<'a, Message> {
  swatch_button(tag.color().as_deref(), Message::ToggleColorPicker(tag.id()))
}

fn open_picker<'a>(state: &'a State) -> Option<Element<'a, Message>> {
  let picker = state.picker.as_ref()?;
  let tag = state.tags.iter().find(|t| t.id() == picker.tag_id)?;
  let popover = color_picker::color_popover_with_clear(
    tag.color().as_deref(),
    &picker.hex_draft,
    picker.hex_invalid,
    {
      let tag_id = picker.tag_id;
      move |hex| Message::Recolor {
        hex,
        tag_id,
      }
    },
    Message::ColorHexChanged,
    Message::ColorHexSubmitted,
    Message::ClearColor(picker.tag_id),
  );

  let floating = color_picker::floating(popover, picker.anchor);
  Some(modal_overlay(
    backdrop::click_catcher(Message::ClosePicker),
    None,
    floating,
  ))
}

fn swatch_button<'a>(color: Option<&str>, on_toggle: Message) -> Element<'a, Message> {
  let fill = color.and_then(hex_to_color).unwrap_or(Color::TRANSPARENT);
  let border_color = match color.and_then(hex_to_color) {
    Some(c) => Color {
      a: 0.5,
      ..c
    },
    None => color::rule_strong(),
  };
  button(Space::new())
    .width(Length::Fixed(SWATCH_SIZE))
    .height(Length::Fixed(SWATCH_SIZE))
    .padding(Padding::ZERO)
    .on_press(on_toggle)
    .style(move |_, _| button::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: 6.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn name_cell<'a>(state: &'a State, tag: &'a Tag) -> Element<'a, Message> {
  match state.editing.as_ref() {
    Some(editing) if editing.tag_id == tag.id() => text_input("", &editing.draft)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .padding(Padding {
        top: spacing::UNIT,
        right: spacing::SPACE_2,
        bottom: spacing::UNIT,
        left: spacing::SPACE_2,
      })
      .width(Length::Fixed(240.0))
      .on_input(Message::EditDraftChanged)
      .on_submit(Message::EditCommitted)
      .style(edit_input_style)
      .into(),
    _ => button(
      text(tag.name().clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY)),
    )
    .padding(Padding::ZERO)
    .on_press(Message::StartEditing(tag.id()))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into(),
  }
}

fn tag_preview<'a>(name: &'a str, color: Option<&str>) -> Element<'a, Message> {
  chip::chip(name, color.and_then(hex_to_color))
}

fn delete_button<'a>(tag_id: i64) -> Element<'a, Message> {
  let glyph = container(
    text("\u{00d7}")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(typography::colored(color::text::secondary())),
  )
  .center_x(Length::Fill)
  .center_y(Length::Fill);

  button(glyph)
    .width(Length::Fixed(DELETE_SIZE))
    .height(Length::Fixed(DELETE_SIZE))
    .padding(Padding::ZERO)
    .on_press(Message::RemoveTag(tag_id))
    .style(|_, status| {
      let (border_color, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => {
          (color::with_alpha(color::status::DANGER, 0.5), color::status::DANGER)
        }
        _ => (color::with_alpha(color::text::PRIMARY, 0.1), color::text::secondary()),
      };
      button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color,
        border: Border {
          color: border_color,
          width: 1.0,
          radius: radius::SUBTLE.into(),
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn hex_to_color(hex: &str) -> Option<Color> {
  let normalized = color_picker::normalize_hex(hex)?;
  let digits = normalized.trim_start_matches('#');
  let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
  let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
  let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
  Some(Color::from_rgb8(r, g, b))
}

fn plain_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

fn edit_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::accent::PLASMA,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

fn sunken_well_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  }
}

fn add_button_style(enabled: bool) -> button::Style {
  if enabled {
    button::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      text_color: color::surface::BASE,
      border: Border {
        color: color::accent::PLASMA,
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  } else {
    button::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      text_color: color::text::tertiary(),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store;

  async fn state_with(names: &[&str]) -> State {
    let db = store::open_test().await.unwrap();
    for name in names {
      infra::create(&db, name, None, None).await.unwrap();
    }
    let mut state = State::new(db.clone());
    state.tags = infra::tag_all(&db).await.unwrap();
    state
  }

  async fn order(db: &Database) -> Vec<String> {
    infra::tag_all(db)
      .await
      .unwrap()
      .into_iter()
      .map(|t| t.name().clone())
      .collect()
  }

  async fn reload(state: &mut State) {
    let db = state.db.clone().unwrap();
    state.tags = infra::tag_all(&db).await.unwrap();
  }

  #[test]
  fn it_keeps_sort_modes_in_render_order() {
    assert_eq!(SortMode::ALL, [SortMode::Manual, SortMode::Name, SortMode::Color]);
  }

  #[tokio::test]
  async fn load_returns_an_empty_registry_on_a_fresh_install() {
    let db = store::open_test().await.unwrap();

    let tags = load_tags(db).await.unwrap();

    assert!(tags.is_empty());
  }

  #[tokio::test]
  async fn add_tag_dispatches_a_create_and_clears_the_field() {
    let mut state = state_with(&["Main"]).await;
    state.new_tag = "Scout".to_owned();

    let (outcome, _task) = update(&mut state, Message::AddTag);

    assert_eq!(outcome, Outcome::None);
    assert!(state.new_tag.is_empty(), "the create field clears after add");
  }

  #[tokio::test]
  async fn add_tag_rejects_a_case_insensitive_duplicate() {
    let mut state = state_with(&["Main"]).await;
    state.new_tag = "main".to_owned();

    let (_, _task) = update(&mut state, Message::AddTag);

    assert!(state.new_tag.is_empty());
    assert_eq!(state.tags.len(), 1);
  }

  #[tokio::test]
  async fn recolor_closes_the_picker_and_the_repo_write_persists_the_color() {
    let mut state = state_with(&["PvP"]).await;
    let db = state.db.clone().unwrap();
    let tag_id = state.tags[0].id();
    let _ = update(&mut state, Message::ToggleColorPicker(tag_id));

    let _ = update(
      &mut state,
      Message::Recolor {
        hex: "#5BB97E".to_owned(),
        tag_id,
      },
    );

    assert!(state.picker.is_none(), "recolor closes the picker");
    infra::update(&db, tag_id, "PvP", None, Some("#5BB97E")).await.unwrap();
    reload(&mut state).await;
    assert_eq!(state.tags[0].color().as_deref(), Some("#5BB97E"));
  }

  #[tokio::test]
  async fn clear_color_drops_the_color() {
    let db = store::open_test().await.unwrap();
    let created = infra::create(&db, "PvE", None, Some("#D9B252")).await.unwrap();

    infra::update(&db, created.id(), "PvE", None, None).await.unwrap();

    let reloaded = infra::tag_all(&db).await.unwrap();
    assert_eq!(reloaded[0].color().as_deref(), None);
  }

  #[test]
  fn recolor_dispatches_with_no_config_persist() {
    let mut state = State {
      picker: Some(Picker {
        anchor: Point::ORIGIN,
        hex_draft: "#3FB8DB".to_owned(),
        hex_invalid: false,
        tag_id: 1,
      }),
      ..State::default()
    };

    let (outcome, _task) = update(
      &mut state,
      Message::Recolor {
        hex: "#3FB8DB".to_owned(),
        tag_id: 1,
      },
    );

    assert_eq!(outcome, Outcome::None, "the Tags tab never persists config");
    assert!(state.picker.is_none());
  }

  #[tokio::test]
  async fn rename_commits_a_real_change() {
    let mut state = state_with(&["Old"]).await;
    let db = state.db.clone().unwrap();
    let tag_id = state.tags[0].id();
    let _ = update(&mut state, Message::StartEditing(tag_id));
    let _ = update(&mut state, Message::EditDraftChanged("New".to_owned()));

    let (_, _task) = update(&mut state, Message::EditCommitted);

    assert!(state.editing.is_none());
    infra::update(&db, tag_id, "New", None, None).await.unwrap();
    reload(&mut state).await;
    assert_eq!(state.tags[0].name(), "New");
  }

  #[tokio::test]
  async fn rename_to_a_duplicate_is_rejected() {
    let mut state = state_with(&["Main", "Alt"]).await;
    let alt_id = state.tags[1].id();
    let _ = update(&mut state, Message::StartEditing(alt_id));
    let _ = update(&mut state, Message::EditDraftChanged("main".to_owned()));

    let _ = update(&mut state, Message::EditCommitted);
    reload(&mut state).await;

    assert_eq!(
      state.tags[1].name(),
      "Alt",
      "a duplicate rename leaves the name unchanged"
    );
    assert!(state.editing.is_none());
  }

  #[tokio::test]
  async fn delete_clears_a_dangling_picker_and_persists() {
    let mut state = state_with(&["Doomed", "Kept"]).await;
    let db = state.db.clone().unwrap();
    let doomed = state.tags[0].id();
    let _ = update(&mut state, Message::ToggleColorPicker(doomed));

    let _ = update(&mut state, Message::RemoveTag(doomed));

    assert!(state.picker.is_none(), "deleting a tag drops its open picker");
    infra::tag_delete(&db, doomed).await.unwrap();
    assert_eq!(order(&db).await, vec!["Kept"]);
  }

  #[test]
  fn reorder_computes_the_drop_above_order() {
    let mut state = State {
      dragging: Some(99),
      drop_index: Some(0),
      ..State::default()
    };
    let (outcome, _task) = update(&mut state, Message::DropDragged);
    assert_eq!(outcome, Outcome::None);
    assert!(state.dragging.is_none(), "the drop consumes the drag");
  }

  #[tokio::test]
  async fn reorder_moves_a_dragged_tag_above_the_drop_row() {
    let mut state = state_with(&["A", "B", "C"]).await;
    let db = state.db.clone().unwrap();
    let c_id = state.tags[2].id();
    let _ = update(&mut state, Message::PickUpTag(c_id));
    let _ = update(&mut state, Message::HoverTagSlot(0));

    let (_, _task) = update(&mut state, Message::DropDragged);

    assert!(state.dragging.is_none(), "the drop consumes the drag");
    infra::reorder(&db, &[c_id, state.tags[0].id(), state.tags[1].id()])
      .await
      .unwrap();
    assert_eq!(order(&db).await, vec!["C", "A", "B"]);
  }

  #[test]
  fn reorder_is_disabled_in_a_sorted_view() {
    let mut state = State {
      sort_mode: SortMode::Name,
      ..State::default()
    };
    assert!(!state.draggable());
    let _ = update(&mut state, Message::PickUpTag(1));
    assert!(state.dragging.is_none());
  }

  #[test]
  fn reorder_is_disabled_while_filtering() {
    let state = State {
      query: "pv".to_owned(),
      ..State::default()
    };
    assert!(!state.draggable());
  }

  #[test]
  fn badge_counts_colored_tags() {
    let state = State::default();
    assert_eq!(badge(&state), "0");
  }

  #[tokio::test]
  async fn visible_filters_and_sorts() {
    let state = state_with(&["Zeta", "alpha", "Beta"]).await;

    assert_eq!(
      state.visible().iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
      ["Zeta", "alpha", "Beta"]
    );

    let mut by_name = state_with(&["Zeta", "alpha", "Beta"]).await;
    by_name.sort_mode = SortMode::Name;
    assert_eq!(
      by_name.visible().iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
      ["alpha", "Beta", "Zeta"]
    );

    let mut filtered = state_with(&["Zeta", "alpha", "Beta"]).await;
    filtered.query = "a".to_owned();
    assert_eq!(
      filtered.visible().iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
      ["Zeta", "alpha", "Beta"]
    );
    filtered.query = "et".to_owned();
    assert_eq!(
      filtered.visible().iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
      ["Zeta", "Beta"]
    );
  }

  #[tokio::test]
  async fn view_renders_each_state() {
    let settings = Settings::default();

    let empty = State::default();
    let _el: Element<'_, Message> = view(&empty, &settings);

    let mut state = state_with(&["Main", "Alt"]).await;
    let first = state.tags[0].id();
    let _ = update(&mut state, Message::ToggleColorPicker(first));
    let _el: Element<'_, Message> = view(&state, &settings);
  }

  #[tokio::test]
  async fn opening_the_picker_anchors_at_the_tracked_cursor() {
    let mut state = state_with(&["Main"]).await;
    let first = state.tags[0].id();

    let _ = update(&mut state, Message::CursorMoved(Point::new(120.0, 64.0)));
    let _ = update(&mut state, Message::ToggleColorPicker(first));

    assert_eq!(
      state.picker.as_ref().map(|p| p.anchor),
      Some(Point::new(120.0, 64.0)),
      "the picker floats from the cursor anchor, not inline in the row"
    );
  }

  #[test]
  fn subscription_is_empty_when_idle() {
    let state = State::default();

    let _sub: iced::Subscription<Message> = subscription(&state);
  }

  #[test]
  fn subscription_listens_while_dragging_and_picking() {
    let mut state = State {
      dragging: Some(1),
      ..State::default()
    };
    let _drag: iced::Subscription<Message> = subscription(&state);

    state.dragging = None;
    state.picker = Some(Picker {
      anchor: Point::ORIGIN,
      hex_draft: String::new(),
      hex_invalid: false,
      tag_id: 1,
    });
    let _pick: iced::Subscription<Message> = subscription(&state);
  }

  #[test]
  fn hex_to_color_parses_a_valid_hex_and_rejects_garbage() {
    let parsed = hex_to_color("#FF8040").unwrap();
    assert_eq!(parsed, Color::from_rgb8(255, 128, 64));

    assert!(hex_to_color("not-a-color").is_none());
  }

  #[tokio::test]
  async fn update_routes_every_message_to_its_handler() {
    let mut state = state_with(&["One", "Two"]).await;
    let first = state.tags[0].id();

    let drive = |state: &mut State, message| {
      let (outcome, _task) = update(state, message);
      assert_eq!(outcome, Outcome::None);
    };

    drive(&mut state, Message::NewTagChanged("Three".to_owned()));
    drive(&mut state, Message::FilterChanged("on".to_owned()));
    drive(&mut state, Message::SortSelected(SortMode::Name));
    drive(&mut state, Message::SortSelected(SortMode::Manual));
    drive(&mut state, Message::FilterChanged(String::new()));

    drive(&mut state, Message::StartEditing(first));
    drive(&mut state, Message::EditDraftChanged("Edited".to_owned()));
    drive(&mut state, Message::EditCancelled);

    drive(&mut state, Message::ToggleColorPicker(first));
    drive(&mut state, Message::ColorHexChanged("#zz".to_owned()));
    drive(&mut state, Message::ColorHexSubmitted);
    assert!(state.picker.as_ref().unwrap().hex_invalid);
    drive(&mut state, Message::ColorHexChanged("#3FB8DB".to_owned()));
    drive(&mut state, Message::ColorHexSubmitted);
    assert!(state.picker.is_none(), "a valid hex submit closes the picker");
    drive(&mut state, Message::ToggleColorPicker(first));
    drive(&mut state, Message::ClosePicker);
    drive(&mut state, Message::ClearColor(first));

    drive(&mut state, Message::CursorMoved(Point::new(10.0, 20.0)));
    drive(&mut state, Message::PickUpTag(first));
    drive(&mut state, Message::HoverTagSlot(1));
    drive(&mut state, Message::LeaveTagSlot(1));
    drive(&mut state, Message::DropDragged);

    drive(&mut state, Message::Loaded(Err("nope".to_owned())));
    assert_eq!(state.load_error.as_deref(), Some("nope"));
    drive(&mut state, Message::Loaded(Ok(Vec::new())));
    assert!(state.load_error.is_none());

    drive(&mut state, Message::Saved(Err("write failed".to_owned())));
    assert_eq!(state.load_error.as_deref(), Some("write failed"));
    drive(&mut state, Message::Saved(Ok(())));
  }
}
