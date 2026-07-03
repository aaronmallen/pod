mod editor;
mod index;

use std::{collections::HashMap, sync::Arc};

use iced::{
  Element, Length, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, button, container, scrollable, text},
};

use super::character_detail::{
  self, ContactsPage, LoadState,
  tabs::{
    contact_modal::{self, ContactModal, DeleteConfirm},
    contacts::{ContactFilter, ContactRow, ContactSort, SortColumn, SortDirection},
  },
};
use crate::{
  clients::esi,
  store::{
    Database,
    images::{self, ImageKind, ImageState},
    model::{CharacterContact, SyncList, SyncListContact},
    repo::{character, contact_sync as repo, infra, org},
  },
  ui::{
    components::{
      avatar::Avatar,
      button::Button,
      confirm_modal::confirm_modal,
      entity_search::EntityKind,
      icon::Icon,
      modal_overlay::{modal_layers, stable_overlay},
      rule,
    },
    style::{color, control, radius, spacing, typography},
  },
};

const BACK_BUTTON_SIZE: f32 = 36.0;
const CONTENT_MAX_WIDTH: f32 = 920.0;
const ESTIMATED_ROW_HEIGHT: f32 = 46.0;
const HEADER_GAP: f32 = 16.0;
const HEADER_TITLE_SIZE: f32 = 20.0;
const PILOT_AVATAR_RADIUS: f32 = 6.0;
const SCREEN_PADDING: f32 = 32.0;
const VIEWPORT_SLACK: f32 = 120.0;

#[derive(Clone, Debug)]
pub enum Message {
  ContactAdded(Result<SyncListContact, String>),
  Contacts(Box<character_detail::Message>),
  CreateList,
  EditorClosed,
  Exit,
  ListCreated(Result<SyncList, String>),
  ListDeleteCancelled,
  ListDeleteConfirmed,
  ListDeleteRequested(i64),
  ListOpened(i64),
  Loaded(Box<Snapshot>),
  NameChanged(String),
  Persisted(Result<(), String>),
  TargetToggled(i64),
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::ContactAdded(_) | Message::Loaded(_))
  }
}

#[derive(Clone, Debug)]
pub struct Pilot {
  character_id: i64,
  name: String,
  portrait: ImageState,
  subtitle: String,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
  lists: Vec<ListModel>,
  names: HashMap<i64, String>,
  pilots: Vec<Pilot>,
}

#[derive(Debug)]
pub struct State {
  contact_delete: Option<DeleteConfirm>,
  contact_filter: ContactFilter,
  contact_modal: Option<ContactModal>,
  contact_search_generation: u64,
  contact_sort: ContactSort,
  contacts: LoadState<ContactsPage>,
  contacts_query: String,
  editing: Option<i64>,
  list_delete: Option<i64>,
  lists: Vec<ListModel>,
  names: HashMap<i64, String>,
  pilots: Vec<Pilot>,
}

impl State {
  pub fn new() -> Self {
    State {
      contact_delete: None,
      contact_filter: ContactFilter::All,
      contact_modal: None,
      contact_search_generation: 0,
      contact_sort: ContactSort::default(),
      contacts: LoadState::Loading,
      contacts_query: String::new(),
      editing: None,
      list_delete: None,
      lists: Vec::new(),
      names: HashMap::new(),
      pilots: Vec::new(),
    }
  }

  pub fn contact_search_generation(&self) -> u64 {
    self.contact_search_generation
  }

  pub fn stale_images(&self) -> Vec<(ImageKind, i64)> {
    let pilot_keys = self.pilots.iter().filter_map(|pilot| pilot.portrait.stale_key());
    let contact_keys = match &self.contacts {
      LoadState::Loaded(page) => page.rows().iter().filter_map(|row| row.image.stale_key()).collect(),
      _ => Vec::new(),
    };
    pilot_keys.chain(contact_keys).collect()
  }

  fn editing_list(&self) -> Option<&ListModel> {
    let id = self.editing?;
    self.lists.iter().find(|list| list.id == id)
  }

  fn editing_list_mut(&mut self) -> Option<&mut ListModel> {
    let id = self.editing?;
    self.lists.iter_mut().find(|list| list.id == id)
  }

  fn entity_name(&self, entity_id: i64) -> String {
    self
      .names
      .get(&entity_id)
      .cloned()
      .unwrap_or_else(|| format!("#{entity_id}"))
  }
}

#[derive(Clone, Debug)]
struct ListModel {
  contacts: Vec<SyncListContact>,
  id: i64,
  name: String,
  target_ids: Vec<i64>,
}

