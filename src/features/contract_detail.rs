use iced::{
  Background, Border, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, Space, container, image, scrollable, text},
};

use crate::{
  clients::eve_image::Size,
  store::{
    Database, images,
    repo::{character, finance, org, sde},
  },
  ui::{
    components::{backdrop, clip::clip_layer, eyebrow::eyebrow_text, icon_tile::icon_tile},
    style::{color, radius, spacing, typography},
  },
};

const FIELD_GAP: f32 = 1.0;
const ITEM_ICON_BOX: f32 = 22.0;
const ITEM_ICON_SIZE: Size = Size::S64;
const KIND_TINT: iced::Color = iced::Color {
  r: 0.482,
  g: 0.545,
  b: 0.851,
  a: 1.0,
};
const MODAL_CONTENT_MAX_HEIGHT: f32 = 560.0;
const MODAL_MAX_WIDTH: f32 = 840.0;
const PORTRAIT_BOX: f32 = 34.0;
const ROUTE_RULE: f32 = 34.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractKind {
  Auction,
  Courier,
  ItemExchange,
}

impl ContractKind {
  pub fn from_status(value: &str) -> Self {
    match value {
      "auction" => ContractKind::Auction,
      "courier" => ContractKind::Courier,
      _ => ContractKind::ItemExchange,
    }
  }

  fn label(self) -> &'static str {
    match self {
      ContractKind::Auction => "Auction",
      ContractKind::Courier => "Courier",
      ContractKind::ItemExchange => "Item Exchange",
    }
  }
}

#[derive(Clone, Debug)]
pub struct BidView {
  pub amount: f64,
  pub bidder: String,
  pub when: String,
}

#[derive(Clone, Debug)]
pub struct ContractDetail {
  pub acceptor: Option<PartyView>,
  pub availability: String,
  pub bids: Vec<BidView>,
  pub buyout: Option<f64>,
  pub collateral: Option<f64>,
  pub contract_id: i64,
  pub days_to_complete: Option<i64>,
  pub expiry: ExpiryView,
  pub headline: f64,
  pub headline_label: &'static str,
  pub issued_time: String,
  pub issuer: PartyView,
  pub items: Vec<ItemView>,
  pub items_value: f64,
  pub kind: ContractKind,
  pub location_name: String,
  pub route: Option<RouteView>,
  pub status: String,
  pub title: String,
  pub volume: f64,
}

impl ContractDetail {
  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    let mut keys: Vec<(images::ImageKind, i64)> = Vec::new();
    keys.extend(self.issuer.portrait.stale_key());
    if let Some(acceptor) = &self.acceptor {
      keys.extend(acceptor.portrait.stale_key());
    }
    keys
  }
}

#[derive(Clone, Debug)]
pub struct ExpiryView {
  pub future: bool,
  pub label: String,
  pub title: &'static str,
}

#[derive(Clone, Debug)]
pub struct ItemView {
  pub icon: images::IconResolution,
  pub included: bool,
  pub name: String,
  pub quantity: i64,
  pub singleton: bool,
  pub value_isk: f64,
}

