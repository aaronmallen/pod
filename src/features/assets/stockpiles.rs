mod card;
mod card_grid;
mod item_row;

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use iced::{
  Background, Border, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, image, scrollable, text, text_editor},
};

use super::{
  Message, StockpileContextMenu, fmt_count, fmt_isk,
  stockpile_search::{self, MultibuyMatch, MultibuyResolution},
};
use crate::{
  clients::{esi, eve_image, eve_image::Size, eve_sso},
  store::{
    Database,
    images::{self, IconResolution},
    model::stockpile_fill::{StockpileFill, StockpileItemFill},
    repo::{assets, finance, sde},
  },
  ui::{
    components::{
      backdrop,
      chip::Chip,
      context_menu::{self, Item},
      eyebrow::eyebrow,
      icon::Icon,
      icon_tile::icon_tile,
      rule,
      segmented::segment_button_style,
      text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

pub const SEARCH_MIN_CHARS: usize = 3;

const MAX_SUGGESTIONS: usize = 20;
const SUGGESTIONS_MAX_HEIGHT: f32 = 240.0;
const ICON_SIZE: Size = Size::S64;
const ICON_BOX: f32 = 22.0;
const EDITOR_MODAL_WIDTH: f32 = 560.0;
const EXPORT_MODAL_WIDTH: f32 = 500.0;
const IMPORT_PANEL_WIDTH: f32 = 560.0;
const IMPORT_FIELD_HEIGHT: f32 = 168.0;
const MODAL_CONTENT_MAX_HEIGHT: f32 = 440.0;
const MODAL_PAD_X: f32 = 20.0;
const MODAL_PAD_Y: f32 = 16.0;
const MULTIBUY_EXPORT_BODY_HEIGHT: f32 = 240.0;

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
  pub(super) fill_isk: f64,
  pub(super) id: i64,
  pub(super) items: Vec<StockpileItemLine>,
  pub(super) location_id: Option<i64>,
  pub(super) location_name: Option<String>,
  pub(super) name: String,
  pub(super) overall_pct: f64,
  pub(super) target_isk: f64,
}

impl StockpileCard {
  pub(super) fn multibuy_deficit(&self) -> Vec<(String, u64)> {
    self
      .items
      .iter()
      .filter_map(|item| {
        let deficit = item.target - item.have;
        (deficit > 0).then(|| (item.type_name.clone(), deficit as u64))
      })
      .collect()
  }

  pub(super) fn multibuy_lines(&self, mode: MultibuyMode) -> Vec<(String, u64)> {
    match mode {
      MultibuyMode::Remaining => self.multibuy_deficit(),
      MultibuyMode::Target => self.multibuy_target(),
    }
  }

  pub(super) fn multibuy_target(&self) -> Vec<(String, u64)> {
    self
      .items
      .iter()
      .filter(|&item| item.target > 0)
      .map(|item| (item.type_name.clone(), item.target as u64))
      .collect()
  }

  pub(super) fn multibuy_value(&self, mode: MultibuyMode) -> f64 {
    match mode {
      MultibuyMode::Remaining => self.fill_isk,
      MultibuyMode::Target => self.target_isk,
    }
  }

  fn is_full(&self) -> bool {
    self.items.iter().all(|item| item.have >= item.target)
  }

  fn short_items(&self) -> usize {
    self.items.iter().filter(|item| item.have < item.target).count()
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MultibuyMode {
  Remaining,
  #[default]
  Target,
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

fn economics(items: &[StockpileItemLine], prices: &HashMap<i64, f64>) -> (f64, f64) {
  let mut target_isk = 0.0;
  let mut fill_isk = 0.0;
  for item in items {
    let price = prices.get(&item.type_id).copied().unwrap_or(0.0);
    target_isk += item.target as f64 * price;
    if item.have < item.target {
      fill_isk += (item.target - item.have) as f64 * price;
    }
  }
  (target_isk, fill_isk)
}

pub(super) async fn load_cards(db: &Database) -> Vec<StockpileCard> {
  let stockpiles = assets::list_with_items(db).await.unwrap_or_default();
  let prices: HashMap<i64, f64> = finance::market_prices_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|price| price.average_price().map(|average| (price.type_id(), average)))
    .collect();
  let mut cards = Vec::with_capacity(stockpiles.len());
  for entry in stockpiles {
    let id = entry.stockpile.id();
    let fill = assets::fill_status(db, id, None).await.ok().flatten();
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

    let (target_isk, fill_isk) = economics(&items, &prices);

    cards.push(StockpileCard {
      character_id: entry.stockpile.character_id(),
      fill_isk,
      id,
      items,
      location_id,
      location_name,
      name: entry.stockpile.name().to_owned(),
      overall_pct,
      target_isk,
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
  expanded: &HashSet<i64>,
) -> Element<'a, Message> {
  let list = container(
    scrollable(card_grid::view(cards, expanded))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill);

  let mut layers: Vec<Element<'a, Message>> = vec![list.into()];
  if let Some(editor) = editor {
    layers.push(backdrop::backdrop(Message::StockpileEditorClosed));
    layers.push(editor_form(editor));
  }
  if let Some(panel) = import {
    layers.push(backdrop::backdrop(Message::StockpileImportClosed));
    layers.push(import_overlay(panel));
  }

  if layers.len() == 1 {
    layers.pop().unwrap()
  } else {
    Stack::with_children(layers)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }
}

pub(super) fn context_menu_view(menu: &StockpileContextMenu) -> Element<'_, Message> {
  let items = vec![
    Item::action("Edit", Message::StockpileEditStarted(menu.id)),
    Item::action("Export to Multibuy", Message::StockpileMultibuyExportOpened(menu.id)),
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

fn modal_close_button<'a>(close: Message) -> Element<'a, Message> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 1.0,
    bottom: spacing::UNIT + 1.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .on_press(close)
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      text_color: color::text::SECONDARY,
      ..button::Style::default()
    }
  })
  .into()
}

fn modal_footer<'a>(left: Option<Element<'a, Message>>, actions: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(actions.len() + 2);
  if let Some(left) = left {
    children.push(left);
  }
  children.push(Space::new().width(Length::Fill).into());
  children.extend(actions);

  modal_section(
    Row::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center)
      .width(Length::Fill)
      .into(),
  )
}

fn modal_header<'a>(title: &'a str, subtitle: &'a str, close: Message) -> Element<'a, Message> {
  let titles = Column::with_children(vec![
    text(title)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    eyebrow(subtitle, Some(color::text::SECONDARY)),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  modal_section(
    Row::with_children(vec![titles.into(), modal_close_button(close)])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill)
      .into(),
  )
}

fn modal_overlay<'a>(panel: Element<'a, Message>) -> Element<'a, Message> {
  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn modal_panel<'a>(width: f32, sections: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  container(Column::with_children(sections).width(Length::Fill))
    .width(Length::Fixed(width))
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

fn modal_section<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: MODAL_PAD_Y,
      bottom: MODAL_PAD_Y,
      left: MODAL_PAD_X,
      right: MODAL_PAD_X,
    })
    .into()
}

