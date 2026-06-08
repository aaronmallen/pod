mod card;
mod card_grid;
mod item_row;

use std::sync::Arc;

use iced::{
  Background, Border, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, image, scrollable, text, text_editor},
};

use super::{
  Message, StockpileContextMenu, fmt_count,
  stockpile_search::{self, MultibuyMatch, MultibuyResolution},
};
use crate::{
  clients::{esi, eve_image, eve_image::Size, eve_sso},
  store::{
    Database,
    images::{self, IconResolution},
    model::stockpile_fill::{StockpileFill, StockpileItemFill},
    repo::{assets, sde},
  },
  ui::{
    components::{
      backdrop,
      chip::Chip,
      context_menu::{self, Item},
      icon_tile::icon_tile,
      text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

pub const SEARCH_MIN_CHARS: usize = 3;

const MAX_SUGGESTIONS: usize = 20;
const ICON_SIZE: Size = Size::S64;
const ICON_BOX: f32 = 22.0;
const FORM_WIDTH: f32 = 400.0;
const IMPORT_PANEL_WIDTH: f32 = 480.0;
const IMPORT_FIELD_HEIGHT: f32 = 168.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct StockpileItemLine {
  pub have: i64,
  pub pct: f64,
  pub target: i64,
  pub type_id: i64,
  pub type_name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StockpileCard {
  pub(super) character_id: Option<i64>,
  pub(super) id: i64,
  pub(super) items: Vec<StockpileItemLine>,
  pub(super) location_id: Option<i64>,
  pub(super) location_name: Option<String>,
  pub(super) name: String,
  pub(super) overall_pct: f64,
}

impl StockpileCard {
  fn is_full(&self) -> bool {
    self.items.iter().all(|item| item.have >= item.target)
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct EditorItem {
  pub query: String,
  pub searching: bool,
  pub suggestions: Vec<(i64, String)>,
  pub target: String,
  pub type_id: Option<i64>,
  pub type_name: Option<String>,
}

impl EditorItem {
  fn resolved(type_id: i64, type_name: String, target: i64) -> Self {
    Self {
      query: type_name.clone(),
      searching: false,
      suggestions: Vec::new(),
      target: target.to_string(),
      type_id: Some(type_id),
      type_name: Some(type_name),
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct ImportPanel {
  resolution: Option<MultibuyResolution>,
  text: text_editor::Content,
}

impl ImportPanel {
  pub(super) fn blank() -> Self {
    Self::default()
  }

  pub(super) fn apply(&mut self, action: text_editor::Action) {
    if action.is_edit() {
      self.resolution = None;
    }
    self.text.perform(action);
  }

  pub(super) fn content(&self) -> &text_editor::Content {
    &self.text
  }

  pub(super) fn matched(&self) -> &[MultibuyMatch] {
    self.resolution.as_ref().map(|r| r.matched.as_slice()).unwrap_or(&[])
  }

  pub(super) fn resolution(&self) -> Option<&MultibuyResolution> {
    self.resolution.as_ref()
  }

  pub(super) fn set_resolution(&mut self, resolution: MultibuyResolution) {
    self.resolution = Some(resolution);
  }

  #[cfg(test)]
  pub(super) fn set_text(&mut self, value: String) {
    self.text = text_editor::Content::with_text(&value);
    self.resolution = None;
  }

  pub(super) fn text(&self) -> String {
    self.text.text()
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
  character_id: Option<i64>,
  editing_id: Option<i64>,
  error: String,
  items: Vec<EditorItem>,
  location_id: Option<i64>,
  location_name: Option<String>,
  location_query: String,
  location_searching: bool,
  location_suggestions: Vec<(i64, String)>,
  name: String,
}

impl Editor {
  pub(super) fn blank() -> Self {
    Self {
      character_id: None,
      editing_id: None,
      error: String::new(),
      items: vec![EditorItem::default()],
      location_id: None,
      location_name: None,
      location_query: String::new(),
      location_searching: false,
      location_suggestions: Vec::new(),
      name: String::new(),
    }
  }

  pub(super) fn from_card(card: &StockpileCard) -> Self {
    let items = if card.items.is_empty() {
      vec![EditorItem::default()]
    } else {
      card
        .items
        .iter()
        .map(|item| EditorItem {
          query: item.type_name.clone(),
          searching: false,
          suggestions: Vec::new(),
          target: item.target.to_string(),
          type_id: Some(item.type_id),
          type_name: Some(item.type_name.clone()),
        })
        .collect()
    };
    Self {
      character_id: card.character_id,
      editing_id: Some(card.id),
      error: String::new(),
      items,
      location_id: card.location_id,
      location_name: card.location_name.clone(),
      location_query: card.location_name.clone().unwrap_or_default(),
      location_searching: false,
      location_suggestions: Vec::new(),
      name: card.name.clone(),
    }
  }

  pub(super) fn add_item(&mut self) {
    self.items.push(EditorItem::default());
  }

  pub(super) fn clear_item(&mut self, index: usize) {
    if let Some(item) = self.items.get_mut(index) {
      item.type_id = None;
      item.type_name = None;
      item.query.clear();
      item.searching = false;
      item.suggestions.clear();
    }
  }

  pub(super) fn clear_location(&mut self) {
    self.location_id = None;
    self.location_name = None;
    self.location_query.clear();
    self.location_searching = false;
    self.location_suggestions.clear();
  }

  pub(super) fn error(&self) -> &str {
    &self.error
  }

  pub(super) fn is_editing(&self) -> bool {
    self.editing_id.is_some()
  }

  pub(super) fn items(&self) -> &[EditorItem] {
    &self.items
  }

  pub(super) fn location_name(&self) -> Option<&str> {
    self.location_name.as_deref()
  }

  pub(super) fn location_query(&self) -> &str {
    &self.location_query
  }

  pub(super) fn location_suggestions(&self) -> &[(i64, String)] {
    &self.location_suggestions
  }

  pub(super) fn name(&self) -> &str {
    &self.name
  }

  pub(super) fn pick_item(&mut self, index: usize, id: i64, name: String) {
    if let Some(item) = self.items.get_mut(index) {
      item.type_id = Some(id);
      item.type_name = Some(name.clone());
      item.query = name;
      item.searching = false;
      item.suggestions.clear();
    }
  }

  pub(super) fn pick_location(&mut self, id: i64, name: String) {
    self.location_id = Some(id);
    self.location_name = Some(name.clone());
    self.location_query = name;
    self.location_searching = false;
    self.location_suggestions.clear();
  }

  pub(super) fn prefill_items(&mut self, matched: &[MultibuyMatch]) {
    let rows: Vec<EditorItem> = matched
      .iter()
      .map(|m| EditorItem::resolved(m.type_id, m.name.clone(), m.quantity as i64))
      .collect();
    self.items = if rows.is_empty() {
      vec![EditorItem::default()]
    } else {
      rows
    };
  }

  pub(super) fn remove_item(&mut self, index: usize) {
    if index < self.items.len() {
      self.items.remove(index);
    }
  }

  pub(super) fn set_item_query(&mut self, index: usize, value: String) {
    if let Some(item) = self.items.get_mut(index) {
      if value.trim().chars().count() < SEARCH_MIN_CHARS {
        item.suggestions.clear();
        item.searching = false;
      } else {
        item.searching = true;
      }
      item.type_id = None;
      item.type_name = None;
      item.query = value;
    }
  }

  pub(super) fn set_item_suggestions(&mut self, index: usize, results: Vec<(i64, String)>) {
    if let Some(item) = self.items.get_mut(index) {
      item.suggestions = results;
      item.searching = false;
    }
  }

  pub(super) fn set_item_target(&mut self, index: usize, value: String) {
    if let Some(item) = self.items.get_mut(index) {
      item.target = value;
    }
  }

  pub(super) fn set_location_query(&mut self, value: String) {
    if value.trim().chars().count() < SEARCH_MIN_CHARS {
      self.location_suggestions.clear();
      self.location_searching = false;
    } else {
      self.location_searching = true;
    }
    self.location_query = value;
  }

  pub(super) fn set_location_suggestions(&mut self, results: Vec<(i64, String)>) {
    self.location_suggestions = results;
    self.location_searching = false;
  }

  pub(super) fn set_name(&mut self, name: String) {
    self.name = name;
  }

  fn parsed_items(&self) -> Vec<(i64, i64)> {
    self
      .items
      .iter()
      .filter_map(|item| {
        let type_id = item.type_id.filter(|id| *id > 0)?;
        let target = item.target.trim().parse::<i64>().unwrap_or(0);
        Some((type_id, target))
      })
      .collect()
  }
}

pub(super) async fn load_cards(db: &Database) -> Vec<StockpileCard> {
  let stockpiles = assets::list_with_items(db).await.unwrap_or_default();
  let mut cards = Vec::with_capacity(stockpiles.len());
  for entry in stockpiles {
    let id = entry.stockpile.id();
    let fill = assets::fill_status(db, id).await.ok().flatten();
    let overall_pct = fill.as_ref().map(StockpileFill::overall_pct).unwrap_or(1.0);

    let mut items = Vec::with_capacity(entry.items.len());
    for item in &entry.items {
      let have = fill
        .as_ref()
        .and_then(|f| f.items.iter().find(|line| line.type_id == item.type_id()))
        .map(|line| line.have_quantity)
        .unwrap_or(0);
      let pct = fill
        .as_ref()
        .and_then(|f| f.items.iter().find(|line| line.type_id == item.type_id()))
        .map(StockpileItemFill::pct)
        .unwrap_or(0.0);
      items.push(StockpileItemLine {
        have,
        pct,
        target: item.target_quantity(),
        type_id: item.type_id(),
        type_name: type_name_of(db, item.type_id()).await,
      });
    }

    let location_id = entry.stockpile.location_id();
    let location_name = match location_id {
      Some(loc) => assets::location_name(db, loc).await.ok().flatten(),
      None => None,
    };

    cards.push(StockpileCard {
      character_id: entry.stockpile.character_id(),
      id,
      items,
      location_id,
      location_name,
      name: entry.stockpile.name().to_owned(),
      overall_pct,
    });
  }
  cards
}

pub(super) async fn save(db: &Database, editor: &Editor) {
  let name = editor.name.trim();
  let name = if name.is_empty() { "Untitled stockpile" } else { name };
  let location_id = editor.location_id;
  let items = editor.parsed_items();
  match editor.editing_id {
    Some(id) => {
      let _ = assets::update(db, id, name, editor.character_id, location_id, &items).await;
    }
    None => {
      let _ = assets::create(db, name, editor.character_id, location_id, &items).await;
    }
  }
}

pub async fn save_stockpile(
  db: Database,
  esi: Arc<esi::Client>,
  image: Arc<eve_image::Client>,
  sso: Arc<eve_sso::Client>,
  editor: Editor,
) -> Vec<StockpileCard> {
  if let Some(location_id) = editor.location_id {
    stockpile_search::resolve_location(db.clone(), esi, image, sso, location_id).await;
  }
  save(&db, &editor).await;
  load_cards(&db).await
}

pub(super) async fn delete(db: &Database, id: i64) {
  let _ = assets::delete(db, id).await;
}

async fn type_name_of(db: &Database, type_id: i64) -> String {
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item_type| item_type.name().to_owned())
    .unwrap_or_else(|| format!("Type {type_id}"))
}

pub(super) fn body<'a>(
  cards: &'a [StockpileCard],
  editor: Option<&'a Editor>,
  import: Option<&'a ImportPanel>,
) -> Element<'a, Message> {
  let list = container(
    scrollable(card_grid::view(cards))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill);

  let base: Element<'a, Message> = match editor {
    Some(editor) => Row::with_children(vec![list.into(), editor_form(editor)])
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
    None => list.into(),
  };

  match import {
    Some(panel) => Stack::with_children(vec![
      base,
      backdrop::backdrop(Message::StockpileImportClosed),
      import_overlay(panel),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into(),
    None => base,
  }
}

pub(super) fn context_menu_view(menu: &StockpileContextMenu) -> Element<'_, Message> {
  let items = vec![
    Item::action("Edit", Message::StockpileEditStarted(menu.id)),
    Item::separator(),
    Item::danger("Delete", Message::StockpileDeleted(menu.id)),
  ];
  context_menu::context_menu(&menu.name, items, menu.anchor)
}

fn small_button<'a>(label: &'a str, message: Message, text_color: iced::Color) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(text_color),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2,
  })
  .on_press(message)
  .style(|_, _| button::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn editor_form(editor: &Editor) -> Element<'_, Message> {
  let title = if editor.is_editing() {
    "Edit stockpile"
  } else {
    "New stockpile"
  };

  let name_field = Column::with_children(vec![
    field_label("Name"),
    TextInput::new("Stockpile name", editor.name(), Message::StockpileEditorNameChanged)
      .font_size(typography::size::MD)
      .padding(spacing::SPACE_2)
      .render(),
  ])
  .spacing(spacing::UNIT + 1.0);

  let location_field = Column::with_children(vec![field_label("Location (optional)"), location_typeahead(editor)])
    .spacing(spacing::UNIT + 1.0);

  let mut item_rows: Vec<Element<'_, Message>> = Vec::with_capacity(editor.items().len() + 2);
  item_rows.push(
    Row::with_children(vec![
      container(field_label("Items")).width(Length::Fill).into(),
      add_item_button(),
    ])
    .align_y(Vertical::Center)
    .into(),
  );
  for (index, item) in editor.items().iter().enumerate() {
    item_rows.push(editor_item_row(index, item));
  }

  let body = Column::with_children(vec![
    text(title)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    name_field.into(),
    location_field.into(),
    Column::with_children(item_rows).spacing(spacing::SPACE_2).into(),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill);

  container(
    Column::with_children(vec![
      container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(spacing::SPACE_6)
        .into(),
      editor_footer(editor),
    ])
    .height(Length::Fill),
  )
  .width(Length::Fixed(FORM_WIDTH))
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn editor_footer(editor: &Editor) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = Vec::new();
  if !editor.error().is_empty() {
    children.push(
      text(editor.error().to_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
    );
  }
  children.push(Space::new().width(Length::Fill).into());
  children.push(small_button(
    "Cancel",
    Message::StockpileEditorClosed,
    color::text::SECONDARY,
  ));
  children.push(primary_button("Save", Message::StockpileEditorSaved));

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_6,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_6,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn editor_item_row(index: usize, item: &EditorItem) -> Element<'_, Message> {
  let name_field: Element<'_, Message> = match (item.type_id, &item.type_name) {
    (Some(_), Some(name)) => container(
      Chip::new(name.clone(), Some(color::accent::PLASMA))
        .on_remove(Message::StockpileEditorItemCleared(index))
        .view(),
    )
    .width(Length::FillPortion(3))
    .align_y(Vertical::Center)
    .into(),
    _ => container(item_typeahead(index, item))
      .width(Length::FillPortion(3))
      .into(),
  };

  Row::with_children(vec![
    name_field,
    TextInput::new("Qty", &item.target, move |value| {
      Message::StockpileEditorItemTargetChanged(index, value)
    })
    .font_size(typography::size::SM)
    .padding(spacing::SPACE_2)
    .width(Length::FillPortion(2))
    .render(),
    small_button(
      "\u{2715}",
      Message::StockpileEditorItemRemoved(index),
      color::status::DANGER,
    ),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn item_typeahead(index: usize, item: &EditorItem) -> Element<'_, Message> {
  let field = TextInput::new("Search item\u{2026}", &item.query, move |value| {
    Message::StockpileEditorItemSearchChanged(index, value)
  })
  .font_size(typography::size::SM)
  .padding(spacing::SPACE_2)
  .width(Length::Fill)
  .render();

  with_suggestions(
    field,
    suggestions(&item.suggestions, item.searching, true, move |id, name| {
      Message::StockpileEditorItemPicked(index, id, name)
    }),
  )
}

fn location_typeahead(editor: &Editor) -> Element<'_, Message> {
  if let Some(id) = editor.location_id {
    let label = editor
      .location_name()
      .map(str::to_owned)
      .unwrap_or_else(|| format!("Location {id}"));
    return container(
      Chip::new(label, Some(color::accent::PLASMA))
        .on_remove(Message::StockpileEditorLocationCleared)
        .view(),
    )
    .align_y(Vertical::Center)
    .into();
  }

  let field = TextInput::new(
    "Search station, structure, or system\u{2026}",
    editor.location_query(),
    Message::StockpileEditorLocationSearchChanged,
  )
  .font_size(typography::size::SM)
  .padding(spacing::SPACE_2)
  .render();

  with_suggestions(
    field,
    suggestions(
      editor.location_suggestions(),
      editor.location_searching,
      false,
      Message::StockpileEditorLocationPicked,
    ),
  )
}

fn with_suggestions<'a>(field: Element<'a, Message>, dropdown: Option<Element<'a, Message>>) -> Element<'a, Message> {
  let dropdown = dropdown.unwrap_or_else(|| Space::new().width(Length::Shrink).height(Length::Shrink).into());
  Column::with_children(vec![field, dropdown]).width(Length::Fill).into()
}

fn suggestions<'a>(
  results: &'a [(i64, String)],
  searching: bool,
  with_icon: bool,
  make_msg: impl Fn(i64, String) -> Message + 'a,
) -> Option<Element<'a, Message>> {
  if results.is_empty() && !searching {
    return None;
  }

  let mut column = Column::new().width(Length::Fill);
  if results.is_empty() {
    column = column.push(suggestion_status("Searching\u{2026}"));
  } else {
    for (id, name) in results.iter().take(MAX_SUGGESTIONS) {
      column = column.push(suggestion_row(*id, name, with_icon, &make_msg));
    }
  }

  Some(
    container(
      container(column)
        .width(Length::Fill)
        .padding(spacing::UNIT)
        .style(|_| container::Style {
          background: Some(Background::Color(color::surface::RAISED)),
          border: Border {
            color: color::with_alpha(color::text::PRIMARY, 0.16),
            radius: radius::CARD.into(),
            width: 1.0,
          },
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      bottom: spacing::SPACE_2,
      left: 0.0,
      right: 0.0,
    })
    .into(),
  )
}

fn suggestion_status<'a>(label: &str) -> Element<'a, Message> {
  container(
    text(label.to_owned())
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  })
  .into()
}

fn suggestion_row<'a>(
  id: i64,
  name: &str,
  with_icon: bool,
  make_msg: &impl Fn(i64, String) -> Message,
) -> Element<'a, Message> {
  let name = name.to_owned();
  let label = name.clone();

  let mut row = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
  if with_icon {
    row = row.push(type_icon(id));
  }
  row = row.push(text(label).size(typography::size::SM).style(|_| text::Style {
    color: Some(color::text::PRIMARY),
  }));

  iced::widget::mouse_area(container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  }))
  .on_press(make_msg(id, name))
  .into()
}

