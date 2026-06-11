use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, image, text},
};

use super::{HEADER_SIDE_PADDING, Message, RosterPilot, fmt_isk};
use crate::{
  clients::eve_image::Size,
  store::{
    images::{self, IconResolution},
    model::asset_query::InventoryRow,
  },
  ui::{
    components::{
      card::card_padded, empty_state::empty_state as shared_empty_state, eyebrow::eyebrow, icon_tile::icon_tile,
    },
    style::{color, radius, spacing, typography},
  },
};

const ICON_SIZE: Size = Size::S64;
const TOP_ITEM_ICON: f32 = 24.0;
const TOP_ITEM_COUNT: usize = 10;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValueSummary {
  pub(super) by_category: Vec<CategoryValue>,
  pub(super) by_location: Vec<LocationValue>,
  pub(super) matrix_locations: Vec<MatrixLocation>,
  pub(super) matrix_rows: Vec<MatrixRow>,
  pub(super) top_items: Vec<TopItem>,
  pub(super) total_value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CategoryValue {
  pub label: String,
  pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct LocationValue {
  pub label: Option<String>,
  pub location_id: i64,
  pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MatrixLocation {
  pub label: Option<String>,
  pub location_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MatrixRow {
  pub cells: HashMap<i64, f64>,
  pub owner_label: String,
  pub total: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TopItem {
  pub group_name: String,
  pub quantity: i64,
  pub type_id: i64,
  pub type_name: String,
  pub value: f64,
}

pub(super) fn summarize(rows: &[InventoryRow], roster: &[RosterPilot]) -> ValueSummary {
  let total_value: f64 = rows.iter().map(|r| r.value).sum();

  ValueSummary {
    by_category: by_category(rows),
    by_location: by_location(rows),
    matrix_locations: matrix_locations(rows),
    matrix_rows: matrix_rows(rows, roster),
    top_items: top_items(rows),
    total_value,
  }
}

fn by_category(rows: &[InventoryRow]) -> Vec<CategoryValue> {
  let mut totals: HashMap<String, f64> = HashMap::new();
  for row in rows {
    *totals.entry(row.category.clone()).or_default() += row.value;
  }
  let mut out: Vec<CategoryValue> = totals
    .into_iter()
    .map(|(key, value)| CategoryValue {
      label: category_label(&key),
      value,
    })
    .collect();
  out.sort_by(|a, b| b.value.total_cmp(&a.value));
  out
}

fn by_location(rows: &[InventoryRow]) -> Vec<LocationValue> {
  let mut totals: HashMap<i64, f64> = HashMap::new();
  let mut labels: HashMap<i64, String> = HashMap::new();
  for row in rows {
    *totals.entry(row.location_id).or_default() += row.value;
    if let Some(label) = &row.location_label {
      labels.entry(row.location_id).or_insert_with(|| label.clone());
    }
  }
  let mut out: Vec<LocationValue> = totals
    .into_iter()
    .map(|(location_id, value)| LocationValue {
      label: labels.get(&location_id).cloned(),
      location_id,
      value,
    })
    .collect();
  out.sort_by(|a, b| b.value.total_cmp(&a.value));
  out
}

fn matrix_locations(rows: &[InventoryRow]) -> Vec<MatrixLocation> {
  by_location(rows)
    .into_iter()
    .map(|l| MatrixLocation {
      label: l.label,
      location_id: l.location_id,
    })
    .collect()
}

fn matrix_rows(rows: &[InventoryRow], roster: &[RosterPilot]) -> Vec<MatrixRow> {
  let mut owners: HashMap<i64, HashMap<i64, f64>> = HashMap::new();
  for row in rows {
    *owners
      .entry(row.owner_id)
      .or_default()
      .entry(row.location_id)
      .or_default() += row.value;
  }

  let mut out: Vec<MatrixRow> = owners
    .into_iter()
    .map(|(owner_id, cells)| {
      let total = cells.values().sum();
      MatrixRow {
        cells,
        owner_label: owner_label(owner_id, roster),
        total,
      }
    })
    .collect();
  out.sort_by(|a, b| b.total.total_cmp(&a.total));
  out
}

fn top_items(rows: &[InventoryRow]) -> Vec<TopItem> {
  let mut sorted: Vec<&InventoryRow> = rows.iter().collect();
  sorted.sort_by(|a, b| b.value.total_cmp(&a.value));
  sorted
    .into_iter()
    .take(TOP_ITEM_COUNT)
    .map(|row| TopItem {
      group_name: row.group_name.clone(),
      quantity: row.quantity,
      type_id: row.type_id,
      type_name: row.type_name.clone(),
      value: row.value,
    })
    .collect()
}

fn category_label(category: &str) -> String {
  let mut chars = category.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    None => "Other".to_owned(),
  }
}

fn owner_label(owner_id: i64, roster: &[RosterPilot]) -> String {
  roster
    .iter()
    .find(|pilot| pilot.id == owner_id)
    .map(|pilot| pilot.name.clone())
    .unwrap_or_else(|| format!("Owner {owner_id}"))
}

pub(super) fn body(summary: &ValueSummary) -> Element<'_, Message> {
  if summary.total_value <= 0.0 && summary.matrix_rows.is_empty() {
    return empty_state();
  }

  let left = Column::with_children(vec![matrix_card(summary)])
    .width(Length::FillPortion(3))
    .spacing(spacing::SPACE_3);

  let right = Column::with_children(vec![category_card(summary), top_items_card(summary)])
    .width(Length::FillPortion(2))
    .spacing(spacing::SPACE_3);

  container(
    Row::with_children(vec![left.into(), right.into()])
      .spacing(spacing::SPACE_6)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_6 + spacing::SPACE_2,
    left: HEADER_SIDE_PADDING,
  })
  .into()
}

fn matrix_card(summary: &ValueSummary) -> Element<'_, Message> {
  let mut header_cells: Vec<Element<'_, Message>> = vec![matrix_label_cell("Character", Horizontal::Left)];
  for location in &summary.matrix_locations {
    let label = location
      .label
      .clone()
      .unwrap_or_else(|| format!("Loc {}", location.location_id));
    header_cells.push(matrix_label_cell(&label, Horizontal::Right));
  }
  header_cells.push(matrix_label_cell("Total", Horizontal::Right));

  let mut rows: Vec<Element<'_, Message>> = vec![
    container(Row::with_children(header_cells).spacing(spacing::SPACE_3))
      .padding(Padding {
        top: spacing::SPACE_2,
        right: spacing::SPACE_3_5,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3_5,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        ..container::Style::default()
      })
      .into(),
  ];

  for matrix_row in &summary.matrix_rows {
    let mut cells: Vec<Element<'_, Message>> = vec![matrix_text_cell(
      &matrix_row.owner_label,
      Horizontal::Left,
      color::text::PRIMARY,
    )];
    for location in &summary.matrix_locations {
      let value = matrix_row.cells.get(&location.location_id).copied().unwrap_or(0.0);
      let label = if value <= 0.0 {
        "\u{2014}".to_owned()
      } else {
        fmt_isk(value)
      };
      let cell_color = if value <= 0.0 {
        color::text::TERTIARY
      } else {
        color::text::PRIMARY
      };
      cells.push(matrix_numeric_cell(label, cell_color));
    }
    cells.push(matrix_numeric_cell(fmt_isk(matrix_row.total), color::text::PRIMARY));
    rows.push(
      container(Row::with_children(cells).spacing(spacing::SPACE_3))
        .padding(Padding {
          top: spacing::SPACE_2_5,
          right: spacing::SPACE_3_5,
          bottom: spacing::SPACE_2_5,
          left: spacing::SPACE_3_5,
        })
        .style(|_| container::Style {
          border: Border {
            color: color::with_alpha(color::text::PRIMARY, 0.06),
            width: 1.0,
            radius: 0.0.into(),
          },
          ..container::Style::default()
        })
        .into(),
    );
  }

  let mut footer_cells: Vec<Element<'_, Message>> = vec![matrix_label_cell("Column total", Horizontal::Left)];
  for location in &summary.matrix_locations {
    let column_total: f64 = summary
      .matrix_rows
      .iter()
      .map(|r| r.cells.get(&location.location_id).copied().unwrap_or(0.0))
      .sum();
    footer_cells.push(matrix_numeric_cell(fmt_isk(column_total), color::accent::PLASMA));
  }
  footer_cells.push(matrix_numeric_cell(fmt_isk(summary.total_value), color::accent::PLASMA));
  rows.push(
    container(Row::with_children(footer_cells).spacing(spacing::SPACE_3))
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
      .into(),
  );

  card(
    "Value \u{b7} character \u{d7} location",
    Column::with_children(rows).width(Length::Fill).into(),
  )
}

