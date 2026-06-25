use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, button, container, text},
};

use super::shared;
use crate::{
  features::corporation_detail::{ContactsPage, LoadState, Message},
  store::{images::ImageState, model::CorporationContact},
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

const AVATAR_SIZE: f32 = 30.0;
const STANDING_WIDTH: f32 = 70.0;
const TYPE_WIDTH: f32 = 90.0;
const WATCHLIST_WIDTH: f32 = 80.0;

/// Nominal height of one contact row, in pixels. Rows are single-line, so this only feeds the [`VirtualList`]
/// offset math; the overscan margin absorbs any minor variance.
const ESTIMATED_ROW_HEIGHT: f32 = 46.0;

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
    (ContactFilter::All, "All"),
    (ContactFilter::Character, "Characters"),
    (ContactFilter::Corp, "Corps"),
    (ContactFilter::Alliance, "Alliances"),
  ];

  /// The `corporation_contacts.contact_type` value this facet filters to, or `None` for the All facet. The feature
  /// pushes this into the paginated SQL query rather than filtering an in-memory set.
  pub(in crate::features::corporation_detail) fn contact_type(self) -> Option<&'static str> {
    match self {
      ContactFilter::All => None,
      ContactFilter::Character => Some("character"),
      ContactFilter::Corp => Some("corporation"),
      ContactFilter::Alliance => Some("alliance"),
    }
  }
}

/// A render-ready contact row: the raw contact joined to its resolved avatar so the windowed body can build a
/// row without holding the whole address book (and its image map) in memory.
#[derive(Clone, Debug)]
pub struct ContactRow {
  pub contact: CorporationContact,
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

/// The non-scrolling header for the Contacts tab: the address-book title (with a loaded-so-far count) and the
/// entity-type facet. Hoisted above the windowed list so it stays put while the list scrolls.
pub(in crate::features::corporation_detail) fn header<'a>(
  contacts: &LoadState<ContactsPage>,
  filter: ContactFilter,
  query: &'a str,
) -> Element<'a, Message> {
  let (count, suffix) = match contacts {
    LoadState::Loaded(page) => (page.rows().len(), if page.has_more() { "+" } else { "" }),
    _ => (0, ""),
  };

  let controls = Row::with_children(vec![filter_bar(query), segmented(filter)])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let filtering = !query.trim().is_empty() || filter != ContactFilter::All;
  let meta = if filtering {
    format!("{count}{suffix} matching")
  } else {
    format!("{count}{suffix} contacts")
  };

  Column::with_children(vec![controls.into(), section_header("Address book", Some(&meta))])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

/// The contacts name-search field: a bordered input with a leading search glyph and a clear button when a query is
/// present, sat to the left of the type facet. The query is pushed into the paginated SQL load rather than filtering
/// the loaded rows in memory.
fn filter_bar<'a>(query: &'a str) -> Element<'a, Message> {
  let mut field = TextInput::new("Filter by name\u{2026}", query, Message::ContactsSearchChanged)
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