pub fn load(db: &Database, esi: Arc<esi::Client>) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { Box::new(load_snapshot(&db, &esi).await) }, Message::Loaded)
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ContactAdded(result) => contact_added(state, result),
    Message::Contacts(message) => update_contacts(state, *message, db),
    Message::CreateList => {
      let db = db.clone();
      let name = t!("contact_sync.new_list_name").into_owned();
      Task::perform(
        async move { repo::create_list(&db, &name).await.map_err(|error| error.to_string()) },
        Message::ListCreated,
      )
    }
    Message::EditorClosed => {
      close_editor(state);
      Task::none()
    }
    Message::Exit => Task::none(),
    Message::ListCreated(result) => list_created(state, result),
    Message::ListDeleteCancelled => {
      state.list_delete = None;
      Task::none()
    }
    Message::ListDeleteConfirmed => list_delete_confirmed(state, db),
    Message::ListDeleteRequested(id) => {
      state.list_delete = Some(id);
      Task::none()
    }
    Message::ListOpened(id) => {
      open_editor(state, id);
      Task::none()
    }
    Message::Loaded(snapshot) => {
      let snapshot = *snapshot;
      state.lists = snapshot.lists;
      state.names = snapshot.names;
      state.pilots = snapshot.pilots;
      refresh_contacts(state);
      Task::none()
    }
    Message::NameChanged(name) => name_changed(state, name, db),
    Message::Persisted(result) => {
      if let Err(error) = result {
        tracing::warn!(target: "pod::contact_sync", %error, "sync list write failed");
      }
      Task::none()
    }
    Message::TargetToggled(character_id) => target_toggled(state, character_id, db),
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let editing = state.editing_list();

  let body: Element<'_, Message> = match editing {
    Some(list) => editor::screen(state, list),
    None => index::screen(state),
  };

  let content = container(
    container(body)
      .width(Length::Fill)
      .max_width(CONTENT_MAX_WIDTH)
      .padding(SCREEN_PADDING),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Center);

  let scroll = scrollable(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(control::scrollbar);

  let base = container(
    Column::with_children(vec![view_header(editing), rule::horizontal(), scroll.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(iced::Background::Color(color::surface::BASE)),
    ..container::Style::default()
  });

  // Always render through the overlay `Stack` with `base` pinned at child[0], even with no
  // overlay active, so the list scrollable keeps its offset across modal open/close instead
  // of snapping to the top on the tree reshape.
  let layers = match view_overlay(state) {
    Some((dismiss, content)) => modal_layers(dismiss, content),
    None => Vec::new(),
  };
  stable_overlay(base.into(), layers)
}

fn back_button<'a>(message: Message) -> Element<'a, Message> {
  button(
    container(Icon::chevron_left().size(16.0).color(color::text::secondary()).render())
      .width(Length::Fixed(BACK_BUTTON_SIZE))
      .height(Length::Fixed(BACK_BUTTON_SIZE))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(message)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: iced::Border {
        color: if hover { color::rule_strong() } else { color::rule() },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if hover {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn close_editor(state: &mut State) {
  state.contact_delete = None;
  state.contact_modal = None;
  state.editing = None;
}

fn contact_added(state: &mut State, result: Result<SyncListContact, String>) -> Task<Message> {
  match result {
    Ok(contact) => {
      if let Some(list) = state.lists.iter_mut().find(|list| list.id == contact.list_id()) {
        // repo::add_contact upserts on (list_id, entity_type, entity_id), so an edit returns the same
        // row rather than a new one; replace it in place instead of pushing a duplicate.
        match list
          .contacts
          .iter_mut()
          .find(|row| row.entity_type() == contact.entity_type() && row.entity_id() == contact.entity_id())
        {
          Some(row) => *row = contact,
          None => list.contacts.push(contact),
        }
      }
      refresh_contacts(state);
    }
    Err(error) => tracing::warn!(target: "pod::contact_sync", %error, "sync list contact write failed"),
  }
  Task::none()
}

fn contact_delete_confirmed(state: &mut State, db: &Database) -> Task<Message> {
  let Some(confirm) = state.contact_delete.take() else {
    return Task::none();
  };
  let entity_type = confirm.contact.contact_type().clone();
  let entity_id = confirm.contact.contact_id();
  let Some(list) = state.editing_list_mut() else {
    return Task::none();
  };
  let Some(position) = list
    .contacts
    .iter()
    .position(|row| row.entity_type() == &entity_type && row.entity_id() == entity_id)
  else {
    return Task::none();
  };
  let row_id = list.contacts[position].id();
  list.contacts.remove(position);
  refresh_contacts(state);

  let db = db.clone();
  Task::perform(
    async move {
      repo::remove_contact(&db, row_id)
        .await
        .map_err(|error| error.to_string())
    },
    Message::Persisted,
  )
}

fn contact_submitted(state: &mut State, db: &Database) -> Task<Message> {
  let Some(modal) = state.contact_modal.take() else {
    return Task::none();
  };
  let Some(entity) = modal.entity().cloned() else {
    return Task::none();
  };
  let Some(list_id) = state.editing else {
    return Task::none();
  };

  state.names.insert(entity.id, entity.name.clone());
  let standing = modal.standing() as i64;
  let entity_type = entity_type_str(entity.kind).to_owned();
  let db = db.clone();
  Task::perform(
    async move {
      repo::add_contact(&db, list_id, &entity_type, entity.id, standing)
        .await
        .map_err(|error| error.to_string())
    },
    Message::ContactAdded,
  )
}

fn contact_view_rows(state: &State, list: &ListModel) -> Vec<ContactRow> {
  let query = state.contacts_query.trim().to_lowercase();
  let contact_type = state.contact_filter.contact_type();

  let mut rows: Vec<ContactRow> = list
    .contacts
    .iter()
    .filter(|contact| contact_type.is_none_or(|kind| contact.entity_type() == kind))
    .map(|contact| synthesize_row(state, contact))
    .filter(|row| query.is_empty() || row.contact.contact_name().to_lowercase().contains(&query))
    .collect();

  sort_rows(&mut rows, state.contact_sort);
  rows
}

fn entity_type_str(kind: EntityKind) -> &'static str {
  match kind {
    EntityKind::Alliance => "alliance",
    EntityKind::Corporation => "corporation",
    // Sync list contacts only support the ESI contact types (character/corporation/alliance); the
    // entity picker also allows a solar system or station, so those are stored as "character".
    EntityKind::Character | EntityKind::SolarSystem | EntityKind::Station => "character",
  }
}

fn image_kind(entity_type: &str) -> ImageKind {
  match entity_type {
    "alliance" => ImageKind::AllianceLogo,
    "corporation" => ImageKind::CorporationLogo,
    _ => ImageKind::CharacterPortrait,
  }
}

fn list_created(state: &mut State, result: Result<SyncList, String>) -> Task<Message> {
  match result {
    Ok(list) => {
      let id = list.id();
      state.lists.push(ListModel {
        contacts: Vec::new(),
        id,
        name: list.name().clone(),
        target_ids: Vec::new(),
      });
      open_editor(state, id);
    }
    Err(error) => tracing::warn!(target: "pod::contact_sync", %error, "sync list create failed"),
  }
  Task::none()
}

fn list_delete_confirmed(state: &mut State, db: &Database) -> Task<Message> {
  let Some(id) = state.list_delete.take() else {
    return Task::none();
  };
  state.lists.retain(|list| list.id != id);
  if state.editing == Some(id) {
    close_editor(state);
  }

  let db = db.clone();
  Task::perform(
    async move { repo::delete_list(&db, id).await.map_err(|error| error.to_string()) },
    Message::Persisted,
  )
}

async fn load_names(db: &Database, esi: &esi::Client, lists: &[ListModel], pilots: &[Pilot]) -> HashMap<i64, String> {
  let mut names: HashMap<i64, String> = pilots
    .iter()
    .map(|pilot| (pilot.character_id, pilot.name.clone()))
    .collect();
  if let Ok(corporations) = org::corporation_names(db).await {
    names.extend(corporations);
  }
  if let Ok(alliances) = org::all_alliances(db).await {
    names.extend(
      alliances
        .into_iter()
        .map(|alliance| (alliance.id(), alliance.name().clone())),
    );
  }

  let missing: Vec<i64> = lists
    .iter()
    .flat_map(|list| list.contacts.iter())
    .map(SyncListContact::entity_id)
    .filter(|id| !names.contains_key(id))
    .collect();
  // ESI caps /universe/names at 1000 ids per request.
  for chunk in missing.chunks(1_000) {
    match esi.universe().names(chunk).await {
      Ok(records) => names.extend(records.into_iter().map(|record| (record.id, record.name))),
      Err(error) => {
        tracing::warn!(target: "pod::contact_sync", %error, "sync list name resolution failed");
      }
    }
  }
  names
}

async fn load_pilots(db: &Database) -> Vec<Pilot> {
  let owned = character::all_owned(db).await.unwrap_or_default();
  let tags = infra::tag_all(db).await.unwrap_or_default();
  let memberships = infra::memberships(db, "character").await.unwrap_or_default();
  let tag_names: HashMap<i64, &str> = tags.iter().map(|tag| (tag.id(), tag.name().as_str())).collect();
  let store = images::default_store();

  let mut pilots = Vec::with_capacity(owned.len());
  for character in owned {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|corp| corp.name().clone())
      .unwrap_or_default();
    let assigned: Vec<&str> = memberships
      .iter()
      .filter(|membership| membership.entity_id() == character.id())
      .filter_map(|membership| tag_names.get(&membership.tag_id()).copied())
      .take(3)
      .collect();
    let subtitle = if assigned.is_empty() {
      corp
    } else {
      assigned.join(" \u{b7} ")
    };
    pilots.push(Pilot {
      character_id: character.id(),
      name: character.name().clone(),
      portrait: images::resolve(&store, ImageKind::CharacterPortrait, character.id()),
      subtitle,
    });
  }
  pilots.sort_by_key(|pilot| pilot.name.to_lowercase());
  pilots
}

async fn load_snapshot(db: &Database, esi: &esi::Client) -> Snapshot {
  let mut lists = Vec::new();
  for list in repo::lists(db).await.unwrap_or_default() {
    let contacts = repo::list_contacts(db, list.id()).await.unwrap_or_default();
    let targets = repo::list_targets(db, list.id()).await.unwrap_or_default();
    lists.push(ListModel {
      contacts,
      id: list.id(),
      name: list.name().clone(),
      target_ids: targets.iter().map(|target| target.character_id()).collect(),
    });
  }

  let pilots = load_pilots(db).await;
  let names = load_names(db, esi, &lists, &pilots).await;

  Snapshot {
    lists,
    names,
    pilots,
  }
}

fn name_changed(state: &mut State, name: String, db: &Database) -> Task<Message> {
  let Some(list) = state.editing_list_mut() else {
    return Task::none();
  };
  list.name = name.clone();
  let id = list.id;

  let db = db.clone();
  Task::perform(
    async move {
      repo::rename_list(&db, id, &name)
        .await
        .map_err(|error| error.to_string())
    },
    Message::Persisted,
  )
}

fn open_editor(state: &mut State, id: i64) {
  state.contact_filter = ContactFilter::All;
  state.contact_sort = ContactSort::default();
  state.contacts_query.clear();
  state.editing = Some(id);
  refresh_contacts(state);
}

fn pilot_avatar<'a>(pilot: &Pilot, size: f32, ring: Option<iced::Color>) -> Element<'a, Message> {
  let mut avatar = Avatar::new(
    pilot.character_id,
    pilot.name.clone(),
    Length::Fixed(size),
    size,
    pilot.portrait.path(),
  )
  .radius(PILOT_AVATAR_RADIUS);
  if let Some(color) = ring {
    avatar = avatar.border(color, 2.0);
  }
  avatar.view()
}

fn refresh_contacts(state: &mut State) {
  let rows = match state.editing_list() {
    Some(list) => contact_view_rows(state, list),
    None => Vec::new(),
  };
  state.contacts = LoadState::Loaded(ContactsPage::unpaged(rows));
}

fn sort_rows(rows: &mut [ContactRow], sort: ContactSort) {
  rows.sort_by(|a, b| {
    let ordering = match sort.column {
      SortColumn::Entity => a
        .contact
        .contact_name()
        .to_lowercase()
        .cmp(&b.contact.contact_name().to_lowercase()),
      SortColumn::Standing => a
        .contact
        .standing()
        .partial_cmp(&b.contact.standing())
        .unwrap_or(std::cmp::Ordering::Equal),
      SortColumn::Type => a.contact.contact_type().cmp(b.contact.contact_type()),
    };
    match sort.direction {
      SortDirection::Ascending => ordering,
      SortDirection::Descending => ordering.reverse(),
    }
  });
}

fn synthesize_row(state: &State, contact: &SyncListContact) -> ContactRow {
  let kind = image_kind(contact.entity_type());
  ContactRow {
    contact: CharacterContact {
      character_id: 0, // sync list contacts have no owning character; the field is unused here
      contact_id: contact.entity_id(),
      contact_name: state.entity_name(contact.entity_id()),
      contact_type: contact.entity_type().clone(),
      is_blocked: false,
      is_watched: false,
      label_ids: "[]".to_owned(),
      standing: contact.standing() as f64,
    },
    image: images::resolve(&images::default_store(), kind, contact.entity_id()),
  }
}

fn target_toggled(state: &mut State, character_id: i64, db: &Database) -> Task<Message> {
  let Some(list) = state.editing_list_mut() else {
    return Task::none();
  };
  match list.target_ids.iter().position(|id| *id == character_id) {
    Some(position) => {
      list.target_ids.remove(position);
    }
    None => list.target_ids.push(character_id),
  }
  let id = list.id;
  let targets = list.target_ids.clone();

  let db = db.clone();
  Task::perform(
    async move {
      repo::set_targets(&db, id, &targets)
        .await
        .map_err(|error| error.to_string())
    },
    Message::Persisted,
  )
}

fn update_contacts(state: &mut State, message: character_detail::Message, db: &Database) -> Task<Message> {
  use character_detail::Message as Detail;

  match message {
    Detail::ContactAddOpened => {
      let exclude: Vec<String> = state
        .editing_list()
        .map(|list| {
          list
            .contacts
            .iter()
            .map(|contact| state.entity_name(contact.entity_id()))
            .collect()
        })
        .unwrap_or_default();
      state.contact_modal = Some(ContactModal::add(exclude, Vec::new()).without_watch());
      Task::none()
    }
    Detail::ContactDeleteCancelled => {
      state.contact_delete = None;
      Task::none()
    }
    Detail::ContactDeleteConfirmed => contact_delete_confirmed(state, db),
    Detail::ContactDeleteRequested(contact) => {
      state.contact_delete = Some(DeleteConfirm {
        contact: *contact,
      });
      Task::none()
    }
    Detail::ContactEditOpened(contact) => {
      state.contact_modal = Some(ContactModal::edit(&contact, Vec::new()).without_watch());
      Task::none()
    }
    Detail::ContactEntityChanged(entity) => with_modal(state, |modal| modal.set_entity(entity)),
    Detail::ContactEntityInput(query) => {
      if let Some(modal) = state.contact_modal.as_mut() {
        state.contact_search_generation = modal.set_query(query);
      }
      Task::none()
    }
    Detail::ContactEntityResults {
      generation,
      results,
    } => with_modal(state, |modal| {
      modal.accept_results(generation, results);
    }),
    Detail::ContactFilterChanged(filter) => {
      state.contact_filter = filter;
      refresh_contacts(state);
      Task::none()
    }
    Detail::ContactModalClosed => {
      state.contact_modal = None;
      Task::none()
    }
    Detail::ContactModalSubmitted => contact_submitted(state, db),
    Detail::ContactSortChanged(sort) => {
      state.contact_sort = sort;
      refresh_contacts(state);
      Task::none()
    }
    Detail::ContactStandingChanged(standing) => with_modal(state, |modal| modal.set_standing(standing)),
    Detail::ContactsSearchChanged(query) => {
      state.contacts_query = query;
      refresh_contacts(state);
      Task::none()
    }
    Detail::ContactsSearchCleared => {
      state.contacts_query.clear();
      refresh_contacts(state);
      Task::none()
    }
    _ => Task::none(),
  }
}

fn view_header<'a>(editing: Option<&'a ListModel>) -> Element<'a, Message> {
  let (title, back, action): (String, Message, Element<'a, Message>) = match editing {
    Some(list) => {
      let title = if list.name.trim().is_empty() {
        t!("contact_sync.untitled").into_owned()
      } else {
        list.name.clone()
      };
      (
        title,
        Message::EditorClosed,
        Button::secondary(t!("contact_sync.done"))
          .on_press(Message::EditorClosed)
          .into(),
      )
    }
    None => (
      t!("contact_sync.title").into_owned(),
      Message::Exit,
      Button::primary(t!("contact_sync.new_list"))
        .icon(Icon::plus())
        .on_press(Message::CreateList)
        .into(),
    ),
  };

  let eyebrow = text(t!("contact_sync.eyebrow").to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::accent()),
    });
  let heading = text(title)
    .font(typography::body::MEDIUM)
    .size(HEADER_TITLE_SIZE)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let identity = Column::with_children(vec![eyebrow.into(), heading.into()])
    .spacing(2.0)
    .width(Length::Fill);

  let row = Row::with_children(vec![back_button(back), identity.into(), action])
    .spacing(HEADER_GAP)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
    .padding(iced::Padding {
      top: 0.0,
      right: SCREEN_PADDING,
      bottom: 0.0,
      left: SCREEN_PADDING,
    })
    .align_y(Vertical::Center)
    .into()
}

