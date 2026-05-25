use iced::{
  Element, Length,
  widget::{column, mouse_area, row, stack},
};
use pod_model::{Character, Corporation};

use crate::{
  components::{self, Icon, NavButton},
  views::{assets, character_detail, characters, mail, settings, skills, wallet},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nav {
  Assets,
  Characters,
  Mail,
  Settings,
  Skills,
  Wallet,
}

#[allow(clippy::large_enum_variant)]
pub enum ActiveView {
  Assets(assets::State),
  CharacterDetail(character_detail::State),
  Characters(characters::State),
  Mail(mail::State),
  Settings(settings::State),
  Skills(skills::State),
  Wallet(wallet::State),
}

impl ActiveView {
  /// Returns the selected character ID when the active view is Skills; 0 otherwise.
  pub fn skills_char_id(&self) -> i64 {
    match self {
      ActiveView::Skills(s) => s.selected_char_id(),
      _ => 0,
    }
  }
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
  Assets(assets::Message),
  CharacterDetail(character_detail::Message),
  Characters(characters::Message),
  DismissToast,
  EveTimeTick,
  HoverNav(Option<Nav>),
  Mail(mail::Message),
  Navigate(Nav),
  RefreshAll,
  Settings(settings::Message),
  ShowToast(String),
  Skills(skills::Message),
  /// Fires on the 60-second background timer to check for expired snoozes.
  SnoozeTick,
  StatusBar(components::status_bar::Message),
  Wallet(wallet::Message),
}

pub struct State {
  pub active_nav: Nav,
  pub active_view: ActiveView,
  pub assets_sidebar_width: f32,
  /// Preserved assets view state for instant restore on navigation return.
  pub cached_assets_state: Option<assets::State>,
  pub characters: Vec<Character>,
  pub corporations: Vec<Corporation>,
  pub esi_connected: bool,
  pub eve_time: String,
  pub feat_asset_tracking: bool,
  pub feat_mail: bool,
  pub feat_skill_monitoring: bool,
  pub feat_wallet: bool,
  pub hovered_nav: Option<Nav>,
  pub mail_folder_pane_width: f32,
  pub mail_message_list_width: f32,
  /// Expired snooze pairs queued while the mail view is inactive.
  ///
  /// Applied to the mail state the next time the user navigates to mail.
  pub pending_snooze_expired: Vec<(i64, i64)>,
  pub refresh_successes: u8,
  pub skills_left_pane_width: f32,
  pub sync: components::status_bar::SyncState,
  pub toast: Option<String>,
  pub wallet_right_rail_width: f32,
}

pub struct Component<'a> {
  state: &'a State,
  window_width: f32,
  window_height: f32,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_width: 800.0,
      window_height: 600.0,
    }
  }

  pub fn window_size(mut self, width: f32, height: f32) -> Self {
    self.window_width = width;
    self.window_height = height;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let rail_el = render_rail(self.state);
    let view = render_active_view(self.state, self.window_width, self.window_height);
    let content = render_content(self.state, view);

    let bar = components::status_bar::Component::new()
      .render(&self.state.eve_time, &self.state.sync, self.state.esi_connected)
      .map(Message::StatusBar);

    column([row([rail_el, content]).height(Length::Fill).into(), bar]).into()
  }
}

fn render_rail(state: &State) -> Element<'_, Message> {
  let active = state.active_nav;
  let hovered = state.hovered_nav;
  let unread_mail: u32 = 0;

  let nav_item = |icon, nav: Nav, has_badge| {
    mouse_area(
      NavButton::new(
        icon,
        active == nav,
        hovered == Some(nav),
        has_badge,
        Message::Navigate(nav),
      )
      .render(),
    )
    .on_enter(Message::HoverNav(Some(nav)))
    .on_exit(Message::HoverNav(None))
    .into()
  };

  let mut nav_items: Vec<Element<'_, Message>> = vec![nav_item(Icon::characters(), Nav::Characters, false)];
  if state.feat_skill_monitoring {
    nav_items.push(nav_item(Icon::skills(), Nav::Skills, false));
  }
  if state.feat_mail {
    nav_items.push(nav_item(Icon::mail(), Nav::Mail, unread_mail > 0));
  }
  if state.feat_wallet {
    nav_items.push(nav_item(Icon::wallet(), Nav::Wallet, false));
  }
  if state.feat_asset_tracking {
    nav_items.push(nav_item(Icon::assets(), Nav::Assets, false));
  }

  let settings_btn = mouse_area(
    NavButton::new(
      Icon::settings(),
      active == Nav::Settings,
      hovered == Some(Nav::Settings),
      false,
      Message::Navigate(Nav::Settings),
    )
    .render(),
  )
  .on_enter(Message::HoverNav(Some(Nav::Settings)))
  .on_exit(Message::HoverNav(None))
  .into();

  components::rail::Component::new(nav_items)
    .bottom_item(settings_btn)
    .render()
}

fn render_active_view(state: &State, window_width: f32, window_height: f32) -> Element<'_, Message> {
  match &state.active_view {
    ActiveView::Assets(s) => assets::Component::new(s).render().map(Message::Assets),
    ActiveView::Settings(s) => settings::Component::new(s).render().map(Message::Settings),
    v => render_active_view_with_size(v, window_width, window_height),
  }
}

fn render_active_view_with_size(view: &ActiveView, window_width: f32, window_height: f32) -> Element<'_, Message> {
  match view {
    ActiveView::CharacterDetail(s) => render_character_detail(s),
    ActiveView::Characters(s) => render_characters(s, window_width, window_height),
    ActiveView::Mail(s) => render_mail(s, window_width),
    ActiveView::Skills(s) => render_skills(s, window_width),
    ActiveView::Wallet(s) => render_wallet(s, window_width),
    _ => unreachable!(),
  }
}

fn render_character_detail(s: &character_detail::State) -> Element<'_, Message> {
  character_detail::Component::new(s)
    .render()
    .map(Message::CharacterDetail)
}

fn render_characters(s: &characters::State, window_width: f32, window_height: f32) -> Element<'_, Message> {
  characters::Component::new(s)
    .window_size(window_width, window_height)
    .render()
    .map(Message::Characters)
}

fn render_mail(s: &mail::State, window_width: f32) -> Element<'_, Message> {
  mail::Component::new(s)
    .window_width(window_width)
    .render()
    .map(Message::Mail)
}

fn render_skills(s: &skills::State, window_width: f32) -> Element<'_, Message> {
  skills::Component::new(s)
    .window_width(window_width)
    .render()
    .map(Message::Skills)
}

fn render_wallet(s: &wallet::State, window_width: f32) -> Element<'_, Message> {
  wallet::Component::new(s)
    .window_width(window_width)
    .render()
    .map(Message::Wallet)
}

fn render_content<'a>(state: &'a State, view: Element<'a, Message>) -> Element<'a, Message> {
  if let Some(toast_msg) = &state.toast {
    stack([view, components::Toast::new(toast_msg).render()])
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  } else {
    view
  }
}
