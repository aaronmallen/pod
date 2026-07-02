use std::f32::consts::FRAC_PI_4;

use iced::{
  Background, Border, Color, Element, Length, Padding, Radians,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, Space, button, container, scrollable, text},
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
const ISK_COLUMN_WIDTH: f32 = 168.0;
const PERSONAL_SECTION_KEY: &str = "personal";
const SHARE_BAR_WIDTH: f32 = 150.0;
const SHOW_FIRST_CORPS_FLAG: &str = "wallet.show_first_corps";
const SWATCH_SIZE: f32 = 30.0;
const WALLET_PINS_LIST: &str = "wallet.pins";

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
  key: String,
  personal: bool,
  pinnable: bool,
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
  let is_all = matches!(state.active(), Scope::All);
  let show_first_corps = state.ui_flag(SHOW_FIRST_CORPS_FLAG, false);
  let pins = state.ui_list(WALLET_PINS_LIST);

  let mut body: Vec<Element<'a, Message>> = Vec::new();
  if let Scope::Character(id) = state.active()
    && let Some(pilot) = state.roster().iter().find(|pilot| pilot.id == id)
  {
    body.push(pilot_hero(pilot, scope_composition(state)));
  }
  let empty = sections.is_empty();
  for section in sections {
    let pinned = pins.contains(&section.key);
    body.push(section_card(section, pinned));
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

  Column::with_children(vec![toolbar(context, sort, is_all, show_first_corps), scrolled.into()])
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
      for section in &mut sections {
        section.pinnable = true;
      }
      order_sections(
        sections,
        state.ui_flag(SHOW_FIRST_CORPS_FLAG, false),
        state.ui_list(WALLET_PINS_LIST),
      )
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

fn order_sections(sections: Vec<Section>, show_first_corps: bool, pins: &[String]) -> Vec<Section> {
  let mut grouped: Vec<Section> = if show_first_corps {
    let (personal, corps): (Vec<Section>, Vec<Section>) = sections.into_iter().partition(|section| section.personal);
    corps.into_iter().chain(personal).collect()
  } else {
    sections
  };

  let mut ordered: Vec<Section> = Vec::with_capacity(grouped.len());
  for key in pins.iter().rev() {
    if let Some(index) = grouped.iter().position(|section| section.key == *key) {
      ordered.push(grouped.remove(index));
    }
  }
  ordered.extend(grouped);
  ordered
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
    caption: Some(t!("wallet.wallets.personal_caption").into_owned()),
    key: PERSONAL_SECTION_KEY.to_owned(),
    personal: true,
    pinnable: false,
    rows,
    subtotal,
    swatch: SectionSwatch::Personal,
    title: t!("wallet.wallets.personal_title").into_owned(),
  }
}

