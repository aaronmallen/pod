use iced::{
  Background, Border, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, Space, container, image, scrollable, text},
};

use super::{Message, fmt_isk, tabs::killlog::relative_time};
use crate::{
  clients::eve_image::Size,
  store::{
    Database, images,
    killmail_slot::SlotGroup,
    model::CharacterKillEntry,
    repo::{character, org, sde},
  },
  ui::{
    components::{backdrop, clip::clip_layer, eyebrow::eyebrow_text, icon_tile::icon_tile},
    style::{color, radius, spacing, typography},
  },
};

const ATTACKER_ICON_BOX: f32 = 30.0;
const ITEM_ICON_BOX: f32 = 22.0;
const ITEM_ICON_SIZE: Size = Size::S64;
const MODAL_CONTENT_MAX_HEIGHT: f32 = 560.0;
const MODAL_MAX_WIDTH: f32 = 880.0;
const PORTRAIT_BOX: f32 = 52.0;
const SHIP_ICON_BOX: f32 = 46.0;

#[derive(Clone, Debug)]
pub struct AttackerView {
  pub corp_name: String,
  pub damage_share: f64,
  pub final_blow: bool,
  pub is_self: bool,
  pub name: String,
  pub ship_icon: images::IconResolution,
  pub ship_name: String,
}

#[derive(Clone, Debug)]
pub struct EntityView {
  pub logo: images::ImageState,
  pub name: String,
}

#[derive(Clone, Debug)]
pub struct ItemView {
  pub dropped: bool,
  pub icon: images::IconResolution,
  pub name: String,
  pub quantity: i64,
  pub value_isk: f64,
}

#[derive(Clone, Debug)]
pub struct KillmailDetail {
  pub attackers: Vec<AttackerView>,
  pub damage_taken: i64,
  /// Derived as `value_isk - value_destroyed_isk`, floored at 0 (the store persists only the destroyed basis and the
  /// display total; this field is never stored directly).
  pub dropped_isk: f64,
  pub is_kill: bool,
  pub kill_time: String,
  pub killmail_id: i64,
  pub ship_icon: images::IconResolution,
  pub ship_name: String,
  pub slots: Vec<SlotGroupView>,
  pub system_name: Option<String>,
  pub system_security: f64,
  pub value_destroyed_isk: f64,
  pub value_isk: f64,
  pub victim_alliance: Option<EntityView>,
  pub victim_corp: Option<EntityView>,
  pub victim_name: String,
  pub victim_portrait: images::ImageState,
}

impl KillmailDetail {
  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    let mut keys: Vec<(images::ImageKind, i64)> = Vec::new();
    keys.extend(self.victim_portrait.stale_key());
    if let Some(corp) = &self.victim_corp {
      keys.extend(corp.logo.stale_key());
    }
    if let Some(alliance) = &self.victim_alliance {
      keys.extend(alliance.logo.stale_key());
    }
    keys
  }
}

#[derive(Clone, Debug)]
pub struct SlotGroupView {
  pub items: Vec<ItemView>,
  pub label: &'static str,
}

/// `viewing_character_id` only flags the matching attacker row as `is_self`; `character_id` scopes every query.
pub async fn load(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
  viewing_character_id: i64,
) -> Option<KillmailDetail> {
  let rows = character::killmails(db, character_id).await.ok()?;
  let row = rows.into_iter().find(|row| row.killmail_id() == killmail_id)?;

  let ship_name = type_name(db, row.ship_type_id()).await;
  let ship_icon = images::default_store().resolve_type_icon(row.ship_type_id(), None, ITEM_ICON_SIZE);

  let (system_name, system_security) = match sde::get_solar_system(db, row.system_id()).await.ok().flatten() {
    Some(system) => (Some(system.name().clone()), system.security_status()),
    None => (None, 0.0),
  };

  let victim_name = victim_name(db, row.victim_id()).await;
  let victim_portrait = match row.victim_id() {
    Some(id) => images::resolve(&images::default_store(), images::ImageKind::CharacterPortrait, id),
    None => images::ImageState::Stale {
      id: 0,
      kind: images::ImageKind::CharacterPortrait,
    },
  };
  let victim_corp = corporation_view(db, row.victim_corp_id()).await;
  let victim_alliance = alliance_view(db, row.victim_alliance_id()).await;

  let slots = load_slots(db, character_id, killmail_id).await;
  let attackers = load_attackers(db, character_id, killmail_id, viewing_character_id).await;

  Some(KillmailDetail {
    attackers,
    damage_taken: row.victim_damage_taken(),
    dropped_isk: dropped_isk(&row),
    is_kill: row.is_kill(),
    kill_time: row.kill_time().clone(),
    killmail_id,
    ship_icon,
    ship_name,
    slots,
    system_name,
    system_security,
    value_destroyed_isk: row.value_destroyed_isk(),
    value_isk: row.value_isk(),
    victim_alliance,
    victim_corp,
    victim_name,
    victim_portrait,
  })
}

