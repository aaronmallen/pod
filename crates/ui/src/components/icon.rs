use iced::{Color, Element, widget::svg};

use crate::style::color;

pub struct Component {
  handle: svg::Handle,
  size: f32,
  color: Color,
}

impl Component {
  pub fn archive() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/archive.svg"))
  }

  pub fn assets() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/assets.svg"))
  }

  pub fn characters() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/characters.svg"))
  }

  pub fn draft() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/draft.svg"))
  }

  pub fn filter() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/filter.svg"))
  }

  pub fn forward() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/forward.svg"))
  }

  pub fn help() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/help.svg"))
  }

  pub fn inbox() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/inbox.svg"))
  }

  pub fn inbox_all() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/inbox-all.svg"))
  }

  pub fn logo_mark() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/logo/pod-mark.svg"))
  }

  pub fn mail() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/mail.svg"))
  }

  pub fn notif_alliance() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-alliance.svg"))
  }

  pub fn notif_clone() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-clone.svg"))
  }

  pub fn notif_combat() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-combat.svg"))
  }

  pub fn notif_contact() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-contact.svg"))
  }

  pub fn notif_contract() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-contract.svg"))
  }

  pub fn notif_corp() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-corp.svg"))
  }

  pub fn notif_fw() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-fw.svg"))
  }

  pub fn notif_incursion() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-incursion.svg"))
  }

  pub fn notif_industry() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-industry.svg"))
  }

  pub fn notif_insurance() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-insurance.svg"))
  }

  pub fn notif_market() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-market.svg"))
  }

  pub fn notif_mission() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-mission.svg"))
  }

  pub fn notif_reward() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-reward.svg"))
  }

  pub fn notif_standing() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-standing.svg"))
  }

  pub fn notif_structure() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-structure.svg"))
  }

  pub fn notif_system() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-system.svg"))
  }

  pub fn notif_war() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/notif-war.svg"))
  }

  pub fn pencil() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/pencil.svg"))
  }

  pub fn pin() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/pin.svg"))
  }

  pub fn reply() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/reply.svg"))
  }

  pub fn reply_all() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/reply-all.svg"))
  }

  pub fn search() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/search.svg"))
  }

  pub fn send() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/send.svg"))
  }

  pub fn settings() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/settings.svg"))
  }

  pub fn skills() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/skills.svg"))
  }

  pub fn snooze() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/snooze.svg"))
  }

  pub fn star() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/star.svg"))
  }

  pub fn trash() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/trash.svg"))
  }

  pub fn wallet() -> Self {
    Self::from_bytes(include_bytes!("../../../../assets/icons/wallet.svg"))
  }

  pub fn size(mut self, size: f32) -> Self {
    self.size = size;
    self
  }

  pub fn color(mut self, color: Color) -> Self {
    self.color = color;
    self
  }

  pub fn render<'a, MSG: 'static>(self) -> Element<'a, MSG> {
    let c = self.color;
    svg(self.handle)
      .width(self.size)
      .height(self.size)
      .style(move |_, _| svg::Style {
        color: Some(c),
      })
      .into()
  }

  fn from_bytes(bytes: &'static [u8]) -> Self {
    Self {
      handle: svg::Handle::from_memory(bytes),
      size: 20.0,
      color: color::text::SECONDARY,
    }
  }
}
