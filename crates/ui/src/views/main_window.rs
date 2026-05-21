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
  StatusBar(components::status_bar::Message),
  Wallet(wallet::Message),
}

pub struct State {
  pub active_nav: Nav,
  pub active_view: ActiveView,
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
    let active = self.state.active_nav;
    let hovered = self.state.hovered_nav;
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

    let mut nav_items: Vec<Element<'a, Message>> = vec![nav_item(Icon::characters(), Nav::Characters, false)];
    if self.state.feat_skill_monitoring {
      nav_items.push(nav_item(Icon::skills(), Nav::Skills, false));
    }
    if self.state.feat_mail {
      nav_items.push(nav_item(Icon::mail(), Nav::Mail, unread_mail > 0));
    }
    if self.state.feat_wallet {
      nav_items.push(nav_item(Icon::wallet(), Nav::Wallet, false));
    }
    if self.state.feat_asset_tracking {
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

    let rail_el = components::rail::Component::new(nav_items)
      .bottom_item(settings_btn)
      .render();

    let view: Element<'a, Message> = match &self.state.active_view {
      ActiveView::Assets(s) => assets::Component::new(s).render().map(Message::Assets),
      ActiveView::CharacterDetail(s) => character_detail::Component::new(s)
        .render()
        .map(Message::CharacterDetail),
      ActiveView::Characters(s) => characters::Component::new(s)
        .window_size(self.window_width, self.window_height)
        .render()
        .map(Message::Characters),
      ActiveView::Mail(s) => mail::Component::new(s)
        .window_width(self.window_width)
        .render()
        .map(Message::Mail),
      ActiveView::Settings(s) => settings::Component::new(s).render().map(Message::Settings),
      ActiveView::Skills(s) => skills::Component::new(s)
        .window_width(self.window_width)
        .render()
        .map(Message::Skills),
      ActiveView::Wallet(s) => wallet::Component::new(s)
        .window_width(self.window_width)
        .render()
        .map(Message::Wallet),
    };

    let content: Element<'a, Message> = if let Some(toast_msg) = &self.state.toast {
      stack([view, components::Toast::new(toast_msg).render()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
      view
    };

    let bar = components::status_bar::Component::new()
      .render(&self.state.eve_time, &self.state.sync, self.state.esi_connected)
      .map(Message::StatusBar);

    column([row([rail_el, content]).height(Length::Fill).into(), bar]).into()
  }
}