fn matrix_label_cell<'a>(label: &str, align: Horizontal) -> Element<'a, Message> {
  container(eyebrow(label, Some(color::text::SECONDARY)))
    .width(Length::Fill)
    .align_x(align)
    .into()
}

fn matrix_text_cell<'a>(label: &str, align: Horizontal, text_color: iced::Color) -> Element<'a, Message> {
  container(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(text_color),
      }),
  )
  .width(Length::Fill)
  .align_x(align)
  .into()
}

fn matrix_numeric_cell<'a>(label: String, text_color: iced::Color) -> Element<'a, Message> {
  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(text_color),
      }),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Right)
  .into()
}

fn category_card(summary: &ValueSummary) -> Element<'_, Message> {
  let total = summary.total_value.max(1.0);

  let bar_segments: Vec<Element<'_, Message>> = summary
    .by_category
    .iter()
    .enumerate()
    .map(|(index, slice)| {
      let portion = (slice.value / total).max(0.0) as u16;
      container(Space::new().height(Length::Fill))
        .width(Length::FillPortion(portion.max(1)))
        .height(Length::Fixed(spacing::SPACE_2_5))
        .style(move |_| container::Style {
          background: Some(Background::Color(color::chart::series(index))),
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  let bar = container(Row::with_children(bar_segments).width(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(spacing::SPACE_2_5))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let mut rows: Vec<Element<'_, Message>> = vec![bar.into()];
  for (index, slice) in summary.by_category.iter().enumerate() {
    let pct = slice.value / total * 100.0;
    rows.push(
      Row::with_children(vec![
        container(Space::new())
          .width(Length::Fixed(spacing::SPACE_2_5))
          .height(Length::Fixed(spacing::SPACE_2_5))
          .style(move |_| container::Style {
            background: Some(Background::Color(color::chart::series(index))),
            border: Border {
              radius: radius::SUBTLE.into(),
              ..Border::default()
            },
            ..container::Style::default()
          })
          .into(),
        container(
          text(slice.label.clone())
            .font(typography::body::REGULAR)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(color::text::PRIMARY),
            }),
        )
        .width(Length::Fill)
        .into(),
        text(fmt_isk(slice.value))
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        text(format!("{pct:.1}%"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(|_| text::Style {
            color: Some(color::text::TERTIARY),
          })
          .into(),
      ])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center)
      .into(),
    );
  }

  card(
    "By category",
    Column::with_children(rows)
      .spacing(spacing::SPACE_2)
      .width(Length::Fill)
      .into(),
  )
}

fn top_items_card(summary: &ValueSummary) -> Element<'_, Message> {
  let mut rows: Vec<Element<'_, Message>> = Vec::new();
  for (index, item) in summary.top_items.iter().enumerate() {
    rows.push(
      Row::with_children(vec![
        text(format!("{:02}", index + 1))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(|_| text::Style {
            color: Some(color::text::TERTIARY),
          })
          .into(),
        top_item_icon(item.type_id),
        Column::with_children(vec![
          text(item.type_name.clone())
            .font(typography::body::REGULAR)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(color::text::PRIMARY),
            })
            .into(),
          text(format!("{} \u{b7} \u{d7}{}", item.group_name, item.quantity))
            .font(typography::mono::REGULAR)
            .size(typography::size::XS)
            .style(|_| text::Style {
              color: Some(color::text::SECONDARY),
            })
            .into(),
        ])
        .spacing(1.0)
        .width(Length::Fill)
        .into(),
        text(fmt_isk(item.value))
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::accent::PLASMA),
          })
          .into(),
      ])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center)
      .into(),
    );
  }

  card(
    "Top items by value",
    Column::with_children(rows)
      .spacing(spacing::SPACE_2)
      .width(Length::Fill)
      .into(),
  )
}