fn secondary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT + 3.0,
    right: spacing::SPACE_3_5,
    bottom: spacing::UNIT + 3.0,
    left: spacing::SPACE_3_5,
  })
  .on_press(message)
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.28),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn editor_form(editor: &Editor) -> Element<'_, Message> {
  let (title, subtitle, save_label) = if editor.is_editing() {
    ("Edit stockpile", "Adjust this target pile", "Save changes")
  } else {
    ("New stockpile", "Define a target pile", "Create stockpile")
  };

  let name_field = Column::with_children(vec![
    field_label("Name"),
    TextInput::new("Stockpile name", editor.name(), Message::StockpileEditorNameChanged)
      .font_size(typography::size::MD)
      .padding(spacing::SPACE_2)
      .render(),
  ])
  .spacing(spacing::UNIT + 1.0)
  .width(Length::Fill);

  let location_field = Column::with_children(vec![field_label("Location"), location_typeahead(editor)])
    .spacing(spacing::UNIT + 1.0)
    .width(Length::Fill);

  let fields = Row::with_children(vec![
    container(name_field).width(Length::FillPortion(1)).into(),
    container(location_field).width(Length::FillPortion(1)).into(),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill);

  let resolved = editor.items().iter().filter(|item| item.type_id.is_some()).count();
  let items_header = Row::with_children(vec![
    field_label("Items"),
    Space::new().width(Length::Fill).into(),
    text(format!("{resolved} type{}", if resolved == 1 { "" } else { "s" }))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    add_item_button(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let mut item_children: Vec<Element<'_, Message>> = Vec::with_capacity(editor.items().len() + 1);
  item_children.push(items_header.into());
  for (index, item) in editor.items().iter().enumerate() {
    item_children.push(editor_item_row(index, item));
  }
  let items_section = Column::with_children(item_children)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  let content_body = Column::with_children(vec![fields.into(), items_section.into()])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  let content = container(scrollable(content_body).style(crate::ui::style::control::scrollbar))
    .max_height(MODAL_CONTENT_MAX_HEIGHT)
    .width(Length::Fill)
    .padding(Padding {
      top: MODAL_PAD_Y,
      bottom: MODAL_PAD_Y,
      left: MODAL_PAD_X,
      right: MODAL_PAD_X,
    });

  let error: Option<Element<'_, Message>> = (!editor.error().is_empty()).then(|| {
    text(editor.error().to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      })
      .into()
  });

  let footer = modal_footer(
    error,
    vec![
      secondary_button("Cancel", Message::StockpileEditorClosed),
      primary_button(save_label, Message::StockpileEditorSaved),
    ],
  );

  modal_overlay(modal_panel(
    EDITOR_MODAL_WIDTH,
    vec![
      modal_header(title, subtitle, Message::StockpileEditorClosed),
      rule::horizontal(),
      content.into(),
      rule::horizontal(),
      footer,
    ],
  ))
}

