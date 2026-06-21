use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, Space, container, scrollable, text},
};

use super::{
  Composition, CorpDivision, CorpWalletSection, HEADER_SIDE_PADDING, Message, RosterPilot, Scope, State, WalletSort,
  scope_composition,
};
use crate::ui::{
  components::{avatar::Avatar, eyebrow::eyebrow_text, icon::Icon, segmented::segment_button},
  format::fmt_isk_full,
  style::{color, radius, spacing, typography},
};

const BODY_MAX_WIDTH: f32 = 1180.0;
const SHARE_BAR_WIDTH: f32 = 150.0;
const ISK_COLUMN_WIDTH: f32 = 168.0;
const SWATCH_SIZE: f32 = 30.0;

struct RowData {
  balance: f64,
  indent: bool,
  name: String,
  sub: Option<String>,
  swatch: Option<RowSwatch>,
}

struct RowSwatch {
  id: i64,
  name: String,
  path: Option<std::path::PathBuf>,
}

struct Section {
  caption: Option<String>,
  rows: Vec<RowData>,
  subtotal: f64,
  swatch: SectionSwatch,
  title: String,
}

enum SectionSwatch {
  Corp {
    id: i64,
    name: String,
    path: Option<std::path::PathBuf>,
  },
  Personal,
}