fn dropped_isk(row: &CharacterKillEntry) -> f64 {
  (row.value_isk() - row.value_destroyed_isk()).max(0.0)
}

async fn load_attackers(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
  viewing_character_id: i64,
) -> Vec<AttackerView> {
  let rows = character::killmail_attackers(db, character_id, killmail_id)
    .await
    .unwrap_or_default();
  let total_damage: f64 = rows.iter().map(|row| row.damage_done() as f64).sum();

  let mut attackers = Vec::with_capacity(rows.len());
  for row in &rows {
    let name = match row.attacker_character_id() {
      Some(id) => character_name(db, id).await,
      None => "Unknown".to_owned(),
    };
    let corp_name = match row.corporation_id() {
      Some(id) => corporation_name(db, id).await,
      None => String::new(),
    };
    let (ship_name, ship_icon) = match row.ship_type_id() {
      Some(type_id) => (
        type_name(db, type_id).await,
        images::default_store().resolve_type_icon(type_id, None, ITEM_ICON_SIZE),
      ),
      None => ("Unknown".to_owned(), images::IconResolution::Missing),
    };
    let damage_share = if total_damage > 0.0 {
      row.damage_done() as f64 / total_damage
    } else {
      0.0
    };

    attackers.push(AttackerView {
      corp_name,
      damage_share,
      final_blow: row.final_blow(),
      is_self: row.attacker_character_id() == Some(viewing_character_id),
      name,
      ship_icon,
      ship_name,
    });
  }

  attackers.sort_by(|a, b| {
    b.final_blow
      .cmp(&a.final_blow)
      .then(b.damage_share.total_cmp(&a.damage_share))
  });
  attackers
}

async fn load_slots(db: &Database, character_id: i64, killmail_id: i64) -> Vec<SlotGroupView> {
  let rows = character::killmail_items(db, character_id, killmail_id)
    .await
    .unwrap_or_default();

  let mut groups: Vec<SlotGroupView> = Vec::new();
  for &group in SlotGroup::display_order() {
    let mut items = Vec::new();
    for row in rows.iter().filter(|row| SlotGroup::from_flag(row.flag()) == group) {
      // An entry is flagged green (dropped) when any of its stack survived, red otherwise; the displayed count is the
      // whole stack so a partially-looted stack still reads honestly.
      items.push(ItemView {
        dropped: row.quantity_dropped() > 0,
        icon: images::default_store().resolve_type_icon(row.type_id(), None, ITEM_ICON_SIZE),
        name: type_name(db, row.type_id()).await,
        quantity: row.quantity_destroyed() + row.quantity_dropped(),
        value_isk: row.value_isk(),
      });
    }
    if !items.is_empty() {
      groups.push(SlotGroupView {
        items,
        label: group.label(),
      });
    }
  }
  groups
}

async fn alliance_view(db: &Database, alliance_id: Option<i64>) -> Option<EntityView> {
  let id = alliance_id?;
  let name = org::get_alliance(db, id)
    .await
    .ok()
    .flatten()
    .map(|alliance| alliance.name().clone())
    .unwrap_or_else(|| format!("Alliance {id}"));
  Some(EntityView {
    logo: images::resolve(&images::default_store(), images::ImageKind::AllianceLogo, id),
    name,
  })
}

async fn character_name(db: &Database, id: i64) -> String {
  character::get(db, id)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().to_owned())
    .unwrap_or_else(|| format!("Pilot {id}"))
}

