use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use super::{Blueprint, BlueprintKind, BlueprintSort, Message, State};
use crate::{
  clients::eve_image::Size,
  store::images::{self, IconResolution},
  ui::{
    components::{
      badge::badge,
      clip::clip_layer,
      icon::Icon,
      icon_tile::icon_tile,
      rule,
      text_input::TextInput,
      virtual_list::{self, VirtualList, VirtualListConfig},
    },
    style::{color, radius, spacing, typography},
  },
};

/// Estimated row height (px) feeding the [`VirtualList`] windowing math; rows are two-line and fixed-padded.
const ESTIMATED_ROW_HEIGHT: f32 = 62.0;
const ME_PIPS: i64 = 10;
const ME_MAX: i64 = 10;
const ROW_SIDE_PADDING: f32 = 24.0;
const SEARCH_MAX_WIDTH: f32 = 360.0;
const TE_MAX: i64 = 20;
const TILE_BOX: f32 = 34.0;
const TILE_ICON: Size = Size::S64;

// Column widths mirror the design grid `1fr 96px 96px 130px 150px`.
const COL_LOCATION: f32 = 150.0;
const COL_ME: f32 = 96.0;
const COL_RUNS: f32 = 130.0;
const COL_TE: f32 = 96.0;

/// TE dot-meter fill (`T.info`, #7B8BD9); no shared token matches, so the periwinkle blue lives here.
const TE_FILL: Color = Color {
  r: 0.482,
  g: 0.545,
  b: 0.851,
  a: 1.0,
};