fn editor_item_row(index: usize, item: &EditorItem) -> Element<'_, Message> {
  let (Some(type_id), Some(name)) = (item.type_id, &item.type_name) else {
    return item_typeahead(index, item);
  };

  let card = Row::with_children(vec![
    type_icon(type_id),
    text(name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    TextInput::new("Qty", &item.target, move |value| {
      Message::StockpileEditorItemTargetChanged(index, value)
    })
    .font_size(typography::size::SM)
    .padding(spacing::SPACE_2)
    .width(Length::Fixed(96.0))
    .render(),
    small_button(
      "\u{2715}",
      Message::StockpileEditorItemRemoved(index),
      color::status::DANGER,
    ),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(card)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT + 3.0,
      bottom: spacing::UNIT + 3.0,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
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

fn item_typeahead(index: usize, item: &EditorItem) -> Element<'_, Message> {
  let field = TextInput::new("Add item \u{2014} search name\u{2026}", &item.query, move |value| {
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
    "Search region, constellation, system, station, or structure\u{2026}",
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
      container(scrollable(column).style(crate::ui::style::control::scrollbar))
        .max_height(SUGGESTIONS_MAX_HEIGHT)
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
  match panel.resolution() {
    Some(resolution) => import_preview(resolution),
    None => import_paste(panel),
  }
}

pub(super) fn multibuy_export_overlay(card: &StockpileCard, mode: MultibuyMode, copied: bool) -> Element<'_, Message> {
  let lines = card.multibuy_lines(mode);
  let units: u64 = lines.iter().map(|(_, qty)| qty).sum();

  let controls = Row::with_children(vec![
    multibuy_mode_toggle(mode),
    Space::new().width(Length::Fill).into(),
    text(format!(
      "{} {} \u{b7} {} units",
      fmt_count(lines.len() as i64),
      if lines.len() == 1 { "line" } else { "lines" },
      fmt_count(units as i64),
    ))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into(),
  ])
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let preview: Element<'_, Message> = if lines.is_empty() {
    container(
      text("Nothing remaining \u{2014} this stockpile is fully stocked.")
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::status::ONLINE),
        }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(MULTIBUY_EXPORT_BODY_HEIGHT))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
  } else {
    multibuy_preview(&lines)
  };

  let content = modal_section(
    Column::with_children(vec![controls.into(), preview])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .into(),
  );

  let footer = modal_footer(
    Some(multibuy_footer(card.multibuy_value(mode))),
    vec![
      secondary_button("Close", Message::StockpileMultibuyExportClosed),
      multibuy_copy_button(card.id, copied, !lines.is_empty()),
    ],
  );

  modal_overlay(modal_panel(
    EXPORT_MODAL_WIDTH,
    vec![
      modal_header("Export to Multibuy", &card.name, Message::StockpileMultibuyExportClosed),
      rule::horizontal(),
      content,
      rule::horizontal(),
      footer,
    ],
  ))
}