async fn corporation_name(db: &Database, id: i64) -> String {
  org::get_corporation(db, id)
    .await
    .ok()
    .flatten()
    .map(|corp| corp.name().to_owned())
    .unwrap_or_else(|| format!("Corp {id}"))
}

async fn corporation_view(db: &Database, corp_id: Option<i64>) -> Option<EntityView> {
  let id = corp_id?;
  Some(EntityView {
    logo: images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, id),
    name: corporation_name(db, id).await,
  })
}

async fn type_name(db: &Database, type_id: i64) -> String {
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
    .unwrap_or_else(|| format!("Type {type_id}"))
}

async fn victim_name(db: &Database, victim_id: Option<i64>) -> String {
  match victim_id {
    Some(id) => character_name(db, id).await,
    None => "Unknown".to_owned(),
  }
}

pub fn overlay<'a>(base: Element<'a, Message>, detail: &'a KillmailDetail) -> Element<'a, Message> {
  iced::widget::Stack::with_children(vec![
    base,
    backdrop::backdrop(Message::CloseKillmailDetail),
    container(modal(detail))
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(spacing::SPACE_6)
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center)
      .into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn modal(detail: &KillmailDetail) -> Element<'_, Message> {
  let body = container(
    scrollable(
      Column::with_children(vec![
        Row::with_children(vec![victim_card(detail), value_card(detail)])
          .spacing(spacing::SPACE_3_5)
          .width(Length::Fill)
          .into(),
        Row::with_children(vec![fitting_panel(detail), attacker_panel(detail)])
          .spacing(spacing::SPACE_3_5)
          .align_y(Vertical::Top)
          .width(Length::Fill)
          .into(),
      ])
      .spacing(spacing::SPACE_3_5 + spacing::SPACE_2)
      .padding(spacing::SPACE_3_5 + spacing::UNIT)
      .width(Length::Fill),
    )
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Shrink),
  )
  .max_height(MODAL_CONTENT_MAX_HEIGHT);

  container(
    Column::with_children(vec![header(detail), body.into()])
      .width(Length::Fill)
      .height(Length::Shrink),
  )
  .max_width(MODAL_MAX_WIDTH)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.16),
      width: 1.0,
      radius: radius::PANEL.into(),
    },
    ..container::Style::default()
  })
  .clip(true)
  .into()
}

