use iced::{
  Element, Length, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, container, text},
};

use crate::{
  store::{
    Database, images,
    repo::{character, org, sde},
  },
  ui::{
    components::{
      avatar::Avatar,
      header::{header as header_band, header_divider, stat_block},
      rule,
      tab_select::{Tab as SelectTab, TabLayout, tab_select_with},
    },
    style::{color, radius, spacing, typography},
  },
};

const LOGO_SIZE: f32 = 44.0;
const PLACEHOLDER: &str = "\u{2014}";
const TAB_BODY_PADDING: f32 = 28.0;
const TAB_STRIP_HEIGHT: f32 = 48.0;

#[derive(Clone, Debug)]
pub struct CorpHead {
  pub alliance: Option<String>,
  pub ceo: Option<String>,
  pub corporation_id: i64,
  pub hq: Option<String>,
  pub logo: images::ImageState,
  pub members: Option<i64>,
  pub name: String,
  pub tax_rate: Option<f64>,
  pub ticker: String,
}

#[derive(Clone, Debug)]
pub enum Message {
  Loaded(Option<CorpHead>),
  TabChanged(Tab),
}

pub struct State {
  active: i64,
  active_tab: Tab,
  head: Option<CorpHead>,
}

impl State {
  pub fn new(active: i64) -> Self {
    Self {
      active,
      active_tab: Tab::ORDER[0],
      head: None,
    }
  }

  pub fn active(&self) -> i64 {
    self.active
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .head
      .as_ref()
      .and_then(|head| head.logo.stale_key())
      .into_iter()
      .collect()
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
  Contacts,
  Killlog,
  Standings,
}

impl Tab {
  const ORDER: [Tab; 3] = [Tab::Standings, Tab::Contacts, Tab::Killlog];

  fn label(self) -> &'static str {
    match self {
      Tab::Contacts => "Contacts",
      Tab::Killlog => "Kill Log",
      Tab::Standings => "Standings",
    }
  }

  fn placeholder_subtitle(self) -> &'static str {
    match self {
      Tab::Contacts => "Corporation contacts will appear here once contact sync ships.",
      Tab::Killlog => "The corporation kill log will appear here once killmail sync ships.",
      Tab::Standings => "Corporation standings will appear here once standings sync ships.",
    }
  }

  fn placeholder_title(self) -> &'static str {
    match self {
      Tab::Contacts => "No contacts yet",
      Tab::Killlog => "No kills yet",
      Tab::Standings => "No standings yet",
    }
  }
}

pub fn load(db: &Database, corporation_id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { load_head(&db, corporation_id).await }, Message::Loaded)
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::Loaded(head) => {
      state.head = head;
      Task::none()
    }
    Message::TabChanged(tab) => {
      state.active_tab = tab;
      Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let body = Column::with_children(vec![
    header(state),
    tab_strip(state.active_tab),
    tab_body(state.active_tab),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

async fn load_head(db: &Database, corporation_id: i64) -> Option<CorpHead> {
  let corp = match org::get_corporation(db, corporation_id).await {
    Ok(Some(corp)) => corp,
    Ok(None) => return None,
    Err(error) => {
      tracing::warn!(corporation_id, %error, "failed to load corporation for detail view");
      return None;
    }
  };

  let alliance = match corp.alliance_id() {
    Some(alliance_id) => org::get_alliance(db, alliance_id)
      .await
      .ok()
      .flatten()
      .map(|alliance| alliance.name().to_owned()),
    None => None,
  };
  let ceo = match corp.ceo_id() {
    Some(ceo_id) => character::get(db, ceo_id)
      .await
      .ok()
      .flatten()
      .map(|character| character.name().to_owned()),
    None => None,
  };
  let hq = match corp.home_station_id() {
    Some(station_id) => sde::get_station(db, station_id)
      .await
      .ok()
      .flatten()
      .map(|station| station.name().to_owned()),
    None => None,
  };
  let store = images::default_store();

  Some(CorpHead {
    alliance,
    ceo,
    corporation_id,
    hq,
    logo: images::resolve(&store, images::ImageKind::CorporationLogo, corporation_id),
    members: corp.member_count().map(i64::from),
    name: corp.name().to_owned(),
    tax_rate: corp.tax_rate(),
    ticker: corp.ticker().to_owned(),
  })
}

fn header(state: &State) -> Element<'_, Message> {
  match &state.head {
    Some(head) => loaded_header(head),
    None => header_band(vec![loading_identity()], Vec::new()),
  }
}

fn loaded_header(head: &CorpHead) -> Element<'_, Message> {
  let left: Vec<Element<'_, Message>> = vec![
    identity(head),
    header_divider(),
    stat_block("Members", format_members(head.members), color::text::PRIMARY, None),
    header_divider(),
    stat_block("Tax Rate", format_tax(head.tax_rate), color::text::PRIMARY, None),
    header_divider(),
    stat_block(
      "Alliance",
      head.alliance.clone().unwrap_or_else(placeholder),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      "CEO",
      head.ceo.clone().unwrap_or_else(placeholder),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      "HQ",
      head.hq.clone().unwrap_or_else(placeholder),
      color::text::PRIMARY,
      None,
    ),
  ];

  header_band(left, Vec::new())
}