fn type_icon<'a>(type_id: i64) -> Element<'a, Message> {
  let content: Element<'a, Message> = match images::default_store().resolve_type_icon(type_id, None, ICON_SIZE) {
    IconResolution::Found(path) => image(image::Handle::from_path(path))
      .width(Length::Fill)
      .height(Length::Fill)
      .content_fit(ContentFit::Contain)
      .into(),
    IconResolution::Missing => Space::new().into(),
  };
  icon_tile(content, ICON_BOX)
}

fn import_overlay(panel: &ImportPanel) -> Element<'_, Message> {
  let inner = match panel.resolution() {
    Some(resolution) => import_preview(resolution),
    None => import_paste(panel),
  };

  container(inner)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn import_paste(panel: &ImportPanel) -> Element<'_, Message> {
  let field = text_editor(panel.content())
    .placeholder("Paste multibuy text here\u{2026}")
    .on_action(Message::StockpileImportTextChanged)
    .padding(spacing::SPACE_2_5)
    .size(typography::size::SM)
    .height(Length::Fixed(IMPORT_FIELD_HEIGHT))
    .font(typography::mono::REGULAR)
    .style(import_field_editor_style);

  let action = if panel.text().trim().is_empty() {
    None
  } else {
    Some(Message::StockpileImportResolveRequested)
  };

  import_shell_body(
    "Import multibuy",
    "Paste a multibuy list \u{2014} one item per line (Name<tab>qty, Name qty, Name xN, or bare Name). Names resolve against EVE.",
    field.into(),
    "Resolve",
    action,
  )
}

