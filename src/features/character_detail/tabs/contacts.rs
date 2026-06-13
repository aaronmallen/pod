use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, button, container, text},
};

use super::{
  super::{LoadState, Message},
  shared,
};
use crate::{
  store::model::{CharacterContact, character_contacts_view::CharacterContacts},
  ui::{
    components::{
      card,
      empty_state::{LoadStateView, load_state_view},
      eyebrow::eyebrow_text,
      section_header::section_header,
      segmented::segment_button_style,
    },
    style::{color, radius, spacing, typography},
  },
};

const STANDING_WIDTH: f32 = 70.0;
const TYPE_WIDTH: f32 = 90.0;
const WATCHLIST_WIDTH: f32 = 80.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ContactFilter {
  Alliance,
  #[default]
  All,
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

  fn contact_type(self) -> Option<&'static str> {
    match self {
      ContactFilter::All => None,
      ContactFilter::Character => Some("character"),
      ContactFilter::Corp => Some("corporation"),
      ContactFilter::Alliance => Some("alliance"),
    }
  }

  fn matches(self, contact: &CharacterContact) -> bool {
    match self.contact_type() {
      None => true,
      Some(kind) => contact.contact_type().eq_ignore_ascii_case(kind),
    }
  }
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

  fn caret(self, column: SortColumn) -> Option<&'static str> {
    (self.column == column).then_some(match self.direction {
      SortDirection::Ascending => "\u{25b2}",
      SortDirection::Descending => "\u{25bc}",
    })
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

fn sort_contacts(rows: &mut [&CharacterContact], sort: ContactSort) {
  rows.sort_by(|a, b| {
    let ordering = match sort.column {
      SortColumn::Entity => a.contact_name().cmp(b.contact_name()),
      SortColumn::Standing => a.standing().total_cmp(&b.standing()),
      SortColumn::Type => a.contact_type().cmp(b.contact_type()),
    };
    let ordering = match sort.direction {
      SortDirection::Ascending => ordering,
      SortDirection::Descending => ordering.reverse(),
    };
    ordering.then_with(|| a.contact_name().cmp(b.contact_name()))
  });
}

pub(in crate::features::character_detail) fn body(
  contacts: &LoadState<CharacterContacts>,
  filter: ContactFilter,
  sort: ContactSort,
  visible: usize,
) -> Element<'_, Message> {
  let loaded = match contacts {
    LoadState::Loaded(loaded) => loaded,
    LoadState::Loading => return load_state_view(LoadStateView::Loading("Loading contacts\u{2026}")),
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };

  let labels: HashMap<i64, &str> = loaded
    .labels
    .iter()
    .map(|label| (label.label_id(), label.label_name().as_str()))
    .collect();

  let mut rows: Vec<&CharacterContact> = loaded
    .contacts
    .iter()
    .filter(|contact| filter.matches(contact))
    .collect();
  sort_contacts(&mut rows, sort);

  let header = Row::with_children(vec![
    section_header("Address book", Some(&format!("{} contacts", rows.len()))),
    segmented(filter),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let table = if rows.is_empty() {
    card::panel(
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
    )
  } else {
    let shown = visible.min(rows.len());
    let mut card_rows: Vec<Element<'_, Message>> = vec![column_header(sort)];
    for (index, contact) in rows.iter().take(shown).enumerate() {
      card_rows.push(contact_row(contact, &labels, index == shown - 1));
    }
    card::panel(Column::with_children(card_rows).width(Length::Fill), false)
  };

  Column::with_children(vec![header.into(), table])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
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
  if let Some(caret) = sort.caret(column) {
    children.push(
      text(caret)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::accent::PLASMA),
        })
        .into(),
    );
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
  let row = Row::with_children(vec![
    sortable_label("Entity", false, SortColumn::Entity, sort),
    cell(sortable_label("Type", false, SortColumn::Type, sort), TYPE_WIDTH),
    cell(
      sortable_label("Standing", true, SortColumn::Standing, sort),
      STANDING_WIDTH,
    ),
    col_label("Note", false),
    cell(col_label("Watchlist", true), WATCHLIST_WIDTH),
  ])
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