fn header(detail: &KillmailDetail) -> Element<'_, Message> {
  let accent = side_accent(detail.is_kill);

  let title = Row::with_children(vec![
    text(detail.ship_name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    kind_badge(detail.is_kill),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let meta = Row::with_children(vec![
    system_label(detail),
    dot(),
    meta_text(relative_time(&detail.kill_time), color::text::secondary()),
    dot(),
    meta_text(format!("#{}", detail.killmail_id), color::text::tertiary()),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let info = Column::with_children(vec![title.into(), meta.into()]).spacing(spacing::UNIT);

  let content = Row::with_children(vec![
    type_icon(&detail.ship_icon, SHIP_ICON_BOX),
    info.into(),
    Space::new().width(Length::Fill).into(),
    close_button(),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let bar = container(Space::new().width(Length::Fixed(4.0)).height(Length::Fill))
    .width(Length::Fixed(4.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(accent)),
      ..container::Style::default()
    });

  let row = Row::with_children(vec![
    bar.into(),
    container(content)
      .padding(Padding {
        top: spacing::SPACE_3_5,
        right: spacing::SPACE_3_5,
        bottom: spacing::SPACE_3_5,
        left: spacing::SPACE_3_5,
      })
      .width(Length::Fill)
      .into(),
  ])
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: 1.0,
        radius: Radius {
          top_left: radius::PANEL,
          top_right: radius::PANEL,
          bottom_right: 0.0,
          bottom_left: 0.0,
        },
      },
      ..container::Style::default()
    })
    .into()
}

fn victim_card(detail: &KillmailDetail) -> Element<'_, Message> {
  let label = if detail.is_kill { "Victim" } else { "Pilot lost" };

  let portrait = portrait_tile(&detail.victim_portrait, &detail.victim_name);

  let mut name_lines: Vec<Element<'_, Message>> = vec![
    text(detail.victim_name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(corp) = &detail.victim_corp {
    name_lines.push(subtitle(&corp.name, color::text::secondary()));
  }
  if let Some(alliance) = &detail.victim_alliance {
    name_lines.push(subtitle(&alliance.name, color::text::tertiary()));
  }

  let identity = Row::with_children(vec![
    portrait,
    Column::with_children(name_lines)
      .spacing(2.0)
      .width(Length::Fill)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let stats = Row::with_children(vec![
    mini("Ship", detail.ship_name.clone()),
    mini("Damage taken", format!("{} HP", detail.damage_taken)),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  panel(
    label,
    None,
    Column::with_children(vec![identity.into(), stats.into()])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .into(),
  )
}

fn value_card(detail: &KillmailDetail) -> Element<'_, Message> {
  let total = text(format!("{} ISK", fmt_isk(Some(detail.value_isk))))
    .font(typography::mono::MEDIUM)
    .size(24.0)
    .style(move |_| text::Style {
      color: Some(side_accent(detail.is_kill)),
    });

  let drop_pct = if detail.value_isk > 0.0 {
    (detail.dropped_isk / detail.value_isk * 100.0).clamp(0.0, 100.0)
  } else {
    0.0
  };
  let bar = Row::with_children(vec![
    container(Space::new())
      .width(Length::FillPortion(((100.0 - drop_pct) * 10.0) as u16))
      .height(Length::Fixed(6.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::status::DANGER)),
        ..container::Style::default()
      })
      .into(),
    container(Space::new())
      .width(Length::FillPortion((drop_pct * 10.0) as u16))
      .height(Length::Fixed(6.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::status::ONLINE)),
        ..container::Style::default()
      })
      .into(),
  ])
  .width(Length::Fill);

  let legend = Row::with_children(vec![
    legend_cell(color::status::DANGER, "Destroyed", detail.value_destroyed_isk, false),
    legend_cell(color::status::ONLINE, "Dropped", detail.dropped_isk, true),
  ])
  .width(Length::Fill);

  panel(
    "Value",
    None,
    Column::with_children(vec![total.into(), bar.into(), legend.into()])
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fill)
      .into(),
  )
}

fn fitting_panel(detail: &KillmailDetail) -> Element<'_, Message> {
  let count: usize = detail.slots.iter().map(|group| group.items.len()).sum();

  let mut sections: Vec<Element<'_, Message>> = Vec::new();
  for (index, group) in detail.slots.iter().enumerate() {
    sections.push(slot_header(group.label, index > 0));
    for item in &group.items {
      sections.push(item_row(item));
    }
  }
  if sections.is_empty() {
    sections.push(
      container(subtitle("No items recorded", color::text::secondary()))
        .padding(spacing::SPACE_3)
        .into(),
    );
  }

  panel_unpadded(
    "Fitting & cargo",
    Some(format!("{count} items")),
    Column::with_children(sections).width(Length::Fill).into(),
  )
}

fn attacker_panel(detail: &KillmailDetail) -> Element<'_, Message> {
  let count = detail.attackers.len();
  let suffix = if count == 1 { "pilot" } else { "pilots" };

  let rows: Vec<Element<'_, Message>> = detail
    .attackers
    .iter()
    .enumerate()
    .map(|(index, attacker)| attacker_row(attacker, index == count - 1))
    .collect();

  let body: Element<'_, Message> = if rows.is_empty() {
    container(subtitle("No attackers recorded", color::text::secondary()))
      .padding(spacing::SPACE_3)
      .into()
  } else {
    Column::with_children(rows).width(Length::Fill).into()
  };

  panel_unpadded("Involved parties", Some(format!("{count} {suffix}")), body)
}