pub(super) fn surface<'a>(state: &State) -> Element<'a, Message> {
  let sections = sections_for(state);
  let context = context_label(state, &sections);
  let sort = state.wallets_sort();

  let mut body: Vec<Element<'a, Message>> = Vec::new();
  if let Scope::Character(id) = state.active()
    && let Some(pilot) = state.roster().iter().find(|pilot| pilot.id == id)
  {
    body.push(pilot_hero(pilot, scope_composition(state)));
  }
  let empty = sections.is_empty();
  for section in sections {
    body.push(section_card(section));
  }
  if empty {
    body.push(empty_state());
  }

  let column = container(Column::with_children(body).spacing(20.0).width(Length::Fill))
    .width(Length::Fill)
    .max_width(BODY_MAX_WIDTH)
    .padding(Padding {
      top: 20.0,
      right: HEADER_SIDE_PADDING,
      bottom: 48.0,
      left: HEADER_SIDE_PADDING,
    });
  let scrolled = scrollable(container(column).center_x(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  Column::with_children(vec![toolbar(context, sort), scrolled.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn sections_for(state: &State) -> Vec<Section> {
  let sort = state.wallets_sort();
  match state.active() {
    Scope::All => {
      let mut sections = vec![personal_section(state, sort)];
      sections.extend(
        state
          .wallet_sections()
          .iter()
          .map(|section| corp_section(section, sort)),
      );
      sections
    }
    Scope::Character(id) => match state.roster().iter().find(|pilot| pilot.id == id) {
      Some(pilot) => vec![single_pilot_section(pilot)],
      None => Vec::new(),
    },
    Scope::Corporation(id) => state
      .wallet_sections()
      .iter()
      .find(|section| section.id == id)
      .map(|section| vec![corp_section(section, sort)])
      .unwrap_or_default(),
  }
}

fn personal_section(state: &State, sort: WalletSort) -> Section {
  let mut rows: Vec<RowData> = state
    .roster()
    .iter()
    .map(|pilot| RowData {
      balance: pilot.liquid.unwrap_or(0.0),
      indent: false,
      name: pilot.name.clone(),
      sub: (!pilot.corp.is_empty()).then(|| pilot.corp.clone()),
      swatch: Some(RowSwatch {
        id: pilot.id,
        name: pilot.name.clone(),
        path: pilot.portrait.path(),
      }),
    })
    .collect();
  sort_rows(&mut rows, sort);
  let subtotal = rows.iter().map(|row| row.balance).sum();

  Section {
    caption: Some("Liquid ISK across your pilots".to_owned()),
    rows,
    subtotal,
    swatch: SectionSwatch::Personal,
    title: "Personal Wallets".to_owned(),
  }
}

fn single_pilot_section(pilot: &RosterPilot) -> Section {
  let balance = pilot.liquid.unwrap_or(0.0);
  let caption = if pilot.corp.is_empty() {
    "Personal wallet".to_owned()
  } else {
    format!("{} \u{00b7} personal wallet", pilot.corp)
  };

  Section {
    caption: Some(caption),
    rows: vec![RowData {
      balance,
      indent: false,
      name: "Master Wallet".to_owned(),
      sub: Some("Personal \u{00b7} liquid ISK".to_owned()),
      swatch: None,
    }],
    subtotal: balance,
    swatch: SectionSwatch::Personal,
    title: pilot.name.clone(),
  }
}

fn corp_section(section: &CorpWalletSection, sort: WalletSort) -> Section {
  let mut rows: Vec<RowData> = section
    .divisions
    .iter()
    .map(|division| RowData {
      balance: division.balance.unwrap_or(0.0),
      indent: true,
      name: division_label(division),
      sub: None,
      swatch: None,
    })
    .collect();
  sort_rows(&mut rows, sort);

  Section {
    caption: Some(corp_caption(section)),
    rows,
    subtotal: section.subtotal(),
    swatch: SectionSwatch::Corp {
      id: section.id,
      name: section.name.clone(),
      path: section.logo.path(),
    },
    title: section.name.clone(),
  }
}

fn corp_caption(section: &CorpWalletSection) -> String {
  match (section.granted_by.as_deref(), section.role.as_deref()) {
    (Some(name), Some(role)) => {
      format!("Corporation wallet \u{00b7} via {name} \u{00b7} {role}")
    }
    (Some(name), None) => format!("Corporation wallet \u{00b7} via {name}"),
    _ => "Corporation wallet".to_owned(),
  }
}

fn division_label(division: &CorpDivision) -> String {
  division.name.clone().unwrap_or_else(|| match division.division {
    1 => "Master Wallet".to_owned(),
    2 => "2nd Wallet".to_owned(),
    3 => "3rd Wallet".to_owned(),
    other => format!("{other}th Wallet"),
  })
}

fn sort_rows(rows: &mut [RowData], sort: WalletSort) {
  rows.sort_by(|a, b| match sort {
    WalletSort::Ascending => a.balance.total_cmp(&b.balance),
    WalletSort::Descending => b.balance.total_cmp(&a.balance),
  });
}

fn toolbar<'a>(context: String, sort: WalletSort) -> Element<'a, Message> {
  let showing = Row::with_children(vec![
    eyebrow_text("Showing", None).into(),
    text(context)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let toggle = container(
    Row::with_children(vec![
      sort_button(
        "High \u{2192} Low",
        sort == WalletSort::Descending,
        WalletSort::Descending,
      ),
      sort_divider(),
      sort_button(
        "Low \u{2192} High",
        sort == WalletSort::Ascending,
        WalletSort::Ascending,
      ),
    ])
    .align_y(Vertical::Center),
  )
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 7.0.into(),
    },
    ..container::Style::default()
  })
  .clip(true);

  let sort_group = Row::with_children(vec![eyebrow_text("Sort", None).into(), toggle.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  container(
    Row::with_children(vec![
      showing.into(),
      Space::new().width(Length::Fill).into(),
      sort_group.into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_3,
    left: HEADER_SIDE_PADDING,
  })
  .style(crate::ui::style::control::bordered_pane)
  .into()
}

fn context_label(state: &State, sections: &[Section]) -> String {
  match state.active() {
    Scope::All => {
      let pilots = state.roster().len();
      let corps = state.wallet_sections().len();
      format!(
        "All wallets \u{00b7} {pilots} {} + {corps} corp {}",
        plural(pilots, "pilot", "pilots"),
        plural(corps, "wallet", "wallets"),
      )
    }
    Scope::Character(_) => sections
      .first()
      .map(|section| format!("{} \u{00b7} personal wallet", section.title))
      .unwrap_or_else(|| "Personal wallet".to_owned()),
    Scope::Corporation(_) => sections
      .first()
      .map(|section| format!("{} \u{00b7} corporation wallet", section.title))
      .unwrap_or_else(|| "Corporation wallet".to_owned()),
  }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
  if count == 1 { singular } else { plural }
}

fn sort_button<'a>(label: &'a str, active: bool, sort: WalletSort) -> Element<'a, Message> {
  segment_button(
    label,
    active,
    Padding {
      top: 7.0,
      right: 13.0,
      bottom: 7.0,
      left: 13.0,
    },
    Message::WalletsSortSelected(sort),
  )
}

fn sort_divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fixed(28.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn pilot_hero<'a>(pilot: &RosterPilot, composition: Composition) -> Element<'a, Message> {
  let liquid = composition.liquid.unwrap_or(0.0);
  let assets = composition.asset_value.unwrap_or(0.0);
  let escrow = composition.escrow.unwrap_or(0.0);
  let net_worth = liquid + assets + escrow;

  let identity = Column::with_children(vec![
    text(pilot.name.clone())
      .font(typography::body::MEDIUM)
      .size(20.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(pilot.corp.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT);

  let stats = Row::with_children(vec![
    hero_stat("Liquid \u{00b7} master wallet", liquid, color::text::PRIMARY),
    hero_stat("Assets \u{00b7} est.", assets, color::text::secondary()),
    hero_stat("Net worth", net_worth, color::text::PRIMARY),
  ])
  .spacing(spacing::SPACE_6);

  container(
    Row::with_children(vec![
      Avatar::new(
        pilot.id,
        pilot.name.clone(),
        Length::Fixed(52.0),
        52.0,
        pilot.portrait.path(),
      )
      .radius(radius::SUBTLE)
      .view(),
      identity.into(),
      Space::new().width(Length::Fill).into(),
      stats.into(),
    ])
    .spacing(20.0)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 22.0,
    right: 26.0,
    bottom: 22.0,
    left: 26.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 12.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn hero_stat<'a>(label: &'a str, value: f64, value_color: Color) -> Element<'a, Message> {
  Column::with_children(vec![
    eyebrow_text(label, None).into(),
    isk_amount(value, 18.0, value_color),
  ])
  .spacing(spacing::UNIT + 1.0)
  .into()
}

fn section_card<'a>(section: Section) -> Element<'a, Message> {
  let max = section.rows.iter().map(|row| row.balance).fold(0.0_f64, f64::max);

  let Section {
    caption,
    rows,
    subtotal,
    swatch,
    title,
  } = section;

  let mut children: Vec<Element<'a, Message>> = vec![section_head(title, caption, subtotal, swatch, rows.len())];
  let last = rows.len().saturating_sub(1);
  for (index, row) in rows.into_iter().enumerate() {
    children.push(wallet_row(row, max, index == last));
  }

  container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 12.0.into(),
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
}

fn section_head<'a>(
  title: String,
  caption: Option<String>,
  subtotal: f64,
  swatch: SectionSwatch,
  count: usize,
) -> Element<'a, Message> {
  let title_row = Row::with_children(vec![
    text(title)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(format!("{count} {}", plural(count, "WALLET", "WALLETS")))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let mut head_text: Vec<Element<'a, Message>> = vec![title_row.into()];
  if let Some(caption) = caption {
    head_text.push(
      text(caption.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  let subtotal = Column::with_children(vec![
    eyebrow_text("Subtotal", None).into(),
    isk_amount(subtotal, 16.0, color::accent::PLASMA),
  ])
  .spacing(spacing::UNIT)
  .align_x(Horizontal::Right);

  container(
    Row::with_children(vec![
      section_swatch(swatch, 34.0),
      Column::with_children(head_text)
        .spacing(spacing::UNIT)
        .width(Length::Fill)
        .into(),
      subtotal.into(),
    ])
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 16.0,
    right: 24.0,
    bottom: 14.0,
    left: 24.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: Radius {
        top_left: radius::CARD,
        top_right: radius::CARD,
        bottom_right: 0.0,
        bottom_left: 0.0,
      },
    },
    ..container::Style::default()
  })
  .into()
}

fn section_swatch<'a>(swatch: SectionSwatch, size: f32) -> Element<'a, Message> {
  match swatch {
    SectionSwatch::Corp {
      id,
      name,
      path,
    } => Avatar::new(id, name, Length::Fixed(size), size, path)
      .radius(radius::SUBTLE)
      .view(),
    SectionSwatch::Personal => container(
      Icon::characters()
        .size(size * 0.5)
        .color(color::accent::PLASMA)
        .render(),
    )
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, 0.30),
        width: 1.0,
        radius: 7.0.into(),
      },
      ..container::Style::default()
    })
    .into(),
  }
}

fn wallet_row<'a>(row: RowData, max: f64, last: bool) -> Element<'a, Message> {
  let lead: Element<'a, Message> = match row.swatch {
    Some(swatch) => Avatar::new(
      swatch.id,
      swatch.name,
      Length::Fixed(SWATCH_SIZE),
      SWATCH_SIZE,
      swatch.path,
    )
    .radius(radius::SUBTLE)
    .view(),
    None => Space::new().width(Length::Fixed(SWATCH_SIZE)).into(),
  };

  let mut name_block: Vec<Element<'a, Message>> = vec![
    text(row.name)
      .font(typography::body::MEDIUM)
      .size(if row.indent {
        typography::size::SM
      } else {
        typography::size::MD
      })
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if let Some(sub) = row.sub {
    name_block.push(
      text(sub.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  let share = if max > 0.0 { (row.balance / max) as f32 } else { 0.0 };

  let left_pad = if row.indent { 20.0 } else { 24.0 };
  container(
    Row::with_children(vec![
      lead,
      Column::with_children(name_block)
        .spacing(spacing::UNIT)
        .width(Length::Fill)
        .into(),
      share_bar(share),
      container(isk_amount(
        row.balance,
        if row.indent { 14.0 } else { 15.0 },
        color::text::PRIMARY,
      ))
      .width(Length::Fixed(ISK_COLUMN_WIDTH))
      .align_x(Horizontal::Right)
      .into(),
    ])
    .spacing(16.0)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: if row.indent { 9.0 } else { 12.0 },
    right: 24.0,
    bottom: if row.indent { 9.0 } else { 12.0 },
    left: left_pad,
  })
  .style(move |_| container::Style {
    border: Border {
      color: if last { Color::TRANSPARENT } else { color::rule() },
      width: if last { 0.0 } else { 1.0 },
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn share_bar<'a>(share: f32) -> Element<'a, Message> {
  let filled = (share.clamp(0.0, 1.0) * 1000.0).max(20.0) as u16;
  let empty = 1000_u16.saturating_sub(filled);

  container(
    Row::with_children(vec![
      bar_segment(filled, color::accent::PLASMA),
      bar_segment(empty, Color::TRANSPARENT),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fixed(SHARE_BAR_WIDTH))
  .height(Length::Fixed(4.0))
  .style(|_| container::Style {
    background: Some(Background::Color(color::rule())),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .clip(true)
  .into()
}

fn bar_segment<'a>(portion: u16, fill: Color) -> Element<'a, Message> {
  if portion == 0 {
    return Space::new().width(Length::FillPortion(0)).into();
  }
  container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(Length::FillPortion(portion))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    })
    .into()
}

fn isk_amount<'a>(value: f64, size: f32, value_color: Color) -> Element<'a, Message> {
  Row::with_children(vec![
    text(fmt_isk_full(value))
      .font(typography::mono::MEDIUM)
      .size(size)
      .style(typography::colored(value_color))
      .into(),
    text(" ISK")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Bottom)
  .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("No wallets to show yet \u{2014} sync populates balances.")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_6)
  .center_x(Length::Fill)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn division(number: i64, name: Option<&str>) -> CorpDivision {
    CorpDivision {
      balance: None,
      division: number,
      name: name.map(str::to_owned),
    }
  }

  mod division_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_the_synced_name() {
      assert_eq!(super::division_label(&division(1, Some("Trading"))), "Trading");
    }

    #[test]
    fn it_names_the_first_three_divisions() {
      assert_eq!(super::division_label(&division(1, None)), "Master Wallet");
      assert_eq!(super::division_label(&division(2, None)), "2nd Wallet");
      assert_eq!(super::division_label(&division(3, None)), "3rd Wallet");
    }

    #[test]
    fn it_falls_back_to_an_ordinal_for_later_divisions() {
      assert_eq!(super::division_label(&division(7, None)), "7th Wallet");
    }
  }
}