pub(super) fn tab(state: &State) -> Element<'_, Message> {
  let scoped = state.visible_blueprints();
  let counts = Counts::of(&scoped);
  let filtered = filter_and_sort(
    &scoped,
    state.blueprint_kind(),
    state.blueprint_search(),
    state.blueprint_sort(),
  );

  let body: Element<'_, Message> = if filtered.is_empty() {
    empty_state()
  } else {
    let offset = state.blueprint_scroll_offset();
    virtual_list::responsive_window(move |viewport_height| {
      let config = VirtualListConfig::new(filtered.len(), ESTIMATED_ROW_HEIGHT)
        .viewport_height(viewport_height)
        .scroll_offset(offset);
      let windowed = VirtualList::new(config, |index| blueprint_row(filtered[index])).view();
      scrollable(windowed)
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_scroll(|viewport| Message::BlueprintScrolled {
          absolute: viewport.absolute_offset().y,
        })
        .into()
    })
  };

  Column::with_children(vec![
    toolbar(state, counts),
    rule::horizontal(),
    column_header(),
    rule::horizontal(),
    container(body).width(Length::Fill).height(Length::Fill).into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn toolbar<'a>(state: &'a State, counts: Counts) -> Element<'a, Message> {
  let search = container(
    TextInput::new(
      "Search blueprints\u{2026}",
      state.blueprint_search(),
      Message::BlueprintSearchChanged,
    )
    .leading_icon(Icon::search())
    .background(color::surface::SUNKEN)
    .width(Length::Fill)
    .render(),
  )
  .max_width(SEARCH_MAX_WIDTH)
  .width(Length::Fill);

  let kinds = [
    (BlueprintKind::All, "All", counts.originals + counts.copies),
    (BlueprintKind::Originals, "Originals", counts.originals),
    (BlueprintKind::Copies, "Copies", counts.copies),
  ];
  let kind_buttons: Vec<Element<'a, Message>> = kinds
    .into_iter()
    .map(|(kind, label, count)| kind_button(label, kind, count, state.blueprint_kind() == kind))
    .collect();
  let kind_group = container(Row::with_children(kind_buttons).spacing(spacing::UNIT))
    .padding(3.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let sorts = [
    (BlueprintSort::Name, "Name"),
    (BlueprintSort::MaterialEfficiency, "ME"),
    (BlueprintSort::Runs, "Runs"),
  ];
  let sort_buttons: Vec<Element<'a, Message>> = sorts
    .into_iter()
    .map(|(sort, label)| sort_button(label, sort, state.blueprint_sort() == sort))
    .collect();

  let band = Row::with_children(vec![
    search.into(),
    kind_group.into(),
    Space::new().width(Length::Fill).into(),
    text("SORT")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Row::with_children(sort_buttons)
      .spacing(2.0)
      .align_y(Vertical::Center)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(band)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: ROW_SIDE_PADDING,
      right: ROW_SIDE_PADDING,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn kind_button<'a>(label: &str, kind: BlueprintKind, count: usize, active: bool) -> Element<'a, Message> {
  let fill = if active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let inner = Row::with_children(vec![
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(fill))
      .into(),
    text(count.to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(fill))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(inner)
    .padding(Padding {
      top: spacing::UNIT + 1.0,
      bottom: spacing::UNIT + 1.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::BlueprintKindSelected(kind))
    .style(move |_, _| button::Style {
      background: active.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.14))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      text_color: fill,
      ..button::Style::default()
    })
    .into()
}

fn sort_button<'a>(label: &str, sort: BlueprintSort, active: bool) -> Element<'a, Message> {
  let text_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };
  button(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: spacing::UNIT + 1.0,
    bottom: spacing::UNIT + 1.0,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  })
  .on_press(Message::BlueprintSortSelected(sort))
  .style(move |_, _| button::Style {
    background: active.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
    border: Border {
      color: if active {
        color::rule_strong()
      } else {
        Color::TRANSPARENT
      },
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn column_header<'a>() -> Element<'a, Message> {
  let head = |label: &str, right: bool| -> Element<'a, Message> {
    aligned(
      text(label.to_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary())),
      right,
    )
  };

  let band = Row::with_children(vec![
    container(head("BLUEPRINT", false)).width(Length::Fill).into(),
    column(head("MATERIAL EFF.", false), COL_ME),
    column(head("TIME EFF.", false), COL_TE),
    column(head("RUNS", true), COL_RUNS),
    column(head("LOCATION", true), COL_LOCATION),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(band)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: ROW_SIDE_PADDING,
      right: ROW_SIDE_PADDING,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn blueprint_row<'a>(blueprint: &'a Blueprint) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    container(identity(blueprint)).width(Length::Fill).into(),
    column(
      efficiency_cell(
        "ME",
        blueprint.material_efficiency,
        ME_MAX,
        color::accent::PLASMA,
        blueprint.reaction,
      ),
      COL_ME,
    ),
    column(
      efficiency_cell("TE", blueprint.time_efficiency, TE_MAX, TE_FILL, blueprint.reaction),
      COL_TE,
    ),
    column(runs_cell(blueprint), COL_RUNS),
    column(location_cell(blueprint), COL_LOCATION),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: ROW_SIDE_PADDING,
      right: ROW_SIDE_PADDING,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn identity<'a>(blueprint: &'a Blueprint) -> Element<'a, Message> {
  let name_row = Row::with_children(vec![
    text(blueprint.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    kind_badge(blueprint.is_original()),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let subtitle = match (blueprint.group_name.is_empty(), &blueprint.product_name) {
    (false, Some(product)) => format!("{} \u{00B7} makes {product}", blueprint.group_name),
    (false, None) => blueprint.group_name.clone(),
    (true, Some(product)) => format!("makes {product}"),
    (true, None) => String::new(),
  };

  let details = Column::with_children(vec![
    name_row.into(),
    text(subtitle)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  Row::with_children(vec![blueprint_tile(blueprint), details.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn kind_badge<'a>(original: bool) -> Element<'a, Message> {
  if original {
    badge("BPO", Some(color::accent::PLASMA))
  } else {
    badge("BPC", Some(color::status::WARNING))
  }
}

fn blueprint_tile<'a>(blueprint: &Blueprint) -> Element<'a, Message> {
  // Resolve the BPO icon, or the `_bpc` variant for a copy.
  let copy = !blueprint.is_original();
  match images::default_store().resolve_type_icon(blueprint.type_id, Some(copy), TILE_ICON) {
    IconResolution::Found(path) => icon_tile(
      clip_layer(
        image(image::Handle::from_path(path))
          .width(Length::Fill)
          .height(Length::Fill)
          .content_fit(ContentFit::Cover),
        Length::Fill,
        Length::Fill,
      ),
      TILE_BOX,
    ),
    IconResolution::Missing => icon_tile(
      Icon::copy()
        .color(color::text::tertiary())
        .size(TILE_BOX * 0.45)
        .render::<Message>(),
      TILE_BOX,
    ),
  }
}

fn efficiency_cell<'a>(label: &str, value: i64, max: i64, fill: Color, reaction: bool) -> Element<'a, Message> {
  if reaction {
    return text("n/a")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into();
  }

  Column::with_children(vec![
    text(format!("{label} {value}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    dot_meter(value, max, fill),
  ])
  .spacing(spacing::UNIT + 1.0)
  .into()
}

/// A 10-pip dot meter; the number of filled pips is `round(value / max * 10)`, clamped to the pip count.
fn dot_meter<'a>(value: i64, max: i64, fill: Color) -> Element<'a, Message> {
  let max = max.max(1);
  let filled = (((value.max(0) as f32 / max as f32) * ME_PIPS as f32).round() as i64).clamp(0, ME_PIPS);

  let pips: Vec<Element<'a, Message>> = (0..ME_PIPS)
    .map(|index| {
      let on = index < filled;
      container(Space::new())
        .width(Length::Fixed(5.0))
        .height(Length::Fixed(5.0))
        .style(move |_| container::Style {
          background: Some(Background::Color(if on {
            fill
          } else {
            color::with_alpha(color::text::PRIMARY, 0.12)
          })),
          border: Border {
            radius: 2.5.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  Row::with_children(pips).spacing(2.5).align_y(Vertical::Center).into()
}

fn runs_cell<'a>(blueprint: &Blueprint) -> Element<'a, Message> {
  if blueprint.is_original() {
    aligned(
      Row::with_children(vec![
        text("\u{221E}")
          .font(typography::mono::REGULAR)
          .size(typography::size::LG)
          .style(typography::colored(color::accent::PLASMA))
          .into(),
        text("ORIGINAL")
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::secondary()))
          .into(),
      ])
      .spacing(spacing::UNIT)
      .align_y(Vertical::Center),
      true,
    )
  } else {
    Column::with_children(vec![
      text(fmt_num(blueprint.runs))
        .font(typography::mono::REGULAR)
        .size(typography::size::LG)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      text("RUNS LEFT")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Right)
    .width(Length::Fill)
    .into()
  }
}

fn location_cell<'a>(blueprint: &Blueprint) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    text(blueprint.location.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if let Some(system) = &blueprint.system_name {
    children.push(
      text(system.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  Column::with_children(children)
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Right)
    .width(Length::Fill)
    .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("No blueprints found.")
      .font(typography::body::REGULAR)
      .size(typography::size::LG)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6)
  .into()
}

fn column<'a>(content: Element<'a, Message>, width: f32) -> Element<'a, Message> {
  container(content).width(Length::Fixed(width)).into()
}

/// Wrap an element in a full-width container aligned left or right.
fn aligned<'a>(content: impl Into<Element<'a, Message>>, right: bool) -> Element<'a, Message> {
  let align = if right { Horizontal::Right } else { Horizontal::Left };
  container(content.into()).width(Length::Fill).align_x(align).into()
}

fn fmt_num(value: i64) -> String {
  let mut out = String::new();
  let digits = value.abs().to_string();
  let bytes = digits.as_bytes();
  for (index, byte) in bytes.iter().enumerate() {
    if index > 0 && (bytes.len() - index).is_multiple_of(3) {
      out.push(',');
    }
    out.push(*byte as char);
  }
  if value < 0 { format!("-{out}") } else { out }
}

/// Effective sort key for runs: a BPO (infinite) sorts above every finite-run BPC.
fn runs_key(blueprint: &Blueprint) -> i64 {
  if blueprint.is_original() {
    i64::MAX
  } else {
    blueprint.runs
  }
}

fn filter_and_sort<'a>(
  blueprints: &[&'a Blueprint],
  kind: BlueprintKind,
  query: &str,
  sort: BlueprintSort,
) -> Vec<&'a Blueprint> {
  let needle = query.trim().to_lowercase();
  let mut out: Vec<&'a Blueprint> = blueprints
    .iter()
    .copied()
    .filter(|blueprint| match kind {
      BlueprintKind::All => true,
      BlueprintKind::Originals => blueprint.is_original(),
      BlueprintKind::Copies => !blueprint.is_original(),
    })
    .filter(|blueprint| needle.is_empty() || matches_query(blueprint, &needle))
    .collect();

  out.sort_by(|a, b| match sort {
    BlueprintSort::Name => a
      .name
      .to_lowercase()
      .cmp(&b.name.to_lowercase())
      .then(a.item_id.cmp(&b.item_id)),
    BlueprintSort::MaterialEfficiency => b
      .material_efficiency
      .cmp(&a.material_efficiency)
      .then(a.name.to_lowercase().cmp(&b.name.to_lowercase())),
    BlueprintSort::Runs => runs_key(b)
      .cmp(&runs_key(a))
      .then(a.name.to_lowercase().cmp(&b.name.to_lowercase())),
  });
  out
}

fn matches_query(blueprint: &Blueprint, needle: &str) -> bool {
  blueprint.name.to_lowercase().contains(needle)
    || blueprint
      .product_name
      .as_deref()
      .is_some_and(|product| product.to_lowercase().contains(needle))
    || blueprint
      .system_name
      .as_deref()
      .is_some_and(|system| system.to_lowercase().contains(needle))
}

#[derive(Clone, Copy)]
struct Counts {
  copies: usize,
  originals: usize,
}

impl Counts {
  fn of(blueprints: &[&Blueprint]) -> Self {
    let originals = blueprints.iter().filter(|blueprint| blueprint.is_original()).count();
    Counts {
      copies: blueprints.len() - originals,
      originals,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn blueprint(item_id: i64, name: &str, runs: i64, me: i64, product: Option<&str>, system: Option<&str>) -> Blueprint {
    Blueprint {
      group_name: "Frigate Blueprint".to_owned(),
      item_id,
      location: "Jita IV - Moon 4".to_owned(),
      material_efficiency: me,
      name: name.to_owned(),
      owner: super::super::Owner::Character(1),
      product_name: product.map(str::to_owned),
      reaction: false,
      runs,
      system_name: system.map(str::to_owned),
      time_efficiency: 0,
      type_id: 681,
    }
  }

  fn sample() -> Vec<Blueprint> {
    vec![
      blueprint(1, "Rifter Blueprint", -1, 10, Some("Rifter"), Some("Jita")),
      blueprint(2, "Hobgoblin I Blueprint", 12, 4, Some("Hobgoblin I"), Some("Amarr")),
      blueprint(
        3,
        "Cap Booster Blueprint",
        300,
        8,
        Some("Cap Booster 400"),
        Some("Rens"),
      ),
    ]
  }

  fn refs(blueprints: &[Blueprint]) -> Vec<&Blueprint> {
    blueprints.iter().collect()
  }

  mod filter_and_sort {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_filters_to_originals_and_copies() {
      let data = sample();
      let originals = super::super::filter_and_sort(&refs(&data), BlueprintKind::Originals, "", BlueprintSort::Name);
      let copies = super::super::filter_and_sort(&refs(&data), BlueprintKind::Copies, "", BlueprintSort::Name);

      assert_eq!(originals.len(), 1);
      assert!(originals[0].is_original());
      assert_eq!(copies.len(), 2);
      assert!(copies.iter().all(|bp| !bp.is_original()));
    }

    #[test]
    fn it_searches_name_product_and_system() {
      let data = sample();

      let by_name = super::super::filter_and_sort(&refs(&data), BlueprintKind::All, "rifter", BlueprintSort::Name);
      let by_product =
        super::super::filter_and_sort(&refs(&data), BlueprintKind::All, "cap booster", BlueprintSort::Name);
      let by_system = super::super::filter_and_sort(&refs(&data), BlueprintKind::All, "amarr", BlueprintSort::Name);

      assert_eq!(by_name.len(), 1);
      assert_eq!(by_name[0].item_id, 1);
      assert_eq!(by_product.len(), 1);
      assert_eq!(by_product[0].item_id, 3);
      assert_eq!(by_system.len(), 1);
      assert_eq!(by_system[0].item_id, 2);
    }

    #[test]
    fn it_sorts_originals_first_by_runs() {
      let data = sample();

      let sorted = super::super::filter_and_sort(&refs(&data), BlueprintKind::All, "", BlueprintSort::Runs);

      // BPO (infinite) leads, then the 300-run copy, then the 12-run copy.
      assert_eq!(sorted.iter().map(|bp| bp.item_id).collect::<Vec<_>>(), vec![1, 3, 2]);
    }

    #[test]
    fn it_sorts_by_material_efficiency_descending() {
      let data = sample();

      let sorted =
        super::super::filter_and_sort(&refs(&data), BlueprintKind::All, "", BlueprintSort::MaterialEfficiency);

      assert_eq!(
        sorted.iter().map(|bp| bp.material_efficiency).collect::<Vec<_>>(),
        vec![10, 8, 4]
      );
    }

    #[test]
    fn it_sorts_by_name_alphabetically() {
      let data = sample();

      let sorted = super::super::filter_and_sort(&refs(&data), BlueprintKind::All, "", BlueprintSort::Name);

      assert_eq!(
        sorted.iter().map(|bp| bp.name.as_str()).collect::<Vec<_>>(),
        vec!["Cap Booster Blueprint", "Hobgoblin I Blueprint", "Rifter Blueprint"]
      );
    }
  }

  mod counts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_originals_and_copies() {
      let data = sample();

      let counts = Counts::of(&refs(&data));

      assert_eq!(counts.originals, 1);
      assert_eq!(counts.copies, 2);
    }
  }

  mod fmt_num {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_groups_thousands_with_commas() {
      assert_eq!(super::super::fmt_num(0), "0");
      assert_eq!(super::super::fmt_num(300), "300");
      assert_eq!(super::super::fmt_num(12_345), "12,345");
      assert_eq!(super::super::fmt_num(1_000_000), "1,000,000");
    }
  }

  mod rows {
    use super::*;

    #[test]
    fn it_renders_a_bpo_a_bpc_and_a_reaction_row() {
      let bpo = blueprint(1, "Rifter Blueprint", -1, 10, Some("Rifter"), Some("Jita"));
      let bpc = blueprint(2, "Hobgoblin I Blueprint", 12, 4, Some("Hobgoblin I"), Some("Amarr"));
      let mut reaction = blueprint(3, "Sulfuric Acid Reaction", -1, 0, Some("Sulfuric Acid"), None);
      reaction.reaction = true;

      for bp in [&bpo, &bpc, &reaction] {
        let _el: Element<'_, Message> = super::super::blueprint_row(bp);
      }
    }
  }
}