fn item_row(item: &ItemView) -> Element<'_, Message> {
  let dot = container(Space::new())
    .width(Length::Fixed(7.0))
    .height(Length::Fixed(7.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(if item.dropped {
        color::status::ONLINE
      } else {
        color::status::DANGER
      })),
      border: Border {
        radius: 2.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let mut name_line: Vec<Element<'_, Message>> = vec![
    type_icon(&item.icon, ITEM_ICON_BOX),
    text(item.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if item.quantity > 1 {
    name_line.push(
      text(format!("\u{00d7}{}", item.quantity))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let row = Row::with_children(vec![
    container(dot)
      .width(Length::Fixed(14.0))
      .align_x(Horizontal::Center)
      .into(),
    Row::with_children(name_line)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .width(Length::Fill)
      .into(),
    text(fmt_isk(Some(item.value_isk)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2 - 1.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2 - 1.0,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn attacker_row(attacker: &AttackerView, last: bool) -> Element<'_, Message> {
  let name_color = if attacker.is_self {
    color::accent::PLASMA
  } else {
    color::text::PRIMARY
  };
  let name_font = if attacker.is_self {
    typography::body::MEDIUM
  } else {
    typography::body::REGULAR
  };

  let mut name_line: Vec<Element<'_, Message>> = vec![
    text(attacker.name.clone())
      .font(name_font)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(name_color),
      })
      .into(),
  ];
  if attacker.final_blow {
    name_line.push(final_blow_chip());
  }

  let subtitle_text = if attacker.corp_name.is_empty() {
    attacker.ship_name.clone()
  } else {
    format!("{} \u{00b7} {}", attacker.ship_name, attacker.corp_name)
  };

  let info = Column::with_children(vec![
    Row::with_children(name_line)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    subtitle(&subtitle_text, color::text::secondary()),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  let share = share_cell(attacker);

  let row = Row::with_children(vec![
    type_icon(&attacker.ship_icon, ATTACKER_ICON_BOX),
    info.into(),
    share,
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };
  let highlight = attacker.final_blow;
  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      background: highlight.then(|| Background::Color(color::with_alpha(color::status::WARNING, 0.06))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: border_bottom,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn share_cell(attacker: &AttackerView) -> Element<'_, Message> {
  let pct = (attacker.damage_share * 100.0).round() as i64;
  let fill_color = if attacker.final_blow {
    color::status::WARNING
  } else {
    color::accent::PLASMA
  };
  let fill = (attacker.damage_share * 100.0).clamp(4.0, 100.0);

  let bar = Row::with_children(vec![
    Space::new().width(Length::FillPortion((100.0 - fill) as u16)).into(),
    container(Space::new())
      .width(Length::FillPortion(fill.max(1.0) as u16))
      .height(Length::Fixed(3.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(fill_color)),
        border: Border {
          radius: 2.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
  ])
  .width(Length::Fill);

  container(
    Column::with_children(vec![
      text(format!("{pct}%"))
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .align_x(Horizontal::Right)
        .into(),
      bar.into(),
    ])
    .spacing(spacing::UNIT)
    .width(Length::Fixed(52.0)),
  )
  .into()
}

fn slot_header(label: &str, top_rule: bool) -> Element<'_, Message> {
  container(eyebrow_text(label, Some(color::text::tertiary())))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2 - 2.0,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: if top_rule { 1.0 } else { 0.0 },
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn legend_cell<'a>(swatch: iced::Color, label: &str, value: f64, right: bool) -> Element<'a, Message> {
  let head = Row::with_children(vec![
    container(Space::new())
      .width(Length::Fixed(7.0))
      .height(Length::Fixed(7.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(swatch)),
        border: Border {
          radius: 2.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    eyebrow_text(label, Some(color::text::secondary())).into(),
  ])
  .spacing(spacing::SPACE_2 - 2.0)
  .align_y(Vertical::Center);

  let value = text(format!("{} ISK", fmt_isk(Some(value))))
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let align = if right { Horizontal::Right } else { Horizontal::Left };
  container(
    Column::with_children(vec![
      container(head).width(Length::Fill).align_x(align).into(),
      container(value).width(Length::Fill).align_x(align).into(),
    ])
    .spacing(spacing::UNIT),
  )
  .width(Length::Fill)
  .into()
}

fn mini<'a>(label: &str, value: String) -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      eyebrow_text(label, Some(color::text::tertiary())).into(),
      text(value)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(spacing::UNIT),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2_5 - 1.0,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2_5 - 1.0,
    left: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.06),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn panel<'a>(label: &str, right: Option<String>, content: Element<'a, Message>) -> Element<'a, Message> {
  panel_unpadded(label, right, container(content).padding(spacing::SPACE_3_5).into())
}

fn panel_unpadded<'a>(label: &str, right: Option<String>, content: Element<'a, Message>) -> Element<'a, Message> {
  let mut head: Vec<Element<'a, Message>> = vec![
    eyebrow_text(label, Some(color::text::secondary()))
      .width(Length::Fill)
      .into(),
  ];
  if let Some(right) = right {
    head.push(
      text(right)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let header = container(
    Row::with_children(head)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  })
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
  });

  container(Column::with_children(vec![header.into(), content]).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.08),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
}

fn close_button() -> Element<'static, Message> {
  iced::widget::button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(spacing::SPACE_2 - 1.0)
  .on_press(Message::CloseKillmailDetail)
  .style(|_, _| iced::widget::button::Style {
    background: None,
    text_color: color::text::secondary(),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.08),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..iced::widget::button::Style::default()
  })
  .into()
}

fn dot() -> Element<'static, Message> {
  container(Space::new())
    .width(Length::Fixed(3.0))
    .height(Length::Fixed(3.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::text::tertiary())),
      border: Border {
        radius: 2.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn final_blow_chip() -> Element<'static, Message> {
  container(
    text("FINAL BLOW")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS - 1.0)
      .style(|_| text::Style {
        color: Some(color::status::WARNING),
      }),
  )
  .padding(Padding {
    top: 1.0,
    right: spacing::UNIT + 1.0,
    bottom: 1.0,
    left: spacing::UNIT + 1.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.4),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn kind_badge(is_kill: bool) -> Element<'static, Message> {
  let (label, tint) = if is_kill {
    ("KILL", color::status::ONLINE)
  } else {
    ("LOSS", color::status::DANGER)
  };
  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(tint),
      }),
  )
  .padding(Padding {
    top: 3.0,
    right: spacing::SPACE_2,
    bottom: 3.0,
    left: spacing::SPACE_2,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.12))),
    border: Border {
      color: color::with_alpha(tint, 0.3),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn meta_text(value: String, tint: iced::Color) -> Element<'static, Message> {
  text(value)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(move |_| text::Style {
      color: Some(tint),
    })
    .into()
}

fn portrait_tile<'a>(portrait: &images::ImageState, name: &str) -> Element<'a, Message> {
  match portrait.path() {
    Some(path) => container(clip_layer(
      image(image::Handle::from_path(path))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
      Length::Fill,
      Length::Fill,
    ))
    .width(Length::Fixed(PORTRAIT_BOX))
    .height(Length::Fixed(PORTRAIT_BOX))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into(),
    None => crate::ui::components::avatar::avatar(0, name, Length::Fixed(PORTRAIT_BOX), PORTRAIT_BOX, None),
  }
}

fn side_accent(is_kill: bool) -> iced::Color {
  if is_kill {
    color::status::ONLINE
  } else {
    color::status::DANGER
  }
}

fn subtitle<'a>(value: &str, tint: iced::Color) -> Element<'a, Message> {
  text(value.to_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(move |_| text::Style {
      color: Some(tint),
    })
    .width(Length::Fill)
    .into()
}

fn system_label(detail: &KillmailDetail) -> Element<'_, Message> {
  let Some(name) = detail.system_name.as_ref() else {
    return meta_text("\u{2014}".to_owned(), color::text::secondary());
  };

  let sec = detail.system_security;
  let sec_color = if sec >= 0.5 {
    color::status::ONLINE
  } else if sec > 0.0 {
    color::status::WARNING
  } else {
    color::status::DANGER
  };

  Row::with_children(vec![
    meta_text(name.clone(), color::text::secondary()),
    text(format!("{:.1}", sec.clamp(-1.0, 1.0)))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(sec_color),
      })
      .into(),
  ])
  .spacing(spacing::UNIT + 1.0)
  .align_y(Vertical::Bottom)
  .into()
}