fn import_preview(resolution: &MultibuyResolution) -> Element<'_, Message> {
  let mut matched_rows: Vec<Element<'_, Message>> =
    vec![section_label(&format!("Matched ({})", resolution.matched.len()))];
  if resolution.matched.is_empty() {
    matched_rows.push(muted_text("No items matched."));
  } else {
    for item in &resolution.matched {
      matched_rows.push(
        Row::with_children(vec![
          text(item.name.clone())
            .font(typography::body::REGULAR)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(color::text::PRIMARY),
            })
            .width(Length::Fill)
            .into(),
          text(fmt_count(item.quantity as i64))
            .font(typography::mono::REGULAR)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(color::text::SECONDARY),
            })
            .into(),
        ])
        .spacing(spacing::SPACE_2)
        .into(),
      );
    }
  }

  let mut children = matched_rows;
  if !resolution.unmatched.is_empty() {
    children.push(Space::new().height(spacing::SPACE_2).into());
    children.push(section_label(&format!("Ignored ({})", resolution.unmatched.len())));
    for line in &resolution.unmatched {
      children.push(muted_text(line));
    }
  }

  let body = container(
    scrollable(
      Column::with_children(children)
        .spacing(spacing::SPACE_2)
        .width(Length::Fill),
    )
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fixed(240.0)),
  )
  .width(Length::Fill);

  let confirm = (!resolution.matched.is_empty()).then_some(Message::StockpileImportConfirmed);

  import_shell_body(
    "Review import",
    "Matched items will prefill the editor. Nothing is saved until you set a name and hit Save.",
    body.into(),
    "Add to stockpile",
    confirm,
  )
}