fn identity(head: &CorpHead) -> Element<'_, Message> {
  let logo = Avatar::new(
    head.corporation_id,
    &head.ticker,
    Length::Fixed(LOGO_SIZE),
    LOGO_SIZE,
    head.logo.path(),
  )
  .border(color::with_alpha(color::text::PRIMARY, 0.1), 1.0)
  .radius(radius::SUBTLE)
  .view::<Message>();

  let name = text(head.name.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));

  let ticker = text(head.ticker.clone())
    .font(typography::mono::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::accent::PLASMA));

  Row::with_children(vec![
    logo,
    Column::with_children(vec![name.into(), ticker.into()])
      .spacing(spacing::UNIT)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .into()
}

fn loading_identity<'a>() -> Element<'a, Message> {
  text("Loading\u{2026}")
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn tab_strip<'a>(active: Tab) -> Element<'a, Message> {
  let tabs: Vec<SelectTab<'a, Message>> = Tab::ORDER
    .into_iter()
    .map(|tab| {
      let selected = tab == active;
      SelectTab {
        count: String::new(),
        icon: None,
        label: tab.label(),
        on_press: (!selected).then_some(Message::TabChanged(tab)),
        selected,
      }
    })
    .collect();

  let strip = container(tab_select_with(tabs, TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT))
    .padding([0.0, TAB_BODY_PADDING]);

  Column::with_children(vec![strip.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn tab_body<'a>(active: Tab) -> Element<'a, Message> {
  let content = Column::with_children(vec![
    text(active.placeholder_title())
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(active.placeholder_subtitle())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_x(Horizontal::Center);

  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn placeholder() -> String {
  PLACEHOLDER.to_owned()
}

fn format_members(members: Option<i64>) -> String {
  let Some(value) = members else {
    return PLACEHOLDER.to_owned();
  };
  if value >= 1_000_000 {
    format!("{:.1}M", value as f64 / 1e6)
  } else if value >= 1_000 {
    group_thousands(value)
  } else {
    value.to_string()
  }
}

fn group_thousands(value: i64) -> String {
  let digits = value.to_string();
  let mut grouped = String::new();
  let len = digits.len();
  for (index, ch) in digits.chars().enumerate() {
    if index > 0 && (len - index).is_multiple_of(3) {
      grouped.push('\u{2009}'); // thin space (U+2009) as thousands separator
    }
    grouped.push(ch);
  }
  grouped
}

fn format_tax(tax_rate: Option<f64>) -> String {
  match tax_rate {
    Some(rate) => format!("{:.1}%", rate * 100.0),
    None => PLACEHOLDER.to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn head() -> CorpHead {
    CorpHead {
      alliance: Some("Iron Helix Pact".to_owned()),
      ceo: Some("Vex Voronova".to_owned()),
      corporation_id: 98_000_001,
      hq: Some("Jita IV \u{2014} Moon 4".to_owned()),
      logo: images::ImageState::Stale {
        id: 98_000_001,
        kind: images::ImageKind::CorporationLogo,
      },
      members: Some(1247),
      name: "Cobalt Syndicate".to_owned(),
      tax_rate: Some(0.10),
      ticker: "COBSY".to_owned(),
    }
  }

  mod format_members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_millions_thin_space_thousands_and_raw_figures() {
      assert_eq!(format_members(Some(2_400_000)), "2.4M");
      assert_eq!(format_members(Some(12_400)), "12\u{2009}400");
      assert_eq!(format_members(Some(89)), "89");
    }

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_count() {
      assert_eq!(format_members(None), PLACEHOLDER);
    }
  }

  mod format_tax {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_a_one_decimal_percentage() {
      assert_eq!(format_tax(Some(0.10)), "10.0%");
      assert_eq!(format_tax(Some(0.025)), "2.5%");
    }

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_rate() {
      assert_eq!(format_tax(None), PLACEHOLDER);
    }
  }

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_on_the_first_tab() {
      let state = State::new(98_000_001);

      assert_eq!(state.active(), 98_000_001);
      assert_eq!(state.active_tab, Tab::Standings);
    }

    #[test]
    fn it_switches_the_active_tab() {
      let mut state = State::new(98_000_001);

      let _task = update(&mut state, Message::TabChanged(Tab::Contacts));

      assert_eq!(state.active_tab, Tab::Contacts);
    }

    #[test]
    fn it_stores_the_loaded_head() {
      let mut state = State::new(98_000_001);

      let _task = update(&mut state, Message::Loaded(Some(head())));

      assert!(state.head.is_some());
    }

    #[test]
    fn it_reports_a_stale_logo_only_once_loaded() {
      let mut state = State::new(98_000_001);
      assert!(state.stale_images().is_empty());

      let _task = update(&mut state, Message::Loaded(Some(head())));

      assert_eq!(
        state.stale_images(),
        vec![(images::ImageKind::CorporationLogo, 98_000_001)]
      );
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_loading_header_before_data_arrives() {
      let state = State::new(98_000_001);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_loaded_header_and_each_tab_body() {
      let mut state = State::new(98_000_001);
      state.head = Some(head());

      for tab in Tab::ORDER {
        state.active_tab = tab;
        let _el: Element<'_, Message> = view(&state);
      }
    }
  }
}