fn contact_row<'a>(contact: &'a CharacterContact, labels: &HashMap<i64, &'a str>, last: bool) -> Element<'a, Message> {
  let standing = contact.standing();
  let standing_color = shared::standing_color(standing);

  let entity = text(contact.contact_name().clone())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    })
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

  let row = Row::with_children(vec![
    entity.into(),
    cell(kind.into(), TYPE_WIDTH),
    cell(right_align(standing_text.into()), STANDING_WIDTH),
    note.into(),
    cell(right_align(watch), WATCHLIST_WIDTH),
  ])
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

  fn loaded() -> CharacterContacts {
    CharacterContacts {
      contacts: vec![
        contact(100, "character", 8.5, true, "[1,2]", "Wingmate"),
        contact(200, "corporation", -5.0, false, "[]", "Hostile Corp"),
        contact(300, "alliance", 0.0, false, "[99]", "Neutral Alliance"),
      ],
      labels: vec![label(1, "Fleet"), label(2, "Trusted")],
    }
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_each_filter() {
      let state = LoadState::Loaded(loaded());

      for filter in [
        ContactFilter::All,
        ContactFilter::Character,
        ContactFilter::Corp,
        ContactFilter::Alliance,
      ] {
        let _el: Element<'_, Message> = body(&state, filter, ContactSort::default(), usize::MAX);
      }
    }

    #[test]
    fn it_renders_each_sort_column_and_direction() {
      let state = LoadState::Loaded(loaded());

      for column in [SortColumn::Entity, SortColumn::Type, SortColumn::Standing] {
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
          let _el: Element<'_, Message> = body(
            &state,
            ContactFilter::All,
            ContactSort {
              column,
              direction,
            },
            usize::MAX,
          );
        }
      }
    }

    #[test]
    fn it_renders_loading_and_error_states() {
      let loading: LoadState<CharacterContacts> = LoadState::Loading;
      let error: LoadState<CharacterContacts> = LoadState::Error("boom".to_owned());

      let _loading: Element<'_, Message> = body(&loading, ContactFilter::All, ContactSort::default(), usize::MAX);
      let _error: Element<'_, Message> = body(&error, ContactFilter::All, ContactSort::default(), usize::MAX);
    }
  }

  mod filter {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_passes_everything_for_all() {
      let contacts = loaded().contacts;

      let matched = contacts.iter().filter(|c| ContactFilter::All.matches(c)).count();
      assert_eq!(matched, 3);
    }

    #[test]
    fn it_filters_by_contact_type() {
      let contacts = loaded().contacts;

      let chars = contacts.iter().filter(|c| ContactFilter::Character.matches(c)).count();
      let corps = contacts.iter().filter(|c| ContactFilter::Corp.matches(c)).count();
      let alliances = contacts.iter().filter(|c| ContactFilter::Alliance.matches(c)).count();

      assert_eq!(chars, 1);
      assert_eq!(corps, 1);
      assert_eq!(alliances, 1);
    }
  }

  mod sort {
    use pretty_assertions::assert_eq;

    use super::*;

    fn names(rows: &[&CharacterContact]) -> Vec<String> {
      rows.iter().map(|c| c.contact_name().clone()).collect()
    }

    #[test]
    fn it_defaults_to_strongest_standing_first() {
      assert_eq!(ContactSort::default().column, SortColumn::Standing);
      assert_eq!(ContactSort::default().direction, SortDirection::Descending);

      let contacts = loaded().contacts;
      let mut rows: Vec<&CharacterContact> = contacts.iter().collect();
      sort_contacts(&mut rows, ContactSort::default());

      assert_eq!(names(&rows), vec!["Wingmate", "Neutral Alliance", "Hostile Corp"]);
    }

    #[test]
    fn it_sorts_by_entity_name_ascending_and_descending() {
      let contacts = loaded().contacts;

      let mut asc: Vec<&CharacterContact> = contacts.iter().collect();
      sort_contacts(
        &mut asc,
        ContactSort {
          column: SortColumn::Entity,
          direction: SortDirection::Ascending,
        },
      );
      assert_eq!(names(&asc), vec!["Hostile Corp", "Neutral Alliance", "Wingmate"]);

      let mut desc: Vec<&CharacterContact> = contacts.iter().collect();
      sort_contacts(
        &mut desc,
        ContactSort {
          column: SortColumn::Entity,
          direction: SortDirection::Descending,
        },
      );
      assert_eq!(names(&desc), vec!["Wingmate", "Neutral Alliance", "Hostile Corp"]);
    }

    #[test]
    fn it_sorts_by_contact_type_ascending() {
      let contacts = loaded().contacts;
      let mut rows: Vec<&CharacterContact> = contacts.iter().collect();
      sort_contacts(
        &mut rows,
        ContactSort {
          column: SortColumn::Type,
          direction: SortDirection::Ascending,
        },
      );

      let kinds: Vec<&str> = rows.iter().map(|c| c.contact_type().as_str()).collect();
      assert_eq!(kinds, vec!["alliance", "character", "corporation"]);
    }

    #[test]
    fn it_starts_a_fresh_column_on_its_natural_direction() {
      let sort = ContactSort::default().toggled(SortColumn::Entity);
      assert_eq!(sort.column, SortColumn::Entity);
      assert_eq!(sort.direction, SortDirection::Ascending);

      let sort = ContactSort::default().toggled(SortColumn::Type);
      assert_eq!(sort.direction, SortDirection::Ascending);
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

      assert_eq!(sort.caret(SortColumn::Standing), Some("\u{25bc}"));
      assert!(sort.caret(SortColumn::Entity).is_none());
      assert!(sort.caret(SortColumn::Type).is_none());
    }
  }

  mod label_note {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_joins_resolved_label_names() {
      let labels: HashMap<i64, &str> = HashMap::from([(1, "Fleet"), (2, "Trusted")]);
      let c = contact(100, "character", 0.0, false, "[1,2]", "Wingmate");

      assert_eq!(label_note(&c, &labels), "Fleet, Trusted");
    }

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
  }
}