fn section_label<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()
}

fn muted_text<'a>(value: &str) -> Element<'a, Message> {
  text(value.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into()
}

fn import_shell_body<'a>(
  title: &'a str,
  hint: &'a str,
  field: Element<'a, Message>,
  action_label: &'a str,
  action_msg: Option<Message>,
) -> Element<'a, Message> {
  let mut action = primary_button_owned(action_label.to_owned());
  if let Some(msg) = action_msg {
    action = action.on_press(msg);
  }

  let body = Column::with_children(vec![
    text(title.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(hint.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    field,
    Row::with_children(vec![
      small_button("Cancel", Message::StockpileImportClosed, color::text::SECONDARY),
      Space::new().width(Length::Fill).into(),
      action.into(),
    ])
    .align_y(Vertical::Center)
    .into(),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill);

  container(body)
    .width(Length::Fixed(IMPORT_PANEL_WIDTH))
    .padding(spacing::SPACE_6)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn primary_button_owned<'a>(label: String) -> button::Button<'a, Message> {
  button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 3.0,
    right: spacing::SPACE_3_5,
    bottom: spacing::UNIT + 3.0,
    left: spacing::SPACE_3_5,
  })
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.35),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  })
}

fn import_field_editor_style(_: &iced::Theme, _: text_editor::Status) -> text_editor::Style {
  text_editor::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: color::accent::PLASMA_MUTED,
  }
}