fn single_pilot_section(pilot: &RosterPilot) -> Section {
  let balance = pilot.liquid.unwrap_or(0.0);
  let caption = if pilot.corp.is_empty() {
    t!("wallet.wallets.personal_wallet").into_owned()
  } else {
    t!("wallet.wallets.personal_wallet_with_corp", corp => pilot.corp).into_owned()
  };

  Section {
    caption: Some(caption),
    key: PERSONAL_SECTION_KEY.to_owned(),
    personal: true,
    pinnable: false,
    rows: vec![RowData {
      balance,
      indent: false,
      name: t!("wallet.wallets.master_wallet").into_owned(),
      sub: Some(t!("wallet.wallets.personal_liquid").into_owned()),
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
    key: section.id.to_string(),
    personal: false,
    pinnable: false,
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
    (Some(name), Some(role)) => t!("wallet.wallets.corp_caption_via_role", name => name, role => role).into_owned(),
    (Some(name), None) => t!("wallet.wallets.corp_caption_via", name => name).into_owned(),
    _ => t!("wallet.wallets.corp_caption").into_owned(),
  }
}

fn division_label(division: &CorpDivision) -> String {
  division.name.clone().unwrap_or_else(|| match division.division {
    1 => t!("wallet.wallets.master_wallet").into_owned(),
    2 => t!("wallet.wallets.division_2nd").into_owned(),
    3 => t!("wallet.wallets.division_3rd").into_owned(),
    other => t!("wallet.wallets.division_nth", n => other).into_owned(),
  })
}

fn sort_rows(rows: &mut [RowData], sort: WalletSort) {
  rows.sort_by(|a, b| match sort {
    WalletSort::Ascending => a.balance.total_cmp(&b.balance),
    WalletSort::Descending => b.balance.total_cmp(&a.balance),
  });
}

fn toolbar<'a>(
  context: String,
  sort: WalletSort,
  show_group_order: bool,
  show_first_corps: bool,
) -> Element<'a, Message> {
  let showing = Row::with_children(vec![
    eyebrow_text(super::i18n::tr_static("wallet.wallets.showing"), None).into(),
    text(context)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let sort_toggle = segmented_frame(vec![
    sort_button(
      super::i18n::tr_static("wallet.wallets.sort_high_low"),
      sort == WalletSort::Descending,
      WalletSort::Descending,
    ),
    segment_divider(),
    sort_button(
      super::i18n::tr_static("wallet.wallets.sort_low_high"),
      sort == WalletSort::Ascending,
      WalletSort::Ascending,
    ),
  ]);

  let sort_group = Row::with_children(vec![
    eyebrow_text(super::i18n::tr_static("wallet.wallets.sort"), None).into(),
    sort_toggle,
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let mut controls: Vec<Element<'a, Message>> = vec![showing.into(), Space::new().width(Length::Fill).into()];
  if show_group_order {
    controls.push(group_order_group(show_first_corps));
  }
  controls.push(sort_group.into());

  container(
    Row::with_children(controls)
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
      t!(
        "wallet.wallets.context_all",
        pilots => pilots,
        pilot_word => plural(
          pilots,
          super::i18n::tr_static("wallet.wallets.pilot_singular"),
          super::i18n::tr_static("wallet.wallets.pilot_plural"),
        ),
        corps => corps,
        wallet_word => plural(
          corps,
          super::i18n::tr_static("wallet.wallets.wallet_singular"),
          super::i18n::tr_static("wallet.wallets.wallet_plural"),
        )
      )
      .into_owned()
    }
    Scope::Character(_) => sections
      .first()
      .map(|section| t!("wallet.wallets.context_personal", name => section.title).into_owned())
      .unwrap_or_else(|| t!("wallet.wallets.personal_wallet").into_owned()),
    Scope::Corporation(_) => sections
      .first()
      .map(|section| t!("wallet.wallets.context_corporation", name => section.title).into_owned())
      .unwrap_or_else(|| t!("wallet.wallets.corp_caption").into_owned()),
  }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
  if count == 1 { singular } else { plural }
}

fn group_order_button<'a>(label: &'a str, active: bool, show_first_corps: bool) -> Element<'a, Message> {
  segment_button(
    label,
    active,
    Padding {
      top: 7.0,
      right: 13.0,
      bottom: 7.0,
      left: 13.0,
    },
    Message::UiFlagSet(SHOW_FIRST_CORPS_FLAG.to_owned(), show_first_corps),
  )
}

fn group_order_group<'a>(show_first_corps: bool) -> Element<'a, Message> {
  let toggle = segmented_frame(vec![
    group_order_button(
      super::i18n::tr_static("wallet.wallets.pilots"),
      !show_first_corps,
      false,
    ),
    segment_divider(),
    group_order_button(super::i18n::tr_static("wallet.wallets.corps"), show_first_corps, true),
  ]);

  Row::with_children(vec![
    eyebrow_text(super::i18n::tr_static("wallet.wallets.show_first"), None).into(),
    toggle,
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn segment_divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fixed(28.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn segmented_frame<'a>(segments: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  container(Row::with_children(segments).align_y(Vertical::Center))
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 7.0.into(),
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
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

fn divider_count(rows: usize) -> usize {
  rows.saturating_sub(1)
}

fn row_divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
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
    hero_stat(
      super::i18n::tr_static("wallet.wallets.hero_liquid"),
      liquid,
      color::text::PRIMARY,
    ),
    hero_stat(
      super::i18n::tr_static("wallet.wallets.hero_assets"),
      assets,
      color::text::secondary(),
    ),
    hero_stat(
      super::i18n::tr_static("wallet.wallets.hero_net_worth"),
      net_worth,
      color::text::PRIMARY,
    ),
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

fn section_card<'a>(section: Section, pinned: bool) -> Element<'a, Message> {
  let Section {
    caption,
    key,
    personal: _,
    pinnable,
    rows,
    subtotal,
    swatch,
    title,
  } = section;

  let pin = pinnable.then_some((key, pinned));
  let mut children: Vec<Element<'a, Message>> = vec![section_head(title, caption, subtotal, swatch, rows.len(), pin)];
  let last = divider_count(rows.len());
  for (index, row) in rows.into_iter().enumerate() {
    children.push(wallet_row(row, subtotal));
    if index < last {
      children.push(row_divider());
    }
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

fn pin_button<'a>(key: String, pinned: bool) -> Element<'a, Message> {
  let tint = if pinned {
    color::accent()
  } else {
    color::text::secondary()
  };
  let rotation = if pinned { 0.0 } else { FRAC_PI_4 };

  let mut content: Vec<Element<'a, Message>> =
    vec![Icon::tack().size(15.0).color(tint).rotation(Radians(rotation)).render()];
  if pinned {
    content.push(
      text(t!("wallet.wallets.pinned"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(typography::colored(color::accent()))
        .into(),
    );
  }

  let padding = if pinned {
    Padding {
      top: 5.0,
      right: 10.0,
      bottom: 5.0,
      left: 8.0,
    }
  } else {
    Padding::from(6.0)
  };

  button(
    Row::with_children(content)
      .spacing(spacing::UNIT + 2.0)
      .align_y(Vertical::Center),
  )
  .padding(padding)
  .on_press(Message::UiListItemToggled(WALLET_PINS_LIST.to_owned(), key))
  .style(move |_, status| pin_button_style(pinned, status))
  .into()
}

fn pin_button_style(pinned: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let (background, border_color) = if pinned {
    (
      Some(color::with_alpha(color::accent(), 0.12)),
      color::with_alpha(color::accent(), 0.35),
    )
  } else if hovered {
    (
      Some(color::with_alpha(color::text::PRIMARY, 0.05)),
      color::rule_strong(),
    )
  } else {
    (None, color::rule())
  };

  button::Style {
    background: background.map(Background::Color),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: 7.0.into(),
    },
    text_color: color::accent(),
    ..button::Style::default()
  }
}

fn section_head<'a>(
  title: String,
  caption: Option<String>,
  subtotal: f64,
  swatch: SectionSwatch,
  count: usize,
  pin: Option<(String, bool)>,
) -> Element<'a, Message> {
  let title_row = Row::with_children(vec![
    text(title)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!(
      "wallet.wallets.wallet_count",
      count => count,
      word => plural(
        count,
        super::i18n::tr_static("wallet.wallets.wallet_unit_singular"),
        super::i18n::tr_static("wallet.wallets.wallet_unit_plural"),
      )
    ))
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
    eyebrow_text(super::i18n::tr_static("wallet.wallets.subtotal"), None).into(),
    isk_amount(subtotal, 16.0, color::accent()),
  ])
  .spacing(spacing::UNIT)
  .align_x(Horizontal::Right);

  let mut head_row: Vec<Element<'a, Message>> = vec![
    section_swatch(swatch, 34.0),
    Column::with_children(head_text)
      .spacing(spacing::UNIT)
      .width(Length::Fill)
      .into(),
    subtotal.into(),
  ];
  if let Some((key, pinned)) = pin {
    head_row.push(pin_button(key, pinned));
  }

  container(
    Row::with_children(head_row)
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
    SectionSwatch::Personal => container(Icon::characters().size(size * 0.5).color(color::accent()).render())
      .width(Length::Fixed(size))
      .height(Length::Fixed(size))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center)
      .style(|_| container::Style {
        background: Some(Background::Color(color::with_alpha(color::accent(), 0.12))),
        border: Border {
          color: color::with_alpha(color::accent(), 0.30),
          width: 1.0,
          radius: 7.0.into(),
        },
        ..container::Style::default()
      })
      .into(),
  }
}

fn wallet_row<'a>(row: RowData, section_subtotal: f64) -> Element<'a, Message> {
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

  let share = row_share(row.balance, section_subtotal);

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
  .into()
}

fn row_share(balance: f64, section_subtotal: f64) -> f32 {
  if section_subtotal > 0.0 {
    (balance / section_subtotal) as f32
  } else {
    0.0
  }
}

fn share_bar<'a>(share: f32) -> Element<'a, Message> {
  let filled = (share.clamp(0.0, 1.0) * 1000.0).max(20.0) as u16;
  let empty = 1000_u16.saturating_sub(filled);

  container(
    Row::with_children(vec![
      bar_segment(filled, color::accent()),
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
    text(t!("wallet.wallets.isk_suffix"))
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
    text(t!("wallet.wallets.empty"))
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

  fn section(key: &str, personal: bool) -> Section {
    Section {
      caption: None,
      key: key.to_owned(),
      personal,
      pinnable: true,
      rows: Vec::new(),
      subtotal: 0.0,
      swatch: SectionSwatch::Personal,
      title: key.to_owned(),
    }
  }

  fn keys(sections: &[Section]) -> Vec<String> {
    sections.iter().map(|section| section.key.clone()).collect()
  }

  mod order_sections {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_pilots_first_by_default() {
      let sections = vec![section("personal", true), section("1", false), section("2", false)];

      let ordered = super::order_sections(sections, false, &[]);

      assert_eq!(keys(&ordered), ["personal", "1", "2"]);
    }

    #[test]
    fn it_floats_the_corp_block_above_pilots_when_show_first_corps() {
      let sections = vec![section("personal", true), section("1", false), section("2", false)];

      let ordered = super::order_sections(sections, true, &[]);

      assert_eq!(keys(&ordered), ["1", "2", "personal"]);
    }

    #[test]
    fn it_floats_pinned_sections_above_the_group_order_newest_first() {
      let sections = vec![section("personal", true), section("1", false), section("2", false)];
      let pins = ["personal".to_owned(), "2".to_owned()];

      let ordered = super::order_sections(sections, false, &pins);

      assert_eq!(keys(&ordered), ["2", "personal", "1"]);
    }

    #[test]
    fn it_ranks_pins_over_group_order() {
      let sections = vec![section("personal", true), section("1", false), section("2", false)];
      let pins = ["1".to_owned()];

      let ordered = super::order_sections(sections, true, &pins);

      assert_eq!(keys(&ordered), ["1", "2", "personal"]);
    }

    #[test]
    fn it_ignores_pins_for_absent_sections() {
      let sections = vec![section("personal", true), section("1", false)];
      let pins = ["9".to_owned(), "1".to_owned()];

      let ordered = super::order_sections(sections, false, &pins);

      assert_eq!(keys(&ordered), ["1", "personal"]);
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

  mod divider_count {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_draws_one_fewer_divider_than_rows() {
      assert_eq!(super::divider_count(3), 2);
      assert_eq!(super::divider_count(5), 4);
    }

    #[test]
    fn it_draws_no_divider_for_a_single_row() {
      assert_eq!(super::divider_count(1), 0);
    }

    #[test]
    fn it_draws_no_divider_for_an_empty_section() {
      assert_eq!(super::divider_count(0), 0);
    }
  }

  mod row_share {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_sums_to_one_across_a_multi_row_section() {
      let balances = [40.0, 35.0, 25.0];
      let subtotal: f64 = balances.iter().sum();

      let total: f32 = balances
        .iter()
        .map(|balance| super::row_share(*balance, subtotal))
        .sum();

      assert!((total - 1.0).abs() < 1e-5, "shares summed to {total}");
    }

    #[test]
    fn it_yields_equal_shares_for_equal_balances() {
      let subtotal = 300.0;

      let first = super::row_share(100.0, subtotal);
      let second = super::row_share(100.0, subtotal);
      let third = super::row_share(100.0, subtotal);

      assert_eq!(first, second);
      assert_eq!(second, third);
    }

    #[test]
    fn it_fills_a_single_row_section() {
      assert_eq!(super::row_share(500.0, 500.0), 1.0);
    }

    #[test]
    fn it_yields_no_fill_for_a_zero_subtotal() {
      assert_eq!(super::row_share(0.0, 0.0), 0.0);
    }

    #[test]
    fn it_yields_no_fill_for_a_negative_subtotal() {
      assert_eq!(super::row_share(100.0, -50.0), 0.0);
    }
  }
}