#[derive(Clone, Debug)]
pub struct PartyView {
  pub name: String,
  pub portrait: images::ImageState,
  pub role: &'static str,
  pub sub: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RouteView {
  pub end: String,
  pub start: String,
}

pub async fn load_for_character(db: &Database, character_id: i64, contract_id: i64) -> Option<ContractDetail> {
  let rows = finance::contracts(db, character_id).await.ok()?;
  let row = rows.into_iter().find(|row| row.contract_id() == contract_id)?;

  let items = finance::contract_items(db, character_id, contract_id)
    .await
    .unwrap_or_default();
  let bids = finance::contract_bids(db, character_id, contract_id)
    .await
    .unwrap_or_default();

  let item_views = item_views(db, items.iter().map(item_basis)).await;
  let bid_views = bid_views(db, bids.iter().map(bid_basis)).await;

  Some(
    assemble(
      db,
      ContractBasis {
        acceptor_id: row.acceptor_id(),
        acceptor_name: row.acceptor_name().clone(),
        availability: row.availability().clone(),
        collateral: row.collateral(),
        contract_id: row.contract_id(),
        date_completed: row.date_completed().clone(),
        date_issued: row.date_issued().clone(),
        days_to_complete: row.days_to_complete(),
        end_location_id: row.end_location_id(),
        for_corporation: row.for_corporation(),
        issuer_corporation_id: row.issuer_corporation_id(),
        issuer_id: row.issuer_id(),
        issuer_name: row.issuer_name().clone(),
        price: row.price(),
        reward: row.reward(),
        start_location_id: row.start_location_id(),
        status: row.status().clone(),
        title: row.title().clone(),
        r#type: row.r#type().clone(),
        volume: row.volume(),
      },
      item_views,
      bid_views,
    )
    .await,
  )
}

pub async fn load_for_corporation(db: &Database, corporation_id: i64, contract_id: i64) -> Option<ContractDetail> {
  let rows = finance::corporation_contracts(db, corporation_id).await.ok()?;
  let row = rows.into_iter().find(|row| row.contract_id() == contract_id)?;

  let items = finance::corporation_contract_items(db, corporation_id, contract_id)
    .await
    .unwrap_or_default();
  let bids = finance::corporation_contract_bids(db, corporation_id, contract_id)
    .await
    .unwrap_or_default();

  let item_views = item_views(db, items.iter().map(corp_item_basis)).await;
  let bid_views = bid_views(db, bids.iter().map(corp_bid_basis)).await;

  Some(
    assemble(
      db,
      ContractBasis {
        acceptor_id: row.acceptor_id(),
        acceptor_name: row.acceptor_name().clone(),
        availability: row.availability().clone(),
        collateral: row.collateral(),
        contract_id: row.contract_id(),
        date_completed: row.date_completed().clone(),
        date_issued: row.date_issued().clone(),
        days_to_complete: row.days_to_complete(),
        end_location_id: row.end_location_id(),
        for_corporation: row.for_corporation(),
        issuer_corporation_id: row.issuer_corporation_id(),
        issuer_id: row.issuer_id(),
        issuer_name: row.issuer_name().clone(),
        price: row.price(),
        reward: row.reward(),
        start_location_id: row.start_location_id(),
        status: row.status().clone(),
        title: row.title().clone(),
        r#type: row.r#type().clone(),
        volume: row.volume(),
      },
      item_views,
      bid_views,
    )
    .await,
  )
}

pub fn contract_status_color(status: &str) -> iced::Color {
  match status {
    "finished" => color::status::ONLINE,
    "in_progress" => color::accent::PLASMA,
    "outstanding" => color::status::WARNING,
    "failed" | "outbid" | "rejected" | "reversed" => color::status::DANGER,
    "cancelled" | "deleted" | "expired" => color::text::tertiary(),
    _ => color::text::tertiary(),
  }
}

pub fn overlay<'a, M: Clone + 'a>(base: Element<'a, M>, detail: &'a ContractDetail, on_close: M) -> Element<'a, M> {
  iced::widget::Stack::with_children(vec![
    base,
    backdrop::backdrop(on_close.clone()),
    container(modal(detail, on_close))
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

#[derive(Clone, Debug)]
struct BidBasis {
  amount: f64,
  bidder_id: i64,
  when: String,
}

#[derive(Clone, Debug)]
struct ContractBasis {
  acceptor_id: Option<i64>,
  acceptor_name: Option<String>,
  availability: Option<String>,
  collateral: Option<f64>,
  contract_id: i64,
  date_completed: Option<String>,
  date_issued: String,
  days_to_complete: Option<i64>,
  end_location_id: Option<i64>,
  for_corporation: bool,
  issuer_corporation_id: Option<i64>,
  issuer_id: i64,
  issuer_name: Option<String>,
  price: Option<f64>,
  reward: Option<f64>,
  start_location_id: Option<i64>,
  status: String,
  title: Option<String>,
  r#type: String,
  volume: Option<f64>,
}

#[derive(Clone, Debug)]
struct ItemBasis {
  included: bool,
  quantity: i64,
  singleton: bool,
  type_id: i64,
  value_isk: f64,
}

async fn assemble(db: &Database, basis: ContractBasis, items: Vec<ItemView>, bids: Vec<BidView>) -> ContractDetail {
  let kind = ContractKind::from_status(&basis.r#type);
  let is_buyer = !basis.price.is_some_and(|value| value > 0.0) && basis.reward.is_none();
  let price = basis.price.unwrap_or(0.0);
  let reward = basis.reward.unwrap_or(0.0);

  let headline_label = match kind {
    ContractKind::Auction => "Current bid",
    ContractKind::Courier => "Reward",
    ContractKind::ItemExchange if is_buyer => "You pay",
    ContractKind::ItemExchange => "Price",
  };
  let headline = match kind {
    ContractKind::Auction => bids.first().map(|bid| bid.amount).unwrap_or(price),
    ContractKind::Courier => reward,
    ContractKind::ItemExchange => price,
  };

  let items_value = items.iter().map(|item| item.value_isk).sum();

  let issuer = match basis.for_corporation.then_some(basis.issuer_corporation_id).flatten() {
    Some(corp_id) => PartyView {
      name: corporation_name(db, corp_id).await,
      portrait: images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, corp_id),
      role: "Issuer",
      sub: Some(party_name(&basis.issuer_name, basis.issuer_id)),
    },
    None => PartyView {
      name: party_name(&basis.issuer_name, basis.issuer_id),
      portrait: images::resolve(
        &images::default_store(),
        images::ImageKind::CharacterPortrait,
        basis.issuer_id,
      ),
      role: "Issuer",
      sub: issuer_sub(db, &basis).await,
    },
  };
  let acceptor = acceptor_view(db, &basis).await;

  let pickup_name = location_name(db, basis.start_location_id).await;

  let route = if kind == ContractKind::Courier {
    Some(RouteView {
      end: location_name(db, basis.end_location_id).await,
      start: pickup_name.clone(),
    })
  } else {
    None
  };

  ContractDetail {
    acceptor,
    availability: availability_label(&basis.availability),
    bids,
    buyout: None,
    collateral: basis.collateral,
    contract_id: basis.contract_id,
    days_to_complete: basis.days_to_complete,
    expiry: expiry_view(&basis.status, &basis.date_completed, &basis.date_issued),
    headline,
    headline_label,
    issued_time: basis.date_issued.clone(),
    issuer,
    items,
    items_value,
    kind,
    location_name: pickup_name,
    route,
    status: basis.status,
    title: basis
      .title
      .unwrap_or_else(|| format!("Contract #{}", basis.contract_id)),
    volume: basis.volume.unwrap_or(0.0),
  }
}

async fn acceptor_view(db: &Database, basis: &ContractBasis) -> Option<PartyView> {
  let id = basis.acceptor_id?;
  let role = if basis.status == "finished" {
    "Acceptor"
  } else {
    "Hauler"
  };
  let sub = if basis.status == "finished" {
    "Completed"
  } else {
    "In progress"
  };
  // The acceptor/assignee may be a corporation (corp-to-corp contracts); resolve it as a corp when the id
  // matches a known corporation so the corp logo loads instead of a (404) character portrait.
  let is_corp = org::get_corporation(db, id).await.ok().flatten().is_some();
  let (name, portrait) = if is_corp {
    (
      corporation_name(db, id).await,
      images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, id),
    )
  } else {
    (
      party_name(&basis.acceptor_name, id),
      images::resolve(&images::default_store(), images::ImageKind::CharacterPortrait, id),
    )
  };
  Some(PartyView {
    name,
    portrait,
    role,
    sub: Some(sub.to_owned()),
  })
}

fn availability_label(availability: &Option<String>) -> String {
  let raw = availability.clone().unwrap_or_else(|| "personal".to_owned());
  let mut chars = raw.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    None => raw,
  }
}

fn bid_basis(bid: &crate::store::model::CharacterContractBid) -> BidBasis {
  BidBasis {
    amount: bid.amount(),
    bidder_id: bid.bidder_id(),
    when: bid.date_bid().clone(),
  }
}

async fn bid_views(db: &Database, bids: impl Iterator<Item = BidBasis>) -> Vec<BidView> {
  let mut rows: Vec<BidBasis> = bids.collect();
  rows.sort_by(|a, b| b.amount.total_cmp(&a.amount));

  let mut views = Vec::with_capacity(rows.len());
  for basis in &rows {
    views.push(BidView {
      amount: basis.amount,
      bidder: character_name(db, basis.bidder_id).await,
      when: relative_time(&basis.when),
    });
  }
  views
}

fn corp_bid_basis(bid: &crate::store::model::CorporationContractBid) -> BidBasis {
  BidBasis {
    amount: bid.amount(),
    bidder_id: bid.bidder_id(),
    when: bid.date_bid().clone(),
  }
}

fn corp_item_basis(item: &crate::store::model::CorporationContractItem) -> ItemBasis {
  ItemBasis {
    included: item.is_included(),
    quantity: item.quantity(),
    singleton: item.is_singleton(),
    type_id: item.type_id(),
    value_isk: item.value_isk(),
  }
}

fn item_basis(item: &crate::store::model::CharacterContractItem) -> ItemBasis {
  ItemBasis {
    included: item.is_included(),
    quantity: item.quantity(),
    singleton: item.is_singleton(),
    type_id: item.type_id(),
    value_isk: item.value_isk(),
  }
}

async fn item_views(db: &Database, items: impl Iterator<Item = ItemBasis>) -> Vec<ItemView> {
  let rows: Vec<ItemBasis> = items.collect();
  let mut views = Vec::with_capacity(rows.len());
  for basis in &rows {
    views.push(ItemView {
      icon: images::default_store().resolve_type_icon(basis.type_id, None, ITEM_ICON_SIZE),
      included: basis.included,
      name: type_name(db, basis.type_id).await,
      quantity: basis.quantity,
      singleton: basis.singleton,
      value_isk: basis.value_isk,
    });
  }
  views
}

async fn issuer_sub(db: &Database, basis: &ContractBasis) -> Option<String> {
  if let Some(corp_id) = basis.issuer_corporation_id {
    let corp = corporation_name(db, corp_id).await;
    return Some(if basis.for_corporation {
      format!("{corp} \u{00b7} for corp")
    } else {
      corp
    });
  }
  if basis.availability.as_deref() == Some("public") {
    return Some("Public contract".to_owned());
  }
  None
}

async fn location_name(db: &Database, location_id: Option<i64>) -> String {
  let Some(id) = location_id else {
    return "\u{2014}".to_owned();
  };
  if let Some(station) = sde::get_station(db, id).await.ok().flatten() {
    return station.name().clone();
  }
  if let Some(structure) = sde::get_structure(db, id).await.ok().flatten() {
    return structure.name().clone();
  }
  format!("Structure {id}")
}

fn party_name(name: &Option<String>, id: i64) -> String {
  name.clone().unwrap_or_else(|| format!("Pilot {id}"))
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

fn expiry_view(status: &str, date_completed: &Option<String>, date_issued: &str) -> ExpiryView {
  match status {
    "outstanding" | "in_progress" | "outbid" => ExpiryView {
      future: true,
      label: "Open".to_owned(),
      title: "Expires",
    },
    "finished" => ExpiryView {
      future: false,
      label: relative_time(date_completed.as_deref().unwrap_or(date_issued)),
      title: "Completed",
    },
    _ => ExpiryView {
      future: false,
      label: relative_time(date_issued),
      title: "Expired",
    },
  }
}

async fn type_name(db: &Database, type_id: i64) -> String {
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
    .unwrap_or_else(|| format!("Type {type_id}"))
}

pub fn relative_time(iso: &str) -> String {
  let Some(ts) = parse_iso8601(iso) else {
    return iso.to_owned();
  };
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let diff = now - ts;
  if diff < 60 {
    "just now".to_owned()
  } else if diff < 3600 {
    format!("{}m ago", diff / 60)
  } else if diff < 86_400 {
    format!("{}h ago", diff / 3600)
  } else {
    format!("{}d ago", diff / 86_400)
  }
}

fn days_since_epoch(y: i64, m: i64, d: i64) -> i64 {
  let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let doy = (153 * m + 2) / 5 + d - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146_097 + doe - 719_468
}

fn fmt_isk(value: f64) -> String {
  let magnitude = value.abs();
  if magnitude >= 1e9 {
    format!("{:.2}B", value / 1e9)
  } else if magnitude >= 1e6 {
    format!("{:.1}M", value / 1e6)
  } else if magnitude >= 1e3 {
    format!("{:.1}K", value / 1e3)
  } else {
    format!("{value:.0}")
  }
}

fn fmt_volume(value: f64) -> String {
  format!("{value:.1} m\u{00b3}")
}

fn parse_iso8601(s: &str) -> Option<i64> {
  let s = s.trim().trim_end_matches('Z');
  let (date, time) = s.split_once('T')?;
  let date_parts: Vec<i64> = date.split('-').filter_map(|p| p.parse().ok()).collect();
  let time_parts: Vec<i64> = time
    .split('+')
    .next()
    .unwrap_or("")
    .split(':')
    .filter_map(|p| p.parse::<f64>().ok().map(|v| v as i64))
    .collect();
  if date_parts.len() < 3 || time_parts.len() < 3 {
    return None;
  }
  let days = days_since_epoch(date_parts[0], date_parts[1], date_parts[2]);
  Some(days * 86_400 + time_parts[0] * 3600 + time_parts[1] * 60 + time_parts[2])
}

fn modal<'a, M: Clone + 'a>(detail: &'a ContractDetail, on_close: M) -> Element<'a, M> {
  let mut sections: Vec<Element<'a, M>> = vec![
    Row::with_children(vec![parties_panel(detail), headline_panel(detail)])
      .spacing(spacing::SPACE_3_5)
      .align_y(Vertical::Top)
      .width(Length::Fill)
      .into(),
    terms_panel(detail),
  ];
  if let Some(route) = &detail.route {
    sections.push(route_panel(detail, route));
  }
  sections.push(manifest_row(detail));

  let body = container(
    scrollable(
      Column::with_children(sections)
        .spacing(spacing::SPACE_3_5 + spacing::SPACE_2)
        .padding(spacing::SPACE_3_5 + spacing::UNIT)
        .width(Length::Fill),
    )
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Shrink),
  )
  .max_height(MODAL_CONTENT_MAX_HEIGHT);

  container(
    Column::with_children(vec![header(detail, on_close), body.into()])
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

fn header<'a, M: Clone + 'a>(detail: &'a ContractDetail, on_close: M) -> Element<'a, M> {
  let accent = contract_status_color(&detail.status);

  let title = Row::with_children(vec![
    kind_badge(detail.kind),
    text(detail.title.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let meta = Row::with_children(vec![
    meta_text(detail.location_name.clone(), color::text::secondary()),
    dot(),
    meta_text(
      format!("issued {}", relative_time(&detail.issued_time)),
      color::text::secondary(),
    ),
    dot(),
    meta_text(format!("#{}", detail.contract_id), color::text::tertiary()),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let info = Column::with_children(vec![title.into(), meta.into()]).spacing(spacing::UNIT);

  let content = Row::with_children(vec![
    info.into(),
    Space::new().width(Length::Fill).into(),
    status_badge(&detail.status),
    close_button(on_close),
  ])
  .spacing(spacing::SPACE_3)
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
      .padding(spacing::SPACE_3_5 + spacing::SPACE_2)
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

fn parties_panel<'a, M: 'a>(detail: &'a ContractDetail) -> Element<'a, M> {
  let mut rows: Vec<Element<'a, M>> = vec![party_row(&detail.issuer)];
  rows.push(divider());
  match &detail.acceptor {
    Some(acceptor) => rows.push(party_row(acceptor)),
    None => {
      let label = if detail.availability == "Public" {
        "Open to anyone \u{00b7} no acceptor yet"
      } else {
        "Assigned \u{00b7} awaiting acceptance"
      };
      rows.push(
        container(subtitle(label, color::text::tertiary()))
          .padding(Padding {
            top: spacing::SPACE_3 - 1.0,
            right: spacing::SPACE_3_5,
            bottom: spacing::SPACE_3 - 1.0,
            left: spacing::SPACE_3_5,
          })
          .into(),
      );
    }
  }

  panel_unpadded("Parties", None, Column::with_children(rows).width(Length::Fill).into())
}

fn headline_panel<'a, M: 'a>(detail: &'a ContractDetail) -> Element<'a, M> {
  let accent = contract_status_color(&detail.status);

  let value = Row::with_children(vec![
    text(fmt_isk(detail.headline))
      .font(typography::mono::MEDIUM)
      .size(26.0)
      .style(move |_| text::Style {
        color: Some(accent),
      })
      .into(),
    text("ISK")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom);

  let second = if detail.kind == ContractKind::Auction {
    field_cell(
      "Buyout",
      detail
        .buyout
        .map(|v| format!("{} ISK", fmt_isk(v)))
        .unwrap_or_else(|| "\u{2014}".to_owned()),
      None,
    )
  } else {
    field_cell("Volume", fmt_volume(detail.volume), None)
  };

  let collateral = match detail.collateral {
    Some(value) if value > 0.0 => field_cell(
      "Collateral",
      format!("{} ISK", fmt_isk(value)),
      Some(color::status::WARNING),
    ),
    _ => field_cell("Collateral", "\u{2014}".to_owned(), Some(color::text::tertiary())),
  };

  let grid = container(
    Row::with_children(vec![collateral, second])
      .spacing(FIELD_GAP)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .clip(true);

  panel(
    detail.headline_label,
    None,
    Column::with_children(vec![value.into(), grid.into()])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .into(),
  )
}

fn terms_panel<'a, M: 'a>(detail: &'a ContractDetail) -> Element<'a, M> {
  let days = match detail.days_to_complete {
    Some(days) if days > 0 => format!("{days} days"),
    _ => "Immediate".to_owned(),
  };
  let expiry_accent = if detail.expiry.future {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };

  let grid = container(
    Row::with_children(vec![
      field_cell("Type", detail.kind.label().to_owned(), None),
      field_cell("Availability", detail.availability.clone(), None),
      field_cell("Days to complete", days, None),
      field_cell(detail.expiry.title, detail.expiry.label.clone(), Some(expiry_accent)),
    ])
    .spacing(FIELD_GAP)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
    ..container::Style::default()
  });

  panel_unpadded("Terms", None, grid.into())
}

fn route_panel<'a, M: 'a>(detail: &'a ContractDetail, route: &'a RouteView) -> Element<'a, M> {
  let accent = contract_status_color(&detail.status);

  let pickup = Column::with_children(vec![
    eyebrow_text("Pickup", Some(color::text::tertiary())).into(),
    text(route.start.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT - 1.0)
  .width(Length::Fill);

  let destination = container(
    Column::with_children(vec![
      eyebrow_text("Destination", Some(color::text::tertiary())).into(),
      text(route.end.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(spacing::UNIT - 1.0),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Right);

  let connector = Row::with_children(vec![
    route_dot(color::text::secondary()),
    route_rule(),
    text("\u{2192}")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    route_rule(),
    route_dot(accent),
  ])
  .spacing(spacing::SPACE_2 - 2.0)
  .align_y(Vertical::Center);

  panel(
    "Route",
    None,
    Row::with_children(vec![pickup.into(), connector.into(), destination.into()])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill)
      .into(),
  )
}

fn manifest_row<'a, M: 'a>(detail: &'a ContractDetail) -> Element<'a, M> {
  let manifest = item_panel(detail);
  if detail.kind == ContractKind::Auction && !detail.bids.is_empty() {
    Row::with_children(vec![manifest, bids_panel(detail)])
      .spacing(spacing::SPACE_3_5)
      .align_y(Vertical::Top)
      .width(Length::Fill)
      .into()
  } else {
    manifest
  }
}

fn item_panel<'a, M: 'a>(detail: &'a ContractDetail) -> Element<'a, M> {
  let label = if detail.kind == ContractKind::Courier {
    "Cargo manifest"
  } else {
    "Contract items"
  };
  let count = detail.items.len();
  let unit = if count == 1 { "item" } else { "items" };
  let right = format!("{count} {unit} \u{00b7} {} ISK est", fmt_isk(detail.items_value));

  let body: Element<'a, M> = if detail.items.is_empty() {
    container(subtitle("No items recorded", color::text::secondary()))
      .padding(spacing::SPACE_3)
      .into()
  } else {
    let rows: Vec<Element<'a, M>> = detail
      .items
      .iter()
      .enumerate()
      .map(|(index, item)| item_row(item, index == count - 1))
      .collect();
    Column::with_children(rows).width(Length::Fill).into()
  };

  panel_unpadded(label, Some(right), body)
}

fn bids_panel<'a, M: 'a>(detail: &'a ContractDetail) -> Element<'a, M> {
  let count = detail.bids.len();
  let rows: Vec<Element<'a, M>> = detail
    .bids
    .iter()
    .enumerate()
    .map(|(index, bid)| bid_row(bid, index == 0, index == count - 1))
    .collect();

  panel_unpadded(
    "Bids",
    Some(format!("{count}")),
    Column::with_children(rows).width(Length::Fill).into(),
  )
}

fn party_row<'a, M: 'a>(party: &'a PartyView) -> Element<'a, M> {
  let mut lines: Vec<Element<'a, M>> = vec![
    text(party.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(sub) = &party.sub {
    lines.push(subtitle(sub, color::text::secondary()));
  }

  let row = Row::with_children(vec![
    portrait_tile(&party.portrait, &party.name),
    Column::with_children(lines).spacing(2.0).width(Length::Fill).into(),
    text(party.role)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_3 - 1.0)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3 - 3.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3 - 3.0,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn item_row<'a, M: 'a>(item: &'a ItemView, last: bool) -> Element<'a, M> {
  let mut sub = String::new();
  if item.singleton {
    sub.push_str("assembled");
  }
  if !item.included {
    if !sub.is_empty() {
      sub.push_str(" \u{00b7} ");
    }
    sub.push_str("requested");
  }

  let mut name_lines: Vec<Element<'a, M>> = vec![
    text(item.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if !sub.is_empty() {
    name_lines.push(subtitle(&sub, color::text::tertiary()));
  }

  let row = Row::with_children(vec![
    type_icon(&item.icon, ITEM_ICON_BOX),
    Column::with_children(name_lines)
      .spacing(2.0)
      .width(Length::Fill)
      .into(),
    text(format!("\u{00d7}{}", item.quantity))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    text(fmt_isk(item.value_isk))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };
  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: border_bottom,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn bid_row<'a, M: 'a>(bid: &'a BidView, top: bool, last: bool) -> Element<'a, M> {
  let name_color = if top {
    color::accent::PLASMA
  } else {
    color::text::PRIMARY
  };
  let name_font = if top {
    typography::body::MEDIUM
  } else {
    typography::body::REGULAR
  };

  let mut name_line: Vec<Element<'a, M>> = vec![
    text(bid.bidder.clone())
      .font(name_font)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(name_color),
      })
      .into(),
  ];
  if top {
    name_line.push(high_bid_chip());
  }

  let info = Column::with_children(vec![
    Row::with_children(name_line)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    subtitle(&bid.when, color::text::secondary()),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  let amount_color = if top {
    color::accent::PLASMA
  } else {
    color::text::PRIMARY
  };
  let amount = Row::with_children(vec![
    text(fmt_isk(bid.amount))
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(amount_color),
      })
      .into(),
    text("ISK")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Bottom);

  let row = Row::with_children(vec![info.into(), amount.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };
  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3 - 3.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3 - 3.0,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      background: top.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: border_bottom,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn field_cell<'a, M: 'a>(label: &str, value: String, accent: Option<iced::Color>) -> Element<'a, M> {
  container(
    Column::with_children(vec![
      eyebrow_text(label, Some(color::text::tertiary())).into(),
      text(value)
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(move |_| text::Style {
          color: Some(accent.unwrap_or(color::text::PRIMARY)),
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
    ..container::Style::default()
  })
  .into()
}

fn panel<'a, M: 'a>(label: &str, right: Option<String>, content: Element<'a, M>) -> Element<'a, M> {
  panel_unpadded(label, right, container(content).padding(spacing::SPACE_3_5).into())
}

fn panel_unpadded<'a, M: 'a>(label: &str, right: Option<String>, content: Element<'a, M>) -> Element<'a, M> {
  let mut head: Vec<Element<'a, M>> = vec![
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

fn close_button<'a, M: Clone + 'a>(on_close: M) -> Element<'a, M> {
  iced::widget::button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(spacing::SPACE_2 - 1.0)
  .on_press(on_close)
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

fn divider<'a, M: 'a>() -> Element<'a, M> {
  container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      ..container::Style::default()
    })
    .into()
}

fn dot<'a, M: 'a>() -> Element<'a, M> {
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

fn high_bid_chip<'a, M: 'a>() -> Element<'a, M> {
  container(
    text("HIGH BID")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS - 1.0)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
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
      color: color::with_alpha(color::accent::PLASMA, 0.4),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn kind_badge<'a, M: 'a>(kind: ContractKind) -> Element<'a, M> {
  container(
    text(kind.label())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(KIND_TINT),
      }),
  )
  .padding(Padding {
    top: 3.0,
    right: spacing::SPACE_2,
    bottom: 3.0,
    left: spacing::SPACE_2,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(KIND_TINT, 0.12))),
    border: Border {
      color: color::with_alpha(KIND_TINT, 0.3),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn meta_text<'a, M: 'a>(value: String, tint: iced::Color) -> Element<'a, M> {
  text(value)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(move |_| text::Style {
      color: Some(tint),
    })
    .into()
}

fn portrait_tile<'a, M: 'a>(portrait: &images::ImageState, name: &str) -> Element<'a, M> {
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

fn route_dot<'a, M: 'a>(tint: iced::Color) -> Element<'a, M> {
  container(Space::new())
    .width(Length::Fixed(6.0))
    .height(Length::Fixed(6.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(tint)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn route_rule<'a, M: 'a>() -> Element<'a, M> {
  container(Space::new().width(Length::Fixed(ROUTE_RULE)).height(Length::Fixed(1.0)))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.16))),
      ..container::Style::default()
    })
    .into()
}

fn status_badge<'a, M: 'a>(status: &str) -> Element<'a, M> {
  let tint = contract_status_color(status);
  let label = status_label(status);

  let row = Row::with_children(vec![
    container(Space::new())
      .width(Length::Fixed(6.0))
      .height(Length::Fixed(6.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(tint)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2 - 2.0)
  .align_y(Vertical::Center);

  container(row)
    .padding(Padding {
      top: 3.0,
      right: spacing::SPACE_2 + 1.0,
      bottom: 3.0,
      left: spacing::SPACE_2 + 1.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tint, 0.12))),
      border: Border {
        color: color::with_alpha(tint, 0.32),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn status_label(status: &str) -> String {
  let mut out = String::new();
  for (index, word) in status.split('_').enumerate() {
    if index > 0 {
      out.push(' ');
    }
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
      out.extend(first.to_uppercase());
      out.push_str(chars.as_str());
    }
  }
  out
}

fn subtitle<'a, M: 'a>(value: &str, tint: iced::Color) -> Element<'a, M> {
  text(value.to_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(move |_| text::Style {
      color: Some(tint),
    })
    .width(Length::Fill)
    .into()
}

fn type_icon<'a, M: 'a>(icon: &images::IconResolution, box_size: f32) -> Element<'a, M> {
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

  #[derive(Clone, Debug)]
  enum Msg {
    Close,
  }

  fn item_trade() -> ContractDetail {
    ContractDetail {
      acceptor: Some(PartyView {
        name: "Buyer Pilot".to_owned(),
        portrait: images::ImageState::Stale {
          id: 5,
          kind: images::ImageKind::CharacterPortrait,
        },
        role: "Acceptor",
        sub: Some("Completed".to_owned()),
      }),
      availability: "Personal".to_owned(),
      bids: Vec::new(),
      buyout: None,
      collateral: None,
      contract_id: 160_000_001,
      days_to_complete: Some(0),
      expiry: ExpiryView {
        future: false,
        label: "2d ago".to_owned(),
        title: "Completed",
      },
      headline: 235_000_000.0,
      headline_label: "Price",
      issued_time: "2024-01-01T00:00:00Z".to_owned(),
      issuer: PartyView {
        name: "Seller Pilot".to_owned(),
        portrait: images::ImageState::Stale {
          id: 3,
          kind: images::ImageKind::CharacterPortrait,
        },
        role: "Issuer",
        sub: Some("Seller Corp".to_owned()),
      },
      items: vec![ItemView {
        icon: images::IconResolution::Missing,
        included: true,
        name: "Cerberus".to_owned(),
        quantity: 1,
        singleton: true,
        value_isk: 235_000_000.0,
      }],
      items_value: 235_000_000.0,
      kind: ContractKind::ItemExchange,
      location_name: "Jita IV - Moon 4".to_owned(),
      route: None,
      status: "finished".to_owned(),
      title: "Cerberus, faction fit".to_owned(),
      volume: 16_000.0,
    }
  }

  fn courier() -> ContractDetail {
    ContractDetail {
      acceptor: None,
      availability: "Public".to_owned(),
      bids: Vec::new(),
      buyout: None,
      collateral: Some(500_000_000.0),
      contract_id: 160_000_002,
      days_to_complete: Some(7),
      expiry: ExpiryView {
        future: true,
        label: "Open".to_owned(),
        title: "Expires",
      },
      headline: 25_000_000.0,
      headline_label: "Reward",
      issued_time: "2024-01-01T00:00:00Z".to_owned(),
      issuer: PartyView {
        name: "Hauler Co".to_owned(),
        portrait: images::ImageState::Fresh("/tmp/p.jpg".into()),
        role: "Issuer",
        sub: Some("Public contract".to_owned()),
      },
      items: vec![ItemView {
        icon: images::IconResolution::Missing,
        included: true,
        name: "Tritanium".to_owned(),
        quantity: 1_000_000,
        singleton: false,
        value_isk: 5_000_000.0,
      }],
      items_value: 5_000_000.0,
      kind: ContractKind::Courier,
      location_name: "Amarr VIII".to_owned(),
      route: Some(RouteView {
        end: "Jita IV - Moon 4".to_owned(),
        start: "Amarr VIII".to_owned(),
      }),
      status: "outstanding".to_owned(),
      title: "Haul to Jita".to_owned(),
      volume: 10_000_000.0,
    }
  }

  fn auction() -> ContractDetail {
    let mut detail = item_trade();
    detail.bids = vec![
      BidView {
        amount: 12_400_000_000.0,
        bidder: "Rax Tallin".to_owned(),
        when: "1h ago".to_owned(),
      },
      BidView {
        amount: 11_000_000_000.0,
        bidder: "Nemo Krast".to_owned(),
        when: "8h ago".to_owned(),
      },
    ];
    detail.buyout = Some(15_000_000_000.0);
    detail.headline = 12_400_000_000.0;
    detail.headline_label = "Current bid";
    detail.kind = ContractKind::Auction;
    detail.status = "in_progress".to_owned();
    detail
  }

  mod contract_status_color {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_status_to_a_distinct_tone() {
      assert_eq!(contract_status_color("finished"), color::status::ONLINE);
      assert_eq!(contract_status_color("in_progress"), color::accent::PLASMA);
      assert_eq!(contract_status_color("outstanding"), color::status::WARNING);
      assert_eq!(contract_status_color("failed"), color::status::DANGER);
      assert_eq!(contract_status_color("outbid"), color::status::DANGER);
      assert_eq!(contract_status_color("rejected"), color::status::DANGER);
      assert_eq!(contract_status_color("reversed"), color::status::DANGER);
      assert_eq!(contract_status_color("cancelled"), color::text::tertiary());
      assert_eq!(contract_status_color("deleted"), color::text::tertiary());
      assert_eq!(contract_status_color("expired"), color::text::tertiary());
      assert_eq!(contract_status_color("mystery"), color::text::tertiary());
    }
  }

  mod overlay {
    use super::*;

    #[test]
    fn it_renders_an_item_trade() {
      let detail = item_trade();
      let base: Element<'_, Msg> = Space::new().into();
      let _el: Element<'_, Msg> = overlay(base, &detail, Msg::Close);
    }

    #[test]
    fn it_renders_a_courier_with_a_route() {
      let detail = courier();
      let base: Element<'_, Msg> = Space::new().into();
      let _el: Element<'_, Msg> = overlay(base, &detail, Msg::Close);
    }

    #[test]
    fn it_renders_an_auction_with_bids() {
      let detail = auction();
      let base: Element<'_, Msg> = Space::new().into();
      let _el: Element<'_, Msg> = overlay(base, &detail, Msg::Close);
    }

    #[test]
    fn it_renders_without_items_or_an_acceptor() {
      let mut bare = item_trade();
      bare.acceptor = None;
      bare.items = Vec::new();

      let base: Element<'_, Msg> = Space::new().into();
      let _el: Element<'_, Msg> = overlay(base, &bare, Msg::Close);
    }
  }

  mod relative_time {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_raw_string_for_an_unparseable_value() {
      assert_eq!(relative_time("not-a-date"), "not-a-date");
    }

    #[test]
    fn it_buckets_a_parseable_timestamp_into_a_relative_label() {
      let label = relative_time("2000-01-01T00:00:00Z");

      assert!(label.ends_with("d ago"), "expected a days-ago bucket, got {label}");
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_stale_party_portraits() {
      let keys = item_trade().stale_images();

      assert_eq!(keys.len(), 2);
      assert!(keys.contains(&(images::ImageKind::CharacterPortrait, 3)));
      assert!(keys.contains(&(images::ImageKind::CharacterPortrait, 5)));
    }

    #[test]
    fn it_skips_fresh_portraits() {
      let mut detail = courier();
      detail.acceptor = None;

      assert!(detail.stale_images().is_empty());
    }
  }

  mod load_for_character {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self, Database,
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };

    async fn seed_character(db: &Database, id: i64) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_returns_none_for_a_missing_contract() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert!(super::super::load_for_character(&db, 42, 999).await.is_none());
    }

    #[tokio::test]
    async fn it_uses_the_corp_logo_for_a_for_corporation_issuer() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      sqlx::query(
        "INSERT INTO character_contracts \
          (character_id, contract_id, type, status, issuer_id, issuer_name, issuer_corporation_id, price, reward, \
          collateral, volume, for_corporation, date_issued, days_to_complete, start_location_id, end_location_id, \
          availability) \
        VALUES (?, ?, 'item_exchange', 'outstanding', ?, 'Issuer Pilot', ?, ?, NULL, NULL, NULL, 1, ?, NULL, NULL, \
          NULL, 'public')",
      )
      .bind(42_i64)
      .bind(7_i64)
      .bind(42_i64)
      .bind(90_000_001_i64)
      .bind(10_000_000.0_f64)
      .bind("2024-01-01T00:00:00Z")
      .execute(&db.0)
      .await
      .unwrap();

      let detail = super::super::load_for_character(&db, 42, 7).await.unwrap();

      assert_eq!(detail.issuer.name, "Test Corp");
      assert_eq!(
        detail.issuer.portrait.stale_key(),
        Some((images::ImageKind::CorporationLogo, 90_000_001))
      );
    }

    #[tokio::test]
    async fn it_assembles_a_courier_with_a_route_and_fallback_names() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      sqlx::query(
        "INSERT INTO character_contracts \
          (character_id, contract_id, type, status, issuer_id, issuer_name, price, reward, collateral, volume, \
          for_corporation, date_issued, days_to_complete, start_location_id, end_location_id, availability) \
        VALUES (?, ?, 'courier', 'outstanding', ?, 'Issuer Pilot', NULL, ?, ?, ?, 0, ?, 7, ?, ?, 'public')",
      )
      .bind(42_i64)
      .bind(1_i64)
      .bind(95_001_i64)
      .bind(25_000_000.0_f64)
      .bind(500_000_000.0_f64)
      .bind(10_000.0_f64)
      .bind("2024-01-01T00:00:00Z")
      .bind(60_003_760_i64)
      .bind(60_003_761_i64)
      .execute(&db.0)
      .await
      .unwrap();
      sqlx::query(
        "INSERT INTO character_contract_items \
          (character_id, contract_id, record_id, type_id, quantity, is_singleton, is_included, value_isk) \
        VALUES (?, ?, ?, ?, ?, 0, 1, ?)",
      )
      .bind(42_i64)
      .bind(1_i64)
      .bind(1_i64)
      .bind(34_i64)
      .bind(1_000_000_i64)
      .bind(5_000_000.0_f64)
      .execute(&db.0)
      .await
      .unwrap();

      let detail = super::super::load_for_character(&db, 42, 1).await.unwrap();

      assert_eq!(detail.kind, ContractKind::Courier);
      assert_eq!(detail.headline_label, "Reward");
      assert_eq!(detail.headline, 25_000_000.0);
      assert_eq!(detail.availability, "Public");
      assert!(detail.route.is_some());
      assert_eq!(detail.route.as_ref().unwrap().start, "Structure 60003760");
      assert_eq!(detail.items.len(), 1);
      assert_eq!(detail.items[0].name, "Type 34");
    }
  }
}