fn add_item_button<'a>() -> Element<'a, Message> {
  button(
    text("+ Add item")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 2.0,
    right: spacing::SPACE_3,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_3,
  })
  .on_press(Message::StockpileEditorItemAdded)
  .style(|_, _| button::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.15),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn primary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 3.0,
    right: spacing::SPACE_3_5,
    bottom: spacing::UNIT + 3.0,
    left: spacing::SPACE_3_5,
  })
  .on_press(message)
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.35),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn field_label<'a>(label: &'a str) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod editor {
    use pretty_assertions::assert_eq;

    use super::*;

    fn matched(name: &str, quantity: u64, type_id: i64) -> MultibuyMatch {
      MultibuyMatch {
        name: name.to_owned(),
        quantity,
        type_id,
      }
    }

    #[test]
    fn it_parses_only_resolved_item_rows() {
      let mut editor = Editor::blank();
      editor.set_name("Cache".to_owned());
      editor.pick_item(0, 34, "Tritanium".to_owned());
      editor.set_item_target(0, "1000".to_owned());
      editor.add_item();
      editor.add_item();
      editor.pick_item(2, 35, "Pyerite".to_owned());
      editor.set_item_target(2, "".to_owned());

      let items = editor.parsed_items();

      assert_eq!(items, vec![(34, 1000), (35, 0)]);
    }

    #[test]
    fn it_drops_unresolved_item_rows() {
      let mut editor = Editor::blank();
      editor.set_item_query(0, "Tri".to_owned());
      editor.set_item_target(0, "500".to_owned());

      assert!(editor.parsed_items().is_empty());
    }

    #[test]
    fn it_clears_item_suggestions_below_the_min_char_threshold() {
      let mut editor = Editor::blank();
      editor.set_item_suggestions(0, vec![(34, "Tritanium".to_owned())]);

      editor.set_item_query(0, "Tr".to_owned());

      assert!(editor.items()[0].suggestions.is_empty());
    }

    #[test]
    fn it_picks_an_item_into_a_resolved_chip() {
      let mut editor = Editor::blank();
      editor.set_item_suggestions(0, vec![(34, "Tritanium".to_owned())]);

      editor.pick_item(0, 34, "Tritanium".to_owned());

      assert_eq!(editor.items()[0].type_id, Some(34));
      assert_eq!(editor.items()[0].type_name.as_deref(), Some("Tritanium"));
      assert!(editor.items()[0].suggestions.is_empty());
    }

    #[test]
    fn it_clears_a_resolved_item_back_to_a_query() {
      let mut editor = Editor::blank();
      editor.pick_item(0, 34, "Tritanium".to_owned());

      editor.clear_item(0);

      assert_eq!(editor.items()[0].type_id, None);
      assert_eq!(editor.items()[0].query, "");
    }

    #[test]
    fn it_removes_an_item_row() {
      let mut editor = Editor::blank();
      editor.pick_item(0, 34, "Tritanium".to_owned());
      editor.add_item();
      editor.pick_item(1, 35, "Pyerite".to_owned());

      editor.remove_item(0);

      assert_eq!(editor.items().len(), 1);
      assert_eq!(editor.items()[0].type_id, Some(35));
    }

    #[test]
    fn it_picks_a_location_into_a_chip_and_clears_it() {
      let mut editor = Editor::blank();
      editor.set_location_suggestions(vec![(60_003_760, "Jita IV".to_owned())]);

      editor.pick_location(60_003_760, "Jita IV".to_owned());

      assert_eq!(editor.location_id, Some(60_003_760));
      assert_eq!(editor.location_name(), Some("Jita IV"));
      assert!(editor.location_suggestions().is_empty());

      editor.clear_location();

      assert_eq!(editor.location_id, None);
      assert_eq!(editor.location_query(), "");
    }

    #[test]
    fn it_clears_location_suggestions_below_the_min_char_threshold() {
      let mut editor = Editor::blank();
      editor.set_location_suggestions(vec![(60_003_760, "Jita IV".to_owned())]);

      editor.set_location_query("Ji".to_owned());

      assert!(editor.location_suggestions().is_empty());
    }

    #[test]
    fn it_seeds_the_location_chip_from_an_existing_card() {
      let card = StockpileCard {
        character_id: None,
        id: 1,
        items: vec![],
        location_id: Some(60_003_760),
        location_name: Some("Jita IV".to_owned()),
        name: "Cache".to_owned(),
        overall_pct: 0.0,
      };

      let editor = Editor::from_card(&card);

      assert_eq!(editor.location_id, Some(60_003_760));
      assert_eq!(editor.location_name(), Some("Jita IV"));
    }

    #[test]
    fn it_prefills_resolved_item_rows_from_matched_multibuy() {
      let mut editor = Editor::blank();

      editor.prefill_items(&[matched("Tritanium", 1000, 34), matched("Pyerite", 50, 35)]);

      let items = editor.parsed_items();
      assert_eq!(items, vec![(34, 1000), (35, 50)]);
      assert_eq!(editor.items()[0].type_name.as_deref(), Some("Tritanium"));
    }

    #[test]
    fn it_prefills_a_blank_row_when_no_items_matched() {
      let mut editor = Editor::blank();

      editor.prefill_items(&[]);

      assert_eq!(editor.items().len(), 1);
      assert_eq!(editor.items()[0].type_id, None);
    }
  }

  mod import_panel {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clears_the_resolution_when_an_edit_is_applied() {
      let mut panel = ImportPanel::blank();
      panel.set_resolution(MultibuyResolution {
        matched: vec![MultibuyMatch {
          name: "Tritanium".to_owned(),
          quantity: 1,
          type_id: 34,
        }],
        unmatched: Vec::new(),
      });

      panel.apply(text_editor::Action::Edit(text_editor::Edit::Paste(
        std::sync::Arc::new("Pyerite 5".to_owned()),
      )));

      assert_eq!(panel.text(), "Pyerite 5");
      assert!(panel.resolution().is_none());
    }

    #[test]
    fn it_clears_the_resolution_when_the_text_changes() {
      let mut panel = ImportPanel::blank();
      panel.set_resolution(MultibuyResolution {
        matched: vec![MultibuyMatch {
          name: "Tritanium".to_owned(),
          quantity: 1,
          type_id: 34,
        }],
        unmatched: Vec::new(),
      });

      panel.set_text("Pyerite 5".to_owned());

      assert_eq!(panel.text(), "Pyerite 5");
      assert!(panel.resolution().is_none());
    }
  }

  mod crud {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_round_trips_a_stockpile_through_create_load_and_delete() {
      let db = store::open_test().await.unwrap();

      let mut editor = Editor::blank();
      editor.set_name("Supply Cache".to_owned());
      editor.pick_item(0, 34, "Tritanium".to_owned());
      editor.set_item_target(0, "1000".to_owned());
      save(&db, &editor).await;

      let cards = load_cards(&db).await;
      assert_eq!(cards.len(), 1);
      let card = &cards[0];
      assert_eq!(card.name, "Supply Cache");
      assert_eq!(card.items.len(), 1);
      assert_eq!(card.items[0].have, 0);
      assert_eq!(card.items[0].target, 1000);
      assert_eq!(card.items[0].pct, 0.0);
      assert_eq!(card.overall_pct, 0.0);
      assert!(!card.is_full());

      let mut edit = Editor::from_card(card);
      edit.set_name("Renamed Cache".to_owned());
      edit.set_item_target(0, "500".to_owned());
      save(&db, &edit).await;
      let cards = load_cards(&db).await;
      assert_eq!(cards[0].name, "Renamed Cache");
      assert_eq!(cards[0].items[0].target, 500);

      delete(&db, cards[0].id).await;
      assert!(load_cards(&db).await.is_empty());
    }
  }

  mod render {
    use super::*;

    fn card_model() -> StockpileCard {
      StockpileCard {
        character_id: None,
        id: 1,
        items: vec![StockpileItemLine {
          have: 400,
          pct: 0.4,
          target: 1000,
          type_id: 34,
          type_name: "Tritanium".to_owned(),
        }],
        location_id: None,
        location_name: None,
        name: "Cache".to_owned(),
        overall_pct: 0.4,
      }
    }

    #[test]
    fn it_renders_the_card_grid_and_fill_status() {
      let cards = vec![card_model()];
      let _el: Element<'_, Message> = body(&cards, None, None);
    }

    #[test]
    fn it_renders_the_editor_form_with_a_typeahead_and_a_resolved_chip() {
      let mut editor = Editor::blank();
      editor.pick_location(60_003_760, "Jita IV".to_owned());
      editor.pick_item(0, 34, "Tritanium".to_owned());
      editor.add_item();
      editor.set_item_suggestions(1, vec![(35, "Pyerite".to_owned())]);

      let _el: Element<'_, Message> = body(&[], Some(&editor), None);
    }

    #[test]
    fn it_renders_the_import_paste_overlay() {
      let mut panel = ImportPanel::blank();
      panel.set_text("Tritanium 1000".to_owned());

      let _el: Element<'_, Message> = body(&[], None, Some(&panel));
    }

    #[test]
    fn it_renders_the_import_reconcile_preview() {
      let mut panel = ImportPanel::blank();
      panel.set_resolution(MultibuyResolution {
        matched: vec![MultibuyMatch {
          name: "Tritanium".to_owned(),
          quantity: 1000,
          type_id: 34,
        }],
        unmatched: vec!["Notathing".to_owned()],
      });

      let _el: Element<'_, Message> = body(&[], None, Some(&panel));
    }

    #[test]
    fn it_renders_the_empty_state() {
      let _el: Element<'_, Message> = body(&[], None, None);
    }
  }
}
