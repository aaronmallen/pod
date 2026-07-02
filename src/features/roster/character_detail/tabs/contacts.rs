use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, button, container, text},
};

use super::{
  super::{ContactsPage, LoadState, Message},
  shared,
};
use crate::{
  store::{images::ImageState, model::CharacterContact},
  ui::{
    components::{
      avatar::avatar,
      card,
      empty_state::{LoadStateView, load_state_view},
      eyebrow::eyebrow_text,
      icon::Icon,
      section_header::section_header,
      segmented::segment_button_style,
      text_input::TextInput,
      virtual_list::{VirtualList, VirtualListConfig},
    },
    style::{color, radius, spacing, typography},
  },
};

const ACTIONS_WIDTH: f32 = 70.0;
const ACTION_SIZE: f32 = 28.0;
const AVATAR_SIZE: f32 = 30.0;
const STANDING_WIDTH: f32 = 70.0;
const TYPE_WIDTH: f32 = 90.0;
const WATCHLIST_WIDTH: f32 = 80.0;

const ESTIMATED_ROW_HEIGHT: f32 = 46.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactColumns {
  pub labels: bool,
  pub watchlist: bool,
}

impl ContactColumns {
  pub fn standings_only() -> Self {
    ContactColumns {
      labels: false,
      watchlist: false,
    }
  }
}