fn view_overlay(state: &State) -> Option<(Message, Element<'_, Message>)> {
  if let Some(id) = state.list_delete {
    let name = state
      .lists
      .iter()
      .find(|list| list.id == id)
      .map(|list| list.name.clone())
      .unwrap_or_default();
    return Some((
      Message::ListDeleteCancelled,
      confirm_modal(
        t!("contact_sync.delete_eyebrow").into_owned(),
        t!("contact_sync.delete_title", name => name).into_owned(),
        t!("contact_sync.delete_body").into_owned(),
        t!("contact_sync.delete_confirm").into_owned(),
        Message::ListDeleteConfirmed,
        Message::ListDeleteCancelled,
      ),
    ));
  }

  if let Some(confirm) = state.contact_delete.as_ref() {
    return Some((
      Message::Contacts(Box::new(character_detail::Message::ContactDeleteCancelled)),
      contact_modal::delete_confirm(confirm).map(|message| Message::Contacts(Box::new(message))),
    ));
  }

  if let Some(modal) = state.contact_modal.as_ref() {
    return Some((
      Message::Contacts(Box::new(character_detail::Message::ContactModalClosed)),
      contact_modal::modal(modal).map(|message| Message::Contacts(Box::new(message))),
    ));
  }

  None
}

fn with_modal(state: &mut State, edit: impl FnOnce(&mut ContactModal)) -> Task<Message> {
  if let Some(modal) = state.contact_modal.as_mut() {
    edit(modal);
  }
  Task::none()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64, name: &str) {
    let corp_id = 90_000_000 + id;
    let alliance_id = 99_000_000 + id;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
    let far_future = chrono::Utc::now().timestamp() + 86_400;
    infra::upsert(
      db,
      id,
      crate::store::model::OwnerType::Character,
      "tok",
      "rt",
      far_future,
      None,
      None,
    )
    .await
    .unwrap();
  }

  fn list_model(id: i64, name: &str, contacts: Vec<SyncListContact>, target_ids: Vec<i64>) -> ListModel {
    ListModel {
      contacts,
      id,
      name: name.to_owned(),
      target_ids,
    }
  }

  fn pilot(character_id: i64, name: &str) -> Pilot {
    Pilot {
      character_id,
      name: name.to_owned(),
      portrait: images::resolve(&images::default_store(), ImageKind::CharacterPortrait, character_id),
      subtitle: "Test Corp".to_owned(),
    }
  }

  mod update_flow {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_opens_the_editor_for_a_created_list() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let list = crate::store::repo::contact_sync::create_list(&db, "Gankers")
        .await
        .unwrap();

      let _ = update(&mut state, Message::ListCreated(Ok(list)), &db);

      assert_eq!(state.editing, state.lists.first().map(|list| list.id));
      assert_eq!(state.lists[0].name, "Gankers");
    }

    #[tokio::test]
    async fn it_renames_the_editing_list_in_memory() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.lists.push(list_model(1, "Old", Vec::new(), Vec::new()));
      state.editing = Some(1);

      let _ = update(&mut state, Message::NameChanged("New".to_owned()), &db);

      assert_eq!(state.lists[0].name, "New");
    }

    #[tokio::test]
    async fn it_toggles_targets_in_memory() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.lists.push(list_model(1, "Blues", Vec::new(), Vec::new()));
      state.editing = Some(1);

      let _ = update(&mut state, Message::TargetToggled(42), &db);
      assert_eq!(state.lists[0].target_ids, vec![42]);

      let _ = update(&mut state, Message::TargetToggled(42), &db);
      assert!(state.lists[0].target_ids.is_empty());
    }

    #[tokio::test]
    async fn it_removes_a_deleted_list_and_closes_its_editor() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.lists.push(list_model(1, "Blues", Vec::new(), Vec::new()));
      state.editing = Some(1);

      let _ = update(&mut state, Message::ListDeleteRequested(1), &db);
      let _ = update(&mut state, Message::ListDeleteConfirmed, &db);

      assert!(state.lists.is_empty());
      assert_eq!(state.editing, None);
      assert_eq!(state.list_delete, None);
    }

    #[tokio::test]
    async fn it_cancels_a_pending_list_delete() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      state.lists.push(list_model(1, "Blues", Vec::new(), Vec::new()));

      let _ = update(&mut state, Message::ListDeleteRequested(1), &db);
      let _ = update(&mut state, Message::ListDeleteCancelled, &db);

      assert_eq!(state.list_delete, None);
      assert_eq!(state.lists.len(), 1);
    }

    #[tokio::test]
    async fn it_logs_and_keeps_state_on_a_failed_write() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::Persisted(Err("boom".to_owned())), &db);
      let _ = update(&mut state, Message::ListCreated(Err("boom".to_owned())), &db);
      let _ = update(&mut state, Message::ContactAdded(Err("boom".to_owned())), &db);
      let _ = update(&mut state, Message::Exit, &db);

      assert!(state.lists.is_empty());
    }

    #[tokio::test]
    async fn it_installs_a_loaded_snapshot() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();
      let snapshot = Snapshot {
        lists: vec![list_model(1, "Blues", Vec::new(), vec![42])],
        names: HashMap::from([(42, "Pilot".to_owned())]),
        pilots: vec![pilot(42, "Pilot")],
      };

      let _ = update(&mut state, Message::Loaded(Box::new(snapshot)), &db);

      assert_eq!(state.lists.len(), 1);
      assert_eq!(state.entity_name(42), "Pilot");
      assert_eq!(state.entity_name(43), "#43");
    }

    #[tokio::test]
    async fn it_upserts_an_added_contact_and_refreshes_the_table() {
      let db = store::open_test().await.unwrap();
      let list = crate::store::repo::contact_sync::create_list(&db, "Gankers")
        .await
        .unwrap();
      let first = crate::store::repo::contact_sync::add_contact(&db, list.id(), "character", 7, -5)
        .await
        .unwrap();
      let mut state = State::new();
      state
        .lists
        .push(list_model(list.id(), "Gankers", Vec::new(), Vec::new()));
      state.editing = Some(list.id());

      let _ = update(&mut state, Message::ContactAdded(Ok(first)), &db);
      assert_eq!(state.lists[0].contacts.len(), 1);
      assert_eq!(state.lists[0].contacts[0].standing(), -5);

      let replaced = crate::store::repo::contact_sync::add_contact(&db, list.id(), "character", 7, -10)
        .await
        .unwrap();
      let _ = update(&mut state, Message::ContactAdded(Ok(replaced)), &db);
      assert_eq!(state.lists[0].contacts.len(), 1);
      assert_eq!(state.lists[0].contacts[0].standing(), -10);

      match &state.contacts {
        LoadState::Loaded(page) => assert_eq!(page.rows().len(), 1),
        other => panic!("expected a loaded contacts page, got {other:?}"),
      }
    }
  }

  mod contacts_editor {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ui::components::entity_search::EntityRef;

    fn detail(message: character_detail::Message) -> Message {
      Message::Contacts(Box::new(message))
    }

    async fn editing_state(db: &Database) -> State {
      let list = crate::store::repo::contact_sync::create_list(db, "Gankers")
        .await
        .unwrap();
      let contact = crate::store::repo::contact_sync::add_contact(db, list.id(), "character", 7, -10)
        .await
        .unwrap();
      let mut state = State::new();
      state.names.insert(7, "Lazar Khane".to_owned());
      state
        .lists
        .push(list_model(list.id(), "Gankers", vec![contact], Vec::new()));
      open_editor(&mut state, list.id());
      state
    }

    #[tokio::test]
    async fn it_filters_and_sorts_the_table_from_reused_messages() {
      let db = store::open_test().await.unwrap();
      let mut state = editing_state(&db).await;

      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactsSearchChanged("laz".to_owned())),
        &db,
      );
      match &state.contacts {
        LoadState::Loaded(page) => assert_eq!(page.rows().len(), 1),
        other => panic!("expected a loaded contacts page, got {other:?}"),
      }

      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactsSearchChanged("zzz".to_owned())),
        &db,
      );
      match &state.contacts {
        LoadState::Loaded(page) => assert!(page.rows().is_empty()),
        other => panic!("expected a loaded contacts page, got {other:?}"),
      }

      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactsSearchCleared),
        &db,
      );
      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactFilterChanged(ContactFilter::Corp)),
        &db,
      );
      match &state.contacts {
        LoadState::Loaded(page) => assert!(page.rows().is_empty(), "no corporations in the list"),
        other => panic!("expected a loaded contacts page, got {other:?}"),
      }

      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactSortChanged(ContactSort {
          column: SortColumn::Entity,
          direction: SortDirection::Ascending,
        })),
        &db,
      );
      assert_eq!(state.contact_sort.column, SortColumn::Entity);
    }

    #[tokio::test]
    async fn it_opens_add_and_edit_modals_without_the_watch_field() {
      let db = store::open_test().await.unwrap();
      let mut state = editing_state(&db).await;

      let _ = update(&mut state, detail(character_detail::Message::ContactAddOpened), &db);
      assert!(state.contact_modal.is_some());

      let _ = update(&mut state, detail(character_detail::Message::ContactModalClosed), &db);
      assert!(state.contact_modal.is_none());

      let row = match &state.contacts {
        LoadState::Loaded(page) => page.rows()[0].contact.clone(),
        other => panic!("expected a loaded contacts page, got {other:?}"),
      };
      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactEditOpened(Box::new(row))),
        &db,
      );
      assert!(state.contact_modal.as_ref().is_some_and(ContactModal::is_edit));
    }

    #[tokio::test]
    async fn it_submits_a_picked_entity_at_a_snapped_standing() {
      let db = store::open_test().await.unwrap();
      let mut state = editing_state(&db).await;
      let _ = update(&mut state, detail(character_detail::Message::ContactAddOpened), &db);

      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactEntityInput("Vex".to_owned())),
        &db,
      );
      let generation = state.contact_search_generation();
      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactEntityResults {
          generation,
          results: vec![EntityRef {
            id: 95,
            kind: EntityKind::Character,
            name: "Vex Voronova".to_owned(),
            portrait: None,
          }],
        }),
        &db,
      );
      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactEntityChanged(Some(EntityRef {
          id: 95,
          kind: EntityKind::Character,
          name: "Vex Voronova".to_owned(),
          portrait: None,
        }))),
        &db,
      );
      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactStandingChanged(7.0)),
        &db,
      );
      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactModalSubmitted),
        &db,
      );

      assert!(state.contact_modal.is_none());
      assert_eq!(state.entity_name(95), "Vex Voronova");
    }

    #[tokio::test]
    async fn it_deletes_a_contact_through_the_reused_confirm() {
      let db = store::open_test().await.unwrap();
      let mut state = editing_state(&db).await;
      let row = match &state.contacts {
        LoadState::Loaded(page) => page.rows()[0].contact.clone(),
        other => panic!("expected a loaded contacts page, got {other:?}"),
      };

      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactDeleteRequested(Box::new(row))),
        &db,
      );
      assert!(state.contact_delete.is_some());

      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactDeleteConfirmed),
        &db,
      );

      assert!(state.contact_delete.is_none());
      assert!(state.lists[0].contacts.is_empty());
    }

    #[tokio::test]
    async fn it_ignores_messages_that_do_not_apply_to_sync_lists() {
      let db = store::open_test().await.unwrap();
      let mut state = editing_state(&db).await;

      let _ = update(&mut state, detail(character_detail::Message::ContactWatchToggled), &db);
      let _ = update(
        &mut state,
        detail(character_detail::Message::ContactDeleteCancelled),
        &db,
      );

      assert!(state.contact_modal.is_none());
    }
  }

  mod load {
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::clients::http;

    #[tokio::test]
    async fn it_loads_lists_pilots_and_resolves_missing_names() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"id":71001,"name":"Lazar Khane","category":"character"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http, server.uri());
      seed_character(&db, 42, "Pilot One").await;
      let list = crate::store::repo::contact_sync::create_list(&db, "Gankers")
        .await
        .unwrap();
      crate::store::repo::contact_sync::add_contact(&db, list.id(), "character", 71_001, -10)
        .await
        .unwrap();
      crate::store::repo::contact_sync::set_targets(&db, list.id(), &[42])
        .await
        .unwrap();

      let snapshot = load_snapshot(&db, &esi).await;

      assert_eq!(snapshot.lists.len(), 1);
      assert_eq!(snapshot.lists[0].target_ids, vec![42]);
      assert_eq!(snapshot.pilots.len(), 1);
      assert_eq!(snapshot.pilots[0].subtitle, "Test Corp");
      assert_eq!(snapshot.names.get(&71_001).map(String::as_str), Some("Lazar Khane"));
      assert_eq!(snapshot.names.get(&42).map(String::as_str), Some("Pilot One"));
    }
  }

  mod render {
    use super::*;

    fn loaded_state() -> State {
      let mut state = State::new();
      state.names.insert(7, "Lazar Khane".to_owned());
      state.pilots = vec![pilot(42, "Pilot One"), pilot(43, "Pilot Two")];
      state.lists.push(list_model(1, "Gankers", Vec::new(), vec![42]));
      state.lists.push(list_model(2, "Blues", Vec::new(), Vec::new()));
      refresh_contacts(&mut state);
      state
    }

    #[test]
    fn it_renders_the_empty_index() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_index_cards_with_targets_and_without() {
      let state = loaded_state();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_editor_screen() {
      let mut state = loaded_state();
      open_editor(&mut state, 1);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_list_delete_confirm_overlay() {
      let mut state = loaded_state();
      state.list_delete = Some(1);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_contact_modal_overlay() {
      let mut state = loaded_state();
      open_editor(&mut state, 1);
      state.contact_modal = Some(ContactModal::add(Vec::new(), Vec::new()).without_watch());

      let _el: Element<'_, Message> = view(&state);
    }

    #[tokio::test]
    async fn it_renders_standing_tallies_and_pilot_overflow() {
      let db = store::open_test().await.unwrap();
      let list = crate::store::repo::contact_sync::create_list(&db, "Mixed")
        .await
        .unwrap();
      let mut contacts = Vec::new();
      for (entity_id, standing) in [(1, -10), (2, -5), (3, 0), (4, 5)] {
        contacts.push(
          crate::store::repo::contact_sync::add_contact(&db, list.id(), "character", entity_id, standing)
            .await
            .unwrap(),
        );
      }
      let mut singular = Vec::new();
      for (entity_id, standing) in [(5, -10), (6, 5), (7, 10)] {
        singular.push(
          crate::store::repo::contact_sync::add_contact(&db, list.id(), "character", entity_id, standing)
            .await
            .unwrap(),
        );
      }

      let mut state = State::new();
      state.pilots = (1..=6).map(|id| pilot(id, &format!("Pilot {id}"))).collect();
      state.lists.push(list_model(1, "Mixed", contacts, (1..=6).collect()));
      state.lists.push(list_model(2, "One Red", singular, vec![1]));

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_contact_delete_overlay() {
      let mut state = loaded_state();
      open_editor(&mut state, 1);
      state.contact_delete = Some(DeleteConfirm {
        contact: CharacterContact {
          character_id: 0,
          contact_id: 7,
          contact_name: "Lazar Khane".to_owned(),
          contact_type: "character".to_owned(),
          is_blocked: false,
          is_watched: false,
          label_ids: "[]".to_owned(),
          standing: -10.0,
        },
      });

      let _el: Element<'_, Message> = view(&state);
    }
  }

  mod helpers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_entity_kinds_and_image_kinds() {
      assert_eq!(entity_type_str(EntityKind::Alliance), "alliance");
      assert_eq!(entity_type_str(EntityKind::Corporation), "corporation");
      assert_eq!(entity_type_str(EntityKind::Character), "character");

      assert_eq!(image_kind("alliance"), ImageKind::AllianceLogo);
      assert_eq!(image_kind("corporation"), ImageKind::CorporationLogo);
      assert_eq!(image_kind("character"), ImageKind::CharacterPortrait);
    }

    fn row(name: &str, kind: &str, standing: f64) -> ContactRow {
      ContactRow {
        contact: CharacterContact {
          character_id: 0,
          contact_id: 1,
          contact_name: name.to_owned(),
          contact_type: kind.to_owned(),
          is_blocked: false,
          is_watched: false,
          label_ids: "[]".to_owned(),
          standing,
        },
        image: images::resolve(&images::default_store(), ImageKind::CharacterPortrait, 1),
      }
    }

    fn names(rows: &[ContactRow]) -> Vec<&str> {
      rows.iter().map(|row| row.contact.contact_name().as_str()).collect()
    }

    #[test]
    fn it_sorts_rows_by_each_column_and_direction() {
      let mut rows = vec![
        row("beta", "corporation", -10.0),
        row("Alpha", "character", 10.0),
        row("gamma", "alliance", 0.0),
      ];

      sort_rows(
        &mut rows,
        ContactSort {
          column: SortColumn::Entity,
          direction: SortDirection::Ascending,
        },
      );
      assert_eq!(names(&rows), ["Alpha", "beta", "gamma"]);

      sort_rows(
        &mut rows,
        ContactSort {
          column: SortColumn::Entity,
          direction: SortDirection::Descending,
        },
      );
      assert_eq!(names(&rows), ["gamma", "beta", "Alpha"]);

      sort_rows(
        &mut rows,
        ContactSort {
          column: SortColumn::Standing,
          direction: SortDirection::Descending,
        },
      );
      assert_eq!(names(&rows), ["Alpha", "gamma", "beta"]);

      sort_rows(
        &mut rows,
        ContactSort {
          column: SortColumn::Type,
          direction: SortDirection::Ascending,
        },
      );
      assert_eq!(names(&rows), ["gamma", "Alpha", "beta"]);
    }

    #[test]
    fn it_reports_data_loading_messages() {
      assert!(
        Message::Loaded(Box::new(Snapshot {
          lists: Vec::new(),
          names: HashMap::new(),
          pilots: Vec::new(),
        }))
        .loads_data()
      );
      assert!(!Message::CreateList.loads_data());
    }

    #[test]
    fn it_collects_stale_image_keys() {
      let mut state = State::new();
      state.pilots = vec![pilot(42, "Pilot One")];

      let _keys = state.stale_images();
    }
  }
}