/// The windowed body for the Contacts tab: the column header plus the keyset page of rows, windowed so only the
/// viewport's rows (plus overscan) are materialized regardless of how many pages have loaded. The caller wraps this
/// in the tab's scrollable inside `responsive`, supplying the real `viewport_height`.
pub(in crate::features::corporation_detail) fn body<'a>(
  contacts: &'a LoadState<ContactsPage>,
  sort: ContactSort,
  viewport_height: f32,
  scroll_offset: f32,
) -> Element<'a, Message> {
  let page = match contacts {
    LoadState::Loaded(page) => page,
    LoadState::Loading => {
      return load_state_view(LoadStateView::Loading("Loading contacts\u{2026}"));
    }
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };

  let rows = page.rows();
  if rows.is_empty() {
    return card::panel(
      container(
        text("No contacts match this filter")
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

  // Window the rows so a multi-thousand-contact address book renders the same handful of widgets. The column
  // header rides inside the windowed column so it scrolls with the rows under the hoisted address-book header.
  let config = VirtualListConfig::new(rows.len(), ESTIMATED_ROW_HEIGHT)
    .viewport_height(viewport_height)
    .scroll_offset(scroll_offset);
  let list = VirtualList::new(config, |index| {
    let row = &rows[index];
    contact_row(&row.contact, Some(&row.image), &labels, index == rows.len() - 1)
  })
  .view();
  let body = Column::with_children(vec![column_header(sort), list]).width(Length::Fill);

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
        text(label)
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

fn column_header<'a>(sort: ContactSort) -> Element<'a, Message> {
  let children = vec![
    sortable_label("Entity", false, SortColumn::Entity, sort),
    cell(sortable_label("Type", false, SortColumn::Type, sort), TYPE_WIDTH),
    cell(
      sortable_label("Standing", true, SortColumn::Standing, sort),
      STANDING_WIDTH,
    ),
    col_label("Note", false),
    cell(col_label("Watchlist", true), WATCHLIST_WIDTH),
  ];

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
  contact: &'a CorporationContact,
  image: Option<&ImageState>,
  labels: &HashMap<i64, &'a str>,
  last: bool,
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
    pill("watch")
  } else {
    text("").width(Length::Fill).into()
  };

  let children = vec![
    entity.into(),
    cell(kind.into(), TYPE_WIDTH),
    cell(right_align(standing_text.into()), STANDING_WIDTH),
    note.into(),
    cell(right_align(watch), WATCHLIST_WIDTH),
  ];

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

fn label_note(contact: &CorporationContact, labels: &HashMap<i64, &str>) -> String {
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
  use crate::store::model::CorporationContactLabel;

  fn contact(id: i64, kind: &str, standing: f64, watched: bool, label_ids: &str, name: &str) -> CorporationContact {
    CorporationContact {
      contact_id: id,
      contact_name: name.to_owned(),
      contact_type: kind.to_owned(),
      corporation_id: 42,
      is_blocked: false,
      is_watched: watched,
      label_ids: label_ids.to_owned(),
      standing,
    }
  }

  fn label(id: i64, name: &str) -> CorporationContactLabel {
    CorporationContactLabel {
      corporation_id: 42,
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

      let _el: Element<'_, Message> = body(&state, ContactSort::default(), 600.0, 0.0);
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
            600.0,
            0.0,
          );
        }
      }
    }

    #[test]
    fn it_renders_loading_and_error_states() {
      let loading: LoadState<ContactsPage> = LoadState::Loading;
      let error: LoadState<ContactsPage> = LoadState::Error("boom".to_owned());

      let _loading: Element<'_, Message> = body(&loading, ContactSort::default(), 600.0, 0.0);
      let _error: Element<'_, Message> = body(&error, ContactSort::default(), 600.0, 0.0);
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

      let _el: Element<'_, Message> = super::super::contact_row(&c, Some(&image), &labels, false);
    }

    #[test]
    fn it_renders_an_unwatched_negative_standing_last_row() {
      let c = contact(200, "corporation", -5.0, false, "[]", "Hostile Corp");
      let labels: HashMap<i64, &str> = HashMap::new();

      let _el: Element<'_, Message> = super::super::contact_row(&c, None, &labels, true);
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
          (ContactFilter::All, "All"),
          (ContactFilter::Character, "Characters"),
          (ContactFilter::Corp, "Corps"),
          (ContactFilter::Alliance, "Alliances"),
        ]
      );
    }

    #[test]
    fn it_renders_a_zero_count_in_the_loading_state() {
      let loading: LoadState<ContactsPage> = LoadState::Loading;

      let _el: Element<'_, Message> = super::super::header(&loading, ContactFilter::All, "");
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
        let _el: Element<'_, Message> = super::super::header(&state, filter, "");
      }
    }

    #[test]
    fn it_renders_the_filter_bar_with_a_clear_button_for_an_active_query() {
      let state = LoadState::Loaded(loaded());

      let _el: Element<'_, Message> = super::super::header(&state, ContactFilter::All, "wing");
    }

    #[test]
    fn it_renders_the_more_pages_count_suffix() {
      let page = ContactsPage::for_test(
        vec![contact_row(100, "character", 1.0, false, "[]", "Pilot")],
        Vec::new(),
        true,
      );
      let state = LoadState::Loaded(page);

      let _el: Element<'_, Message> = super::super::header(&state, ContactFilter::All, "");
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