impl Default for ContactColumns {
  fn default() -> Self {
    ContactColumns {
      labels: true,
      watchlist: true,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ContactFilter {
  #[default]
  All,
  Alliance,
  Character,
  Corp,
}

impl ContactFilter {
  const SEGMENTS: [(ContactFilter, &'static str); 4] = [
    (ContactFilter::All, "roster.contacts.filter_all"),
    (ContactFilter::Character, "roster.contacts.filter_characters"),
    (ContactFilter::Corp, "roster.contacts.filter_corps"),
    (ContactFilter::Alliance, "roster.contacts.filter_alliances"),
  ];

  pub(in crate::features::roster) fn contact_type(self) -> Option<&'static str> {
    match self {
      ContactFilter::All => None,
      ContactFilter::Character => Some("character"),
      ContactFilter::Corp => Some("corporation"),
      ContactFilter::Alliance => Some("alliance"),
    }
  }
}

#[derive(Clone, Debug)]
pub struct ContactRow {
  pub contact: CharacterContact,
  pub image: ImageState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactSort {
  pub column: SortColumn,
  pub direction: SortDirection,
}

impl ContactSort {
  pub fn toggled(self, column: SortColumn) -> Self {
    if self.column == column {
      ContactSort {
        column,
        direction: self.direction.toggled(),
      }
    } else {
      ContactSort {
        column,
        direction: column.natural_direction(),
      }
    }
  }

  fn caret(self, column: SortColumn) -> Option<SortDirection> {
    (self.column == column).then_some(self.direction)
  }
}

impl Default for ContactSort {
  fn default() -> Self {
    ContactSort {
      column: SortColumn::Standing,
      direction: SortDirection::Descending,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortColumn {
  Entity,
  Standing,
  Type,
}

impl SortColumn {
  fn natural_direction(self) -> SortDirection {
    match self {
      SortColumn::Entity | SortColumn::Type => SortDirection::Ascending,
      SortColumn::Standing => SortDirection::Descending,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
  Ascending,
  Descending,
}

impl SortDirection {
  fn toggled(self) -> Self {
    match self {
      SortDirection::Ascending => SortDirection::Descending,
      SortDirection::Descending => SortDirection::Ascending,
    }
  }
}

pub(in crate::features::roster) fn header<'a>(
  contacts: &LoadState<ContactsPage>,
  filter: ContactFilter,
  query: &'a str,
  write_enabled: bool,
) -> Element<'a, Message> {
  let (count, suffix) = match contacts {
    LoadState::Loaded(page) => (page.rows().len(), if page.has_more() { "+" } else { "" }),
    _ => (0, ""),
  };

  let mut controls: Vec<Element<'a, Message>> = vec![filter_bar(query), segmented(filter)];
  if write_enabled {
    controls.push(add_button());
  }

  let controls = Row::with_children(controls)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let filtering = !query.trim().is_empty() || filter != ContactFilter::All;
  let display = format!("{count}{suffix}");
  let meta = if filtering {
    t!("roster.contacts.matching", count => display).into_owned()
  } else {
    t!("roster.contacts.contacts", count => display).into_owned()
  };

  Column::with_children(vec![
    controls.into(),
    section_header(&t!("roster.contacts.address_book"), Some(&meta)),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .into()
}

fn filter_bar<'a>(query: &'a str) -> Element<'a, Message> {
  let placeholder = shared::static_text(t!("roster.contacts.filter_placeholder"));
  let mut field = TextInput::new(placeholder, query, Message::ContactsSearchChanged)
    .leading_icon(Icon::search())
    .font_size(typography::size::SM)
    .width(Length::Fill);

  if !query.is_empty() {
    field = field.trailing(clear_button());
  }

  container(field.render()).width(Length::Fill).into()
}

fn clear_button<'a>() -> Element<'a, Message> {
  button(
    container(Icon::close().size(13.0).color(color::text::secondary()).render())
      .width(Length::Fixed(22.0))
      .height(Length::Fixed(22.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(Message::ContactsSearchCleared)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then_some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      border: Border {
        color: iced::Color::TRANSPARENT,
        radius: 999.0.into(),
        width: 0.0,
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  })
  .into()
}

fn add_button<'a>() -> Element<'a, Message> {
  let label = Row::with_children(vec![
    Icon::plus().size(14.0).color(color::text::PRIMARY).render(),
    text(t!("roster.contacts.add_contact"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(
    container(label)
      .height(Length::Fill)
      .align_y(Vertical::Center)
      .padding(Padding {
        top: 0.0,
        right: spacing::SPACE_3_5,
        bottom: 0.0,
        left: spacing::SPACE_3_5,
      }),
  )
  .height(Length::Fixed(36.0))
  .on_press(Message::ContactAddOpened)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then_some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.08))),
      border: Border {
        color: if hover {
          color::accent::PLASMA
        } else {
          color::rule_strong()
        },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if hover {
        color::accent::PLASMA
      } else {
        color::text::PRIMARY
      },
      ..button::Style::default()
    }
  })
  .into()
}

pub(in crate::features::roster) fn body<'a>(
  contacts: &'a LoadState<ContactsPage>,
  sort: ContactSort,
  write_enabled: bool,
  viewport_height: f32,
  scroll_offset: f32,
  columns: ContactColumns,
) -> Element<'a, Message> {
  let page = match contacts {
    LoadState::Loaded(page) => page,
    LoadState::Loading => {
      return load_state_view(LoadStateView::Loading(shared::static_text(t!(
        "roster.contacts.loading"
      ))));
    }
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };

  let rows = page.rows();
  if rows.is_empty() {
    return card::panel(
      container(
        text(t!("roster.contacts.no_match"))
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          }),
      )
      .width(Length::Fill)
      .padding(spacing::SPACE_3_5),
      false,
    );
  }

  let labels: HashMap<i64, &str> = page
    .labels()
    .iter()
    .map(|label| (label.label_id(), label.label_name().as_str()))
    .collect();

  let config = VirtualListConfig::new(rows.len(), ESTIMATED_ROW_HEIGHT)
    .viewport_height(viewport_height)
    .scroll_offset(scroll_offset);
  let list = VirtualList::new(config, |index| {
    let row = &rows[index];
    contact_row(
      &row.contact,
      Some(&row.image),
      &labels,
      write_enabled,
      index == rows.len() - 1,
      columns,
    )
  })
  .view();
  let body = Column::with_children(vec![column_header(sort, write_enabled, columns), list]).width(Length::Fill);

  card::panel(body, false)
}

fn segmented<'a>(active: ContactFilter) -> Element<'a, Message> {
  let mut buttons: Vec<Element<'a, Message>> = Vec::with_capacity(ContactFilter::SEGMENTS.len());
  for (filter, label) in ContactFilter::SEGMENTS {
    let selected = filter == active;
    let label_color = if selected {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    };
    buttons.push(
      button(
        text(t!(label))
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(move |_| text::Style {
            color: Some(label_color),
          }),
      )
      .padding(Padding {
        top: spacing::UNIT + 1.0,
        right: spacing::SPACE_3,
        bottom: spacing::UNIT + 1.0,
        left: spacing::SPACE_3,
      })
      .on_press(Message::ContactFilterChanged(filter))
      .style(move |_, status| segment_button_style(selected, status))
      .into(),
    );
  }

  container(Row::with_children(buttons).spacing(2.0))
    .padding(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.08),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn col_label<'a>(label: &str, right: bool) -> Element<'a, Message> {
  let cell = eyebrow_text(label, Some(color::text::tertiary())).width(Length::Fill);

  container(cell)
    .width(Length::Fill)
    .align_x(if right { Horizontal::Right } else { Horizontal::Left })
    .into()
}

fn sortable_label<'a>(label: &str, right: bool, column: SortColumn, sort: ContactSort) -> Element<'a, Message> {
  let active = sort.column == column;
  let label_color = if active {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };

  let mut children: Vec<Element<'a, Message>> = vec![eyebrow_text(label, Some(label_color)).into()];
  if let Some(direction) = sort.caret(column) {
    let chevron = match direction {
      SortDirection::Ascending => Icon::chevron_up(),
      SortDirection::Descending => Icon::chevron_down(),
    };
    children.push(chevron.size(typography::size::XS).color(color::accent::PLASMA).render());
  }

  let inner = container(
    Row::with_children(children)
      .spacing(spacing::UNIT)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .align_x(if right { Horizontal::Right } else { Horizontal::Left });

  button(inner)
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::ContactSortChanged(sort.toggled(column)))
    .style(|_, _| button::Style {
      background: None,
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into()
}

fn column_header<'a>(sort: ContactSort, write_enabled: bool, columns: ContactColumns) -> Element<'a, Message> {
  let mut children = vec![
    sortable_label(&t!("roster.contacts.column_entity"), false, SortColumn::Entity, sort),
    cell(
      sortable_label(&t!("roster.contacts.column_type"), false, SortColumn::Type, sort),
      TYPE_WIDTH,
    ),
    cell(
      sortable_label(&t!("roster.contacts.column_standing"), true, SortColumn::Standing, sort),
      STANDING_WIDTH,
    ),
  ];
  if columns.labels {
    children.push(col_label(&t!("roster.contacts.column_note"), false));
  }
  if columns.watchlist {
    children.push(cell(
      col_label(&t!("roster.contacts.column_watchlist"), true),
      WATCHLIST_WIDTH,
    ));
  }
  if write_enabled {
    children.push(cell(col_label(&t!("roster.contacts.column_edit"), true), ACTIONS_WIDTH));
  }

  let row = Row::with_children(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(row_padding())
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: 1.0,
        radius: Radius {
          top_left: radius::CONTROL,
          top_right: radius::CONTROL,
          bottom_right: 0.0,
          bottom_left: 0.0,
        },
      },
      ..container::Style::default()
    })
    .into()
}

fn contact_row<'a>(
  contact: &'a CharacterContact,
  image: Option<&ImageState>,
  labels: &HashMap<i64, &'a str>,
  write_enabled: bool,
  last: bool,
  columns: ContactColumns,
) -> Element<'a, Message> {
  let standing = contact.standing();
  let standing_color = shared::standing_color(standing);

  let portrait = avatar(
    contact.contact_id(),
    contact.contact_name(),
    Length::Fixed(AVATAR_SIZE),
    AVATAR_SIZE,
    image.and_then(ImageState::path),
  );

  let name = text(contact.contact_name().clone())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let entity = Row::with_children(vec![portrait, name.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let kind = text(contact.contact_type().to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let standing_text = text(format!("{}{:.1}", if standing >= 0.0 { "+" } else { "" }, standing))
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(standing_color),
    });

  let note = text(label_note(contact, labels))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .width(Length::Fill);

  let watch: Element<'a, Message> = if contact.is_watched() {
    pill(&t!("roster.contacts.watch_pill"))
  } else {
    text("").width(Length::Fill).into()
  };

  let mut children = vec![
    entity.into(),
    cell(kind.into(), TYPE_WIDTH),
    cell(right_align(standing_text.into()), STANDING_WIDTH),
  ];
  if columns.labels {
    children.push(note.into());
  }
  if columns.watchlist {
    children.push(cell(right_align(watch), WATCHLIST_WIDTH));
  }
  if write_enabled {
    children.push(cell(row_actions(contact), ACTIONS_WIDTH));
  }

  let row = Row::with_children(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };
  container(row)
    .width(Length::Fill)
    .padding(row_padding())
    .style(move |_| shared::row_rule_style(border_bottom))
    .into()
}

fn row_actions(contact: &CharacterContact) -> Element<'_, Message> {
  let edit = action_button(
    Icon::pencil(),
    false,
    Message::ContactEditOpened(Box::new(contact.clone())),
  );
  let delete = action_button(
    Icon::trash(),
    true,
    Message::ContactDeleteRequested(Box::new(contact.clone())),
  );

  Row::with_children(vec![edit, delete])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center)
    .into()
}

fn action_button<'a>(icon: Icon, danger: bool, message: Message) -> Element<'a, Message> {
  let tint = if danger {
    color::status::DANGER
  } else {
    color::text::secondary()
  };

  button(
    container(icon.size(14.0).color(tint).render())
      .width(Length::Fixed(ACTION_SIZE))
      .height(Length::Fixed(ACTION_SIZE))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(message)
  .style(move |_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let (border, background) = if hover && danger {
      (
        color::with_alpha(color::status::DANGER, 0.4),
        color::with_alpha(color::status::DANGER, 0.12),
      )
    } else if hover {
      (color::rule_strong(), color::with_alpha(color::text::PRIMARY, 0.06))
    } else {
      (iced::Color::TRANSPARENT, iced::Color::TRANSPARENT)
    };
    button::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  })
  .into()
}

fn label_note(contact: &CharacterContact, labels: &HashMap<i64, &str>) -> String {
  let ids: Vec<i64> = serde_json::from_str(contact.label_ids()).unwrap_or_default();
  ids
    .into_iter()
    .filter_map(|id| labels.get(&id).copied())
    .collect::<Vec<_>>()
    .join(", ")
}

fn pill<'a>(label: &str) -> Element<'a, Message> {
  container(
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 2.0,
    right: spacing::SPACE_2 - 1.0,
    bottom: 2.0,
    left: spacing::SPACE_2 - 1.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.10))),
    border: Border {
      color: color::accent::PLASMA,
      width: 1.0,
      radius: 999.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn cell(child: Element<'_, Message>, width: f32) -> Element<'_, Message> {
  container(child).width(Length::Fixed(width)).into()
}

fn right_align(child: Element<'_, Message>) -> Element<'_, Message> {
  container(child)
    .width(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .into()
}

fn row_padding() -> Padding {
  Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::model::CharacterContactLabel;

  fn contact(id: i64, kind: &str, standing: f64, watched: bool, label_ids: &str, name: &str) -> CharacterContact {
    CharacterContact {
      character_id: 42,
      contact_id: id,
      contact_name: name.to_owned(),
      contact_type: kind.to_owned(),
      is_blocked: false,
      is_watched: watched,
      label_ids: label_ids.to_owned(),
      standing,
    }
  }

  fn label(id: i64, name: &str) -> CharacterContactLabel {
    CharacterContactLabel {
      character_id: 42,
      label_id: id,
      label_name: name.to_owned(),
    }
  }

  fn contact_row(id: i64, kind: &str, standing: f64, watched: bool, label_ids: &str, name: &str) -> ContactRow {
    ContactRow {
      contact: contact(id, kind, standing, watched, label_ids, name),
      image: ImageState::Stale {
        id,
        kind: crate::store::images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn loaded() -> ContactsPage {
    ContactsPage::for_test(
      vec![
        contact_row(100, "character", 8.5, true, "[1,2]", "Wingmate"),
        contact_row(200, "corporation", -5.0, false, "[]", "Hostile Corp"),
        contact_row(300, "alliance", 0.0, false, "[99]", "Neutral Alliance"),
      ],
      vec![label(1, "Fleet"), label(2, "Trusted")],
      false,
    )
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_an_empty_page_as_a_no_match_panel() {
      let state = LoadState::Loaded(ContactsPage::for_test(Vec::new(), Vec::new(), false));

      let _el: Element<'_, Message> = body(
        &state,
        ContactSort::default(),
        false,
        600.0,
        0.0,
        ContactColumns::default(),
      );
    }

    #[test]
    fn it_renders_each_sort_column_and_direction() {
      let state = LoadState::Loaded(loaded());

      for column in [SortColumn::Entity, SortColumn::Type, SortColumn::Standing] {
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
          let _el: Element<'_, Message> = body(
            &state,
            ContactSort {
              column,
              direction,
            },
            true,
            600.0,
            0.0,
            ContactColumns::default(),
          );
        }
      }
    }

    #[test]
    fn it_renders_loading_and_error_states() {
      let loading: LoadState<ContactsPage> = LoadState::Loading;
      let error: LoadState<ContactsPage> = LoadState::Error("boom".to_owned());

      let _loading: Element<'_, Message> = body(
        &loading,
        ContactSort::default(),
        false,
        600.0,
        0.0,
        ContactColumns::default(),
      );
      let _error: Element<'_, Message> = body(
        &error,
        ContactSort::default(),
        false,
        600.0,
        0.0,
        ContactColumns::default(),
      );
    }
  }

  mod contact_row {
    use super::*;

    #[test]
    fn it_renders_a_watched_contact_with_labels() {
      let c = contact(100, "character", 8.5, true, "[1,2]", "Wingmate");
      let image = ImageState::Stale {
        id: 100,
        kind: crate::store::images::ImageKind::CharacterPortrait,
      };
      let mut labels = HashMap::new();
      labels.insert(1, "Fleet");
      labels.insert(2, "Trusted");

      let _el: Element<'_, Message> =
        super::super::contact_row(&c, Some(&image), &labels, false, false, ContactColumns::default());
    }

    #[test]
    fn it_renders_an_unwatched_negative_standing_last_row() {
      let c = contact(200, "corporation", -5.0, false, "[]", "Hostile Corp");
      let labels: HashMap<i64, &str> = HashMap::new();

      let _el: Element<'_, Message> =
        super::super::contact_row(&c, None, &labels, false, true, ContactColumns::default());
    }

    #[test]
    fn it_renders_the_edit_and_delete_actions_when_writes_are_enabled() {
      let c = contact(100, "character", 8.5, true, "[1,2]", "Wingmate");
      let labels: HashMap<i64, &str> = HashMap::new();

      let _el: Element<'_, Message> =
        super::super::contact_row(&c, None, &labels, true, false, ContactColumns::default());
    }

    #[test]
    fn it_renders_without_the_labels_and_watchlist_columns() {
      let c = contact(100, "character", 8.5, true, "[1,2]", "Wingmate");
      let labels: HashMap<i64, &str> = HashMap::new();

      let _el: Element<'_, Message> =
        super::super::contact_row(&c, None, &labels, true, false, ContactColumns::standings_only());
    }
  }

  mod contact_columns {
    use super::*;

    #[test]
    fn it_defaults_to_every_column_and_standings_only_hides_the_extras() {
      assert!(ContactColumns::default().labels);
      assert!(ContactColumns::default().watchlist);

      assert!(!ContactColumns::standings_only().labels);
      assert!(!ContactColumns::standings_only().watchlist);
    }
  }

  mod filter {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_facet_to_its_contact_type() {
      assert_eq!(ContactFilter::All.contact_type(), None);
      assert_eq!(ContactFilter::Character.contact_type(), Some("character"));
      assert_eq!(ContactFilter::Corp.contact_type(), Some("corporation"));
      assert_eq!(ContactFilter::Alliance.contact_type(), Some("alliance"));
    }
  }

  mod header {
    use super::*;

    #[test]
    fn it_keeps_the_filter_segments_in_render_order() {
      assert_eq!(
        ContactFilter::SEGMENTS,
        [
          (ContactFilter::All, "roster.contacts.filter_all"),
          (ContactFilter::Character, "roster.contacts.filter_characters"),
          (ContactFilter::Corp, "roster.contacts.filter_corps"),
          (ContactFilter::Alliance, "roster.contacts.filter_alliances"),
        ]
      );
    }

    #[test]
    fn it_renders_a_zero_count_in_the_loading_state() {
      let loading: LoadState<ContactsPage> = LoadState::Loading;

      let _el: Element<'_, Message> = super::super::header(&loading, ContactFilter::All, "", false);
    }

    #[test]
    fn it_renders_each_filter() {
      let state = LoadState::Loaded(loaded());

      for filter in [
        ContactFilter::All,
        ContactFilter::Character,
        ContactFilter::Corp,
        ContactFilter::Alliance,
      ] {
        let _el: Element<'_, Message> = super::super::header(&state, filter, "", false);
      }
    }

    #[test]
    fn it_renders_the_add_button_when_writes_are_enabled() {
      let state = LoadState::Loaded(loaded());

      let _el: Element<'_, Message> = super::super::header(&state, ContactFilter::All, "", true);
    }

    #[test]
    fn it_renders_the_filter_bar_with_a_clear_button_for_an_active_query() {
      let state = LoadState::Loaded(loaded());

      let _el: Element<'_, Message> = super::super::header(&state, ContactFilter::All, "wing", false);
    }

    #[test]
    fn it_renders_the_more_pages_count_suffix() {
      let page = ContactsPage::for_test(
        vec![contact_row(100, "character", 1.0, false, "[]", "Pilot")],
        Vec::new(),
        true,
      );
      let state = LoadState::Loaded(page);

      let _el: Element<'_, Message> = super::super::header(&state, ContactFilter::All, "", false);
    }
  }

  mod label_note {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_empty_for_no_labels_or_unknown_ids() {
      let labels: HashMap<i64, &str> = HashMap::from([(1, "Fleet")]);

      assert_eq!(label_note(&contact(1, "character", 0.0, false, "[]", "A"), &labels), "");
      assert_eq!(
        label_note(&contact(2, "character", 0.0, false, "[99]", "B"), &labels),
        ""
      );
      assert_eq!(
        label_note(&contact(3, "character", 0.0, false, "not-json", "C"), &labels),
        ""
      );
    }

    #[test]
    fn it_joins_resolved_label_names() {
      let labels: HashMap<i64, &str> = HashMap::from([(1, "Fleet"), (2, "Trusted")]);
      let c = contact(100, "character", 0.0, false, "[1,2]", "Wingmate");

      assert_eq!(label_note(&c, &labels), "Fleet, Trusted");
    }
  }

  mod sort {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_to_strongest_standing_first() {
      assert_eq!(ContactSort::default().column, SortColumn::Standing);
      assert_eq!(ContactSort::default().direction, SortDirection::Descending);
    }

    #[test]
    fn it_flips_direction_when_re_clicking_the_active_column() {
      let sort = ContactSort {
        column: SortColumn::Entity,
        direction: SortDirection::Ascending,
      };

      let flipped = sort.toggled(SortColumn::Entity);
      assert_eq!(flipped.direction, SortDirection::Descending);

      let flipped_again = flipped.toggled(SortColumn::Entity);
      assert_eq!(flipped_again.direction, SortDirection::Ascending);
    }

    #[test]
    fn it_marks_only_the_active_column_with_a_caret() {
      let sort = ContactSort {
        column: SortColumn::Standing,
        direction: SortDirection::Descending,
      };

      assert_eq!(sort.caret(SortColumn::Standing), Some(SortDirection::Descending));
      assert!(sort.caret(SortColumn::Entity).is_none());
      assert!(sort.caret(SortColumn::Type).is_none());
    }

    #[test]
    fn it_starts_a_fresh_column_on_its_natural_direction() {
      let sort = ContactSort::default().toggled(SortColumn::Entity);
      assert_eq!(sort.column, SortColumn::Entity);
      assert_eq!(sort.direction, SortDirection::Ascending);

      let sort = ContactSort::default().toggled(SortColumn::Type);
      assert_eq!(sort.direction, SortDirection::Ascending);
    }
  }

  mod sortable_label {
    use super::*;

    #[test]
    fn it_renders_active_and_inactive_columns_on_both_sides() {
      let sort = ContactSort::default();

      let _active: Element<'_, Message> = super::super::sortable_label("Standing", true, SortColumn::Standing, sort);
      let _inactive: Element<'_, Message> = super::super::sortable_label("Entity", false, SortColumn::Entity, sort);
    }
  }
}