fn top_item_icon<'a>(type_id: i64) -> Element<'a, Message> {
  let content: Element<'a, Message> = match images::default_store().resolve_type_icon(type_id, None, ICON_SIZE) {
    IconResolution::Found(path) => image(image::Handle::from_path(path))
      .width(Length::Fill)
      .height(Length::Fill)
      .content_fit(iced::ContentFit::Contain)
      .into(),
    IconResolution::Missing => Space::new().into(),
  };
  icon_tile(content, TOP_ITEM_ICON)
}

fn card<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
  let heading = text(title)
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  card_padded(
    Column::with_children(vec![heading.into(), body])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
    spacing::SPACE_3_5,
  )
}

fn empty_state<'a>() -> Element<'a, Message> {
  shared_empty_state("No valued assets in this scope.").render()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn row(type_name: &str, category: &str, owner_id: i64, location_id: i64, quantity: i64, value: f64) -> InventoryRow {
    labeled_row(type_name, category, owner_id, location_id, None, quantity, value)
  }

  fn labeled_row(
    type_name: &str,
    category: &str,
    owner_id: i64,
    location_id: i64,
    location_label: Option<&str>,
    quantity: i64,
    value: f64,
  ) -> InventoryRow {
    InventoryRow {
      category: category.to_owned(),
      container_id: None,
      depth: 0,
      group_name: format!("{type_name} Group"),
      is_active_ship: false,
      is_blueprint_copy: None,
      is_container: false,
      item_id: value as i64,
      location_id,
      location_label: location_label.map(str::to_owned),
      name: None,
      owner_id,
      quantity,
      row_volume: 10.0,
      type_id: 587,
      type_name: type_name.to_owned(),
      unit_price: value / quantity as f64,
      value,
    }
  }

  fn pilot(id: i64, name: &str) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      id,
      name: name.to_owned(),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  mod summarize {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_aggregates_by_category_location_matrix_and_top_items() {
      let rows = vec![
        row("Rifter", "ship", 7, 60_003_760, 1, 1_000.0),
        row("Tritanium", "commodity", 7, 60_003_760, 100, 500.0),
        row("Rupture", "ship", 8, 60_008_494, 1, 2_000.0),
      ];
      let roster = vec![pilot(7, "Vex"), pilot(8, "Korren")];

      let summary = summarize(&rows, &roster);

      assert_eq!(summary.total_value, 3_500.0);
      assert_eq!(summary.by_category[0].label, "Ship");
      assert_eq!(summary.by_category[0].value, 3_000.0);
      assert_eq!(summary.by_category[1].label, "Commodity");
      assert_eq!(summary.matrix_rows[0].owner_label, "Korren");
      assert_eq!(summary.matrix_rows[0].total, 2_000.0);
      assert_eq!(summary.matrix_rows[1].owner_label, "Vex");
      assert_eq!(summary.matrix_rows[1].total, 1_500.0);
      assert_eq!(summary.top_items[0].type_name, "Rupture");
      assert_eq!(summary.top_items[0].value, 2_000.0);
    }

    #[test]
    fn it_yields_an_empty_summary_for_no_rows() {
      let summary = summarize(&[], &[]);
      assert_eq!(summary, ValueSummary::default());
    }
  }

  mod by_location {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_threads_the_location_label_onto_each_value() {
      let rows = vec![labeled_row(
        "Rifter",
        "ship",
        7,
        60_003_760,
        Some("Jita IV - Moon 4"),
        1,
        1_000.0,
      )];

      let out = super::super::by_location(&rows);

      assert_eq!(out[0].location_id, 60_003_760);
      assert_eq!(out[0].label.as_deref(), Some("Jita IV - Moon 4"));
    }

    #[test]
    fn it_leaves_the_label_absent_when_no_row_carries_one() {
      let rows = vec![row("Rifter", "ship", 7, 60_003_760, 1, 1_000.0)];

      let out = super::super::by_location(&rows);

      assert_eq!(out[0].label, None);
    }
  }

  mod matrix_locations {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_the_label_and_id_for_each_column() {
      let rows = vec![
        labeled_row("Rifter", "ship", 7, 60_003_760, Some("Jita IV - Moon 4"), 1, 1_000.0),
        labeled_row("Rupture", "ship", 8, 60_008_494, None, 1, 2_000.0),
      ];

      let out = super::super::matrix_locations(&rows);

      assert_eq!(out[0].location_id, 60_008_494);
      assert_eq!(out[0].label, None);
      assert_eq!(out[1].location_id, 60_003_760);
      assert_eq!(out[1].label.as_deref(), Some("Jita IV - Moon 4"));
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_values_body_from_a_sample_summary() {
      let summary = summarize(
        &[
          row("Rifter", "ship", 7, 60_003_760, 1, 1_000.0),
          row("Rupture", "ship", 8, 60_008_494, 1, 2_000.0),
        ],
        &[pilot(7, "Vex"), pilot(8, "Korren")],
      );
      let _el: Element<'_, Message> = body(&summary);
    }

    #[test]
    fn it_renders_the_empty_values_body() {
      let summary = ValueSummary::default();
      let _el: Element<'_, Message> = body(&summary);
    }
  }
}