fn type_icon<'a>(icon: &images::IconResolution, box_size: f32) -> Element<'a, Message> {
  match icon {
    images::IconResolution::Found(path) => icon_tile(
      clip_layer(
        image(image::Handle::from_path(path.clone()))
          .width(Length::Fill)
          .height(Length::Fill)
          .content_fit(ContentFit::Cover),
        Length::Fill,
        Length::Fill,
      ),
      box_size,
    ),
    images::IconResolution::Missing => icon_tile(Space::new(), box_size),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn detail() -> KillmailDetail {
    KillmailDetail {
      attackers: vec![
        AttackerView {
          corp_name: "Big Corp".to_owned(),
          damage_share: 0.25,
          final_blow: false,
          is_self: false,
          name: "Gunner".to_owned(),
          ship_icon: images::IconResolution::Missing,
          ship_name: "Hurricane".to_owned(),
        },
        AttackerView {
          corp_name: "Big Corp".to_owned(),
          damage_share: 0.75,
          final_blow: true,
          is_self: true,
          name: "Finisher".to_owned(),
          ship_icon: images::IconResolution::Missing,
          ship_name: "Loki".to_owned(),
        },
      ],
      damage_taken: 42_000,
      dropped_isk: 20_000_000.0,
      is_kill: true,
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id: 100,
      ship_icon: images::IconResolution::Missing,
      ship_name: "Rifter".to_owned(),
      slots: vec![SlotGroupView {
        items: vec![ItemView {
          dropped: true,
          icon: images::IconResolution::Missing,
          name: "Damage Control II".to_owned(),
          quantity: 1,
          value_isk: 1_000_000.0,
        }],
        label: "High power",
      }],
      system_name: Some("Jita".to_owned()),
      system_security: 0.9,
      value_destroyed_isk: 80_000_000.0,
      value_isk: 100_000_000.0,
      victim_alliance: Some(EntityView {
        logo: images::ImageState::Stale {
          id: 99,
          kind: images::ImageKind::AllianceLogo,
        },
        name: "Big Alliance".to_owned(),
      }),
      victim_corp: Some(EntityView {
        logo: images::ImageState::Stale {
          id: 7,
          kind: images::ImageKind::CorporationLogo,
        },
        name: "Big Corp".to_owned(),
      }),
      victim_name: "Target".to_owned(),
      victim_portrait: images::ImageState::Stale {
        id: 3,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  mod overlay {
    use super::*;

    #[test]
    fn it_renders_a_kill_and_a_loss() {
      let kill = detail();
      let base: Element<'_, Message> = Space::new().into();
      let _kill: Element<'_, Message> = overlay(base, &kill);

      let mut loss = detail();
      loss.is_kill = false;
      let base: Element<'_, Message> = Space::new().into();
      let _loss: Element<'_, Message> = overlay(base, &loss);
    }

    #[test]
    fn it_renders_without_corp_alliance_or_items() {
      let mut bare = detail();
      bare.victim_corp = None;
      bare.victim_alliance = None;
      bare.slots = Vec::new();
      bare.attackers = Vec::new();

      let base: Element<'_, Message> = Space::new().into();
      let _el: Element<'_, Message> = overlay(base, &bare);
    }

    #[test]
    fn it_renders_a_heavy_kill_with_many_items() {
      let mut heavy = detail();
      heavy.slots = vec![SlotGroupView {
        items: (0..43)
          .map(|index| ItemView {
            dropped: index % 2 == 0,
            icon: images::IconResolution::Missing,
            name: format!("Module {index}"),
            quantity: 1,
            value_isk: 1_000_000.0,
          })
          .collect(),
        label: "High power",
      }];

      let base: Element<'_, Message> = Space::new().into();
      let _el: Element<'_, Message> = overlay(base, &heavy);
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_stale_portrait_and_logo_keys() {
      let keys = detail().stale_images();

      assert_eq!(keys.len(), 3);
      assert!(keys.contains(&(images::ImageKind::CharacterPortrait, 3)));
      assert!(keys.contains(&(images::ImageKind::CorporationLogo, 7)));
      assert!(keys.contains(&(images::ImageKind::AllianceLogo, 99)));
    }

    #[test]
    fn it_skips_fresh_images() {
      let mut fresh = detail();
      fresh.victim_portrait = images::ImageState::Fresh("/tmp/p.jpg".into());
      fresh.victim_corp = None;
      fresh.victim_alliance = None;

      assert!(fresh.stale_images().is_empty());
    }
  }
}