fn multibuy_preview<'a>(lines: &[(String, u64)]) -> Element<'a, Message> {
  let rows: Vec<Element<'a, Message>> = lines
    .iter()
    .enumerate()
    .map(|(index, (name, qty))| {
      let row = container(
        Row::with_children(vec![
          text(name.clone())
            .font(typography::mono::REGULAR)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(color::text::PRIMARY),
            })
            .width(Length::Fill)
            .into(),
          text(format!("\u{d7}{}", fmt_count(*qty as i64)))
            .font(typography::mono::REGULAR)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(color::accent::PLASMA),
            })
            .into(),
        ])
        .spacing(spacing::SPACE_3)
        .align_y(Vertical::Center)
        .width(Length::Fill),
      )
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::UNIT + 3.0,
        bottom: spacing::UNIT + 3.0,
        left: spacing::SPACE_3_5,
        right: spacing::SPACE_3_5,
      });

      if index == 0 {
        row.into()
      } else {
        Column::with_children(vec![rule::horizontal_alpha(0.05), row.into()])
          .width(Length::Fill)
          .into()
      }
    })
    .collect();

  container(
    scrollable(Column::with_children(rows).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fixed(MULTIBUY_EXPORT_BODY_HEIGHT)),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn multibuy_footer<'a>(value: f64) -> Element<'a, Message> {
  Column::with_children(vec![
    text("est. value (ESI avg)")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    text(format!("{} ISK", fmt_isk(value)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .into()
}

fn multibuy_mode_toggle<'a>(mode: MultibuyMode) -> Element<'a, Message> {
  let segment = |label: &'a str, value: MultibuyMode| {
    let active = value == mode;
    button(
      text(label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(move |_| text::Style {
          color: Some(if active {
            color::accent::PLASMA
          } else {
            color::text::SECONDARY
          }),
        }),
    )
    .padding(Padding {
      top: spacing::UNIT + 1.0,
      bottom: spacing::UNIT + 1.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::StockpileMultibuyModeChanged(value))
    .style(move |_, status| segment_button_style(active, status))
    .into()
  };

  container(
    Row::with_children(vec![
      segment("Target", MultibuyMode::Target),
      segment("Remaining", MultibuyMode::Remaining),
    ])
    .spacing(0.0),
  )
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn multibuy_copy_button<'a>(card_id: i64, copied: bool, enabled: bool) -> Element<'a, Message> {
  let label = if copied { "Copied!" } else { "Copy" };
  let tint = if enabled {
    color::accent::PLASMA
  } else {
    color::text::TERTIARY
  };

  let content = Row::with_children(vec![
    Icon::copy().size(typography::size::MD).color(tint).render(),
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let mut export = button(content)
    .padding(Padding {
      top: spacing::UNIT + 3.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::UNIT + 3.0,
      left: spacing::SPACE_3_5,
    })
    .style(move |_, _| button::Style {
      background: Some(Background::Color(color::with_alpha(tint, 0.12))),
      border: Border {
        color: color::with_alpha(tint, 0.35),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    });
  if enabled {
    export = export.on_press(Message::StockpileMultibuyExportCopied(card_id));
  }
  export.into()
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
    "Paste an in-game multibuy or inventory list",
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
    "Confirm the matched items",
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
  subtitle: &'a str,
  hint: &'a str,
  field: Element<'a, Message>,
  action_label: &'a str,
  action_msg: Option<Message>,
) -> Element<'a, Message> {
  let mut action = primary_button_owned(action_label.to_owned());
  if let Some(msg) = action_msg {
    action = action.on_press(msg);
  }

  let content = modal_section(
    Column::with_children(vec![
      text(hint.to_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      field,
    ])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into(),
  );

  let footer = modal_footer(
    None,
    vec![
      secondary_button("Cancel", Message::StockpileImportClosed),
      action.into(),
    ],
  );

  modal_overlay(modal_panel(
    IMPORT_PANEL_WIDTH,
    vec![
      modal_header(title, subtitle, Message::StockpileImportClosed),
      rule::horizontal(),
      content,
      rule::horizontal(),
      footer,
    ],
  ))
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
        fill_isk: 0.0,
        id: 1,
        items: vec![],
        location_id: Some(60_003_760),
        location_name: Some("Jita IV".to_owned()),
        name: "Cache".to_owned(),
        overall_pct: 0.0,
        target_isk: 0.0,
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

  mod multibuy_deficit {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card(items: Vec<StockpileItemLine>) -> StockpileCard {
      StockpileCard {
        character_id: None,
        fill_isk: 0.0,
        id: 1,
        items,
        location_id: None,
        location_name: None,
        name: "Cache".to_owned(),
        overall_pct: 0.0,
        target_isk: 0.0,
      }
    }

    fn line(type_name: &str, have: i64, target: i64) -> StockpileItemLine {
      StockpileItemLine {
        have,
        pct: 0.0,
        target,
        type_id: 0,
        type_name: type_name.to_owned(),
      }
    }

    #[test]
    fn it_omits_items_at_or_above_target() {
      let card = card(vec![
        line("Tritanium", 1000, 1000),
        line("Pyerite", 80, 50),
        line("Mexallon", 10, 25),
      ]);

      assert_eq!(card.multibuy_deficit(), vec![("Mexallon".to_owned(), 15)]);
    }

    #[test]
    fn it_returns_an_empty_list_for_a_fully_stocked_stockpile() {
      let card = card(vec![line("Tritanium", 1000, 1000)]);

      assert!(card.multibuy_deficit().is_empty());
    }

    #[test]
    fn it_returns_target_minus_have_for_positive_deficits() {
      let card = card(vec![line("Tritanium", 400, 1000)]);

      assert_eq!(card.multibuy_deficit(), vec![("Tritanium".to_owned(), 600)]);
    }
  }

  mod multibuy_modes {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card(items: Vec<StockpileItemLine>, fill_isk: f64, target_isk: f64) -> StockpileCard {
      StockpileCard {
        character_id: None,
        fill_isk,
        id: 1,
        items,
        location_id: None,
        location_name: None,
        name: "Cache".to_owned(),
        overall_pct: 0.0,
        target_isk,
      }
    }

    fn line(type_name: &str, have: i64, target: i64) -> StockpileItemLine {
      StockpileItemLine {
        have,
        pct: 0.0,
        target,
        type_id: 0,
        type_name: type_name.to_owned(),
      }
    }

    #[test]
    fn it_lists_full_targets_in_target_mode() {
      let card = card(vec![line("Tritanium", 400, 1000), line("Pyerite", 50, 50)], 0.0, 0.0);

      assert_eq!(
        card.multibuy_lines(MultibuyMode::Target),
        vec![("Tritanium".to_owned(), 1000), ("Pyerite".to_owned(), 50)]
      );
    }

    #[test]
    fn it_omits_zero_target_items_in_target_mode() {
      let card = card(vec![line("Tritanium", 0, 0), line("Pyerite", 0, 50)], 0.0, 0.0);

      assert_eq!(card.multibuy_target(), vec![("Pyerite".to_owned(), 50)]);
    }

    #[test]
    fn it_lists_only_shortfalls_in_remaining_mode() {
      let card = card(vec![line("Tritanium", 400, 1000), line("Pyerite", 50, 50)], 0.0, 0.0);

      assert_eq!(
        card.multibuy_lines(MultibuyMode::Remaining),
        vec![("Tritanium".to_owned(), 600)]
      );
    }

    #[test]
    fn it_reports_target_value_in_target_mode_and_fill_value_in_remaining_mode() {
      let card = card(vec![line("Tritanium", 400, 1000)], 3600.0, 6000.0);

      assert_eq!(card.multibuy_value(MultibuyMode::Target), 6000.0);
      assert_eq!(card.multibuy_value(MultibuyMode::Remaining), 3600.0);
    }
  }

  mod short_items {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card(items: Vec<StockpileItemLine>) -> StockpileCard {
      StockpileCard {
        character_id: None,
        fill_isk: 0.0,
        id: 1,
        items,
        location_id: None,
        location_name: None,
        name: "Cache".to_owned(),
        overall_pct: 0.0,
        target_isk: 0.0,
      }
    }

    fn line(have: i64, target: i64) -> StockpileItemLine {
      StockpileItemLine {
        have,
        pct: 0.0,
        target,
        type_id: 34,
        type_name: "Tritanium".to_owned(),
      }
    }

    #[test]
    fn it_counts_only_items_below_target() {
      let card = card(vec![line(1000, 1000), line(80, 50), line(10, 25), line(0, 100)]);

      assert_eq!(card.short_items(), 2);
    }

    #[test]
    fn it_reports_zero_for_a_full_pile() {
      let card = card(vec![line(1000, 1000), line(80, 50)]);

      assert_eq!(card.short_items(), 0);
    }
  }

  mod economics {
    use pretty_assertions::assert_eq;

    use super::*;

    fn line(type_id: i64, have: i64, target: i64) -> StockpileItemLine {
      StockpileItemLine {
        have,
        pct: 0.0,
        target,
        type_id,
        type_name: format!("Type {type_id}"),
      }
    }

    fn prices(entries: &[(i64, f64)]) -> HashMap<i64, f64> {
      entries.iter().copied().collect()
    }

    #[test]
    fn it_sums_target_value_from_average_price() {
      let items = vec![line(34, 0, 1000), line(35, 0, 50)];

      let (target_isk, _) = economics(&items, &prices(&[(34, 6.0), (35, 11.0)]));

      assert_eq!(target_isk, 1000.0 * 6.0 + 50.0 * 11.0);
    }

    #[test]
    fn it_sums_fill_value_over_short_items_only() {
      let items = vec![line(34, 400, 1000), line(35, 80, 50)];

      let (_, fill_isk) = economics(&items, &prices(&[(34, 6.0), (35, 11.0)]));

      assert_eq!(fill_isk, (1000.0 - 400.0) * 6.0);
    }

    #[test]
    fn it_reports_zero_fill_value_for_a_full_pile() {
      let items = vec![line(34, 1000, 1000), line(35, 80, 50)];

      let (_, fill_isk) = economics(&items, &prices(&[(34, 6.0), (35, 11.0)]));

      assert_eq!(fill_isk, 0.0);
    }

    #[test]
    fn it_treats_an_unpriced_type_as_zero_value() {
      let items = vec![line(34, 0, 1000)];

      let (target_isk, fill_isk) = economics(&items, &prices(&[]));

      assert_eq!(target_isk, 0.0);
      assert_eq!(fill_isk, 0.0);
    }
  }

  mod render {
    use super::*;

    fn card_model() -> StockpileCard {
      StockpileCard {
        character_id: None,
        fill_isk: 0.0,
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
        target_isk: 0.0,
      }
    }

    #[test]
    fn it_renders_the_card_grid_and_fill_status() {
      let cards = vec![card_model()];
      let _el: Element<'_, Message> = body(&cards, None, None, &HashSet::new());
    }

    #[test]
    fn it_renders_the_editor_form_with_a_typeahead_and_a_resolved_chip() {
      let mut editor = Editor::blank();
      editor.pick_location(60_003_760, "Jita IV".to_owned());
      editor.pick_item(0, 34, "Tritanium".to_owned());
      editor.add_item();
      editor.set_item_suggestions(1, vec![(35, "Pyerite".to_owned())]);

      let _el: Element<'_, Message> = body(&[], Some(&editor), None, &HashSet::new());
    }

    #[test]
    fn it_renders_the_import_paste_overlay() {
      let mut panel = ImportPanel::blank();
      panel.set_text("Tritanium 1000".to_owned());

      let _el: Element<'_, Message> = body(&[], None, Some(&panel), &HashSet::new());
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

      let _el: Element<'_, Message> = body(&[], None, Some(&panel), &HashSet::new());
    }

    #[test]
    fn it_renders_the_empty_state() {
      let _el: Element<'_, Message> = body(&[], None, None, &HashSet::new());
    }

    #[test]
    fn it_renders_the_multibuy_export_overlay_in_both_modes() {
      let card = card_model();

      let _remaining: Element<'_, Message> = multibuy_export_overlay(&card, MultibuyMode::Remaining, false);
      let _target: Element<'_, Message> = multibuy_export_overlay(&card, MultibuyMode::Target, false);
      let _copied: Element<'_, Message> = multibuy_export_overlay(&card, MultibuyMode::Remaining, true);
    }

    #[test]
    fn it_renders_the_multibuy_export_overlay_empty_state_when_fully_stocked() {
      let mut card = card_model();
      card.items[0].have = card.items[0].target;

      let _el: Element<'_, Message> = multibuy_export_overlay(&card, MultibuyMode::Remaining, false);
    }
  }
}
