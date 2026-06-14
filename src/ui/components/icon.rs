use iced::{Color, Element, widget::svg};

use crate::ui::style::color;

const DEFAULT_SIZE: f32 = 20.0;

pub struct Icon {
  color: Color,
  handle: svg::Handle,
  size: f32,
}

impl Icon {
  pub fn archive() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/archive.svg"))
  }

  #[allow(dead_code)]
  pub fn assets() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/assets.svg"))
  }

  #[allow(dead_code)]
  pub fn calendar() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/calendar.svg"))
  }

  #[allow(dead_code)]
  pub fn caret() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/caret.svg"))
  }

  #[allow(dead_code)]
  pub fn characters() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/characters.svg"))
  }

  #[allow(dead_code)]
  pub fn check() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/check.svg"))
  }

  pub fn chevron() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/chevron.svg"))
  }

  #[allow(dead_code)]
  pub fn chevron_left() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/chevron-left.svg"))
  }

  pub fn chevron_right() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/chevron-right.svg"))
  }

  #[allow(dead_code)]
  pub fn clock() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/clock.svg"))
  }

  #[allow(dead_code)]
  pub fn close() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/close.svg"))
  }

  pub fn compare() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/compare.svg"))
  }

  pub fn copy() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/copy.svg"))
  }

  pub fn doc() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/doc.svg"))
  }

  #[allow(dead_code)]
  pub fn corp() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/corp.svg"))
  }

  #[allow(dead_code)]
  pub fn cross() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/cross.svg"))
  }

  pub fn draft() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/draft.svg"))
  }

  #[allow(dead_code)]
  pub fn faction() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/faction.svg"))
  }

  #[allow(dead_code)]
  pub fn filter() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/filter.svg"))
  }

  #[allow(dead_code)]
  pub fn fleet() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/fleet.svg"))
  }

  pub fn forward() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/forward.svg"))
  }

  pub fn help() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/help.svg"))
  }

  pub fn inbox() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/inbox.svg"))
  }

  pub fn inbox_all() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/inbox-all.svg"))
  }

  pub fn industry() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/industry.svg"))
  }

  #[allow(dead_code)]
  pub fn live() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/live.svg"))
  }

  #[allow(dead_code)]
  pub fn lock() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/lock.svg"))
  }

  #[allow(dead_code)]
  pub fn logo_mark() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/identity/pod-mark.svg"))
  }

  #[allow(dead_code)]
  pub fn mail() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/mail.svg"))
  }

  #[allow(dead_code)]
  pub fn mutamarket() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/mutamarket.svg"))
  }

  #[allow(dead_code)]
  pub fn notif_alliance() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-alliance.svg"))
  }

  pub fn notif_clone() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-clone.svg"))
  }

  pub fn notif_combat() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-combat.svg"))
  }

  #[allow(dead_code)]
  pub fn notif_contact() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-contact.svg"))
  }

  pub fn notif_contract() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-contract.svg"))
  }

  pub fn notif_corp() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-corp.svg"))
  }

  #[allow(dead_code)]
  pub fn notif_fw() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-fw.svg"))
  }

  #[allow(dead_code)]
  pub fn notif_incursion() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-incursion.svg"))
  }

  pub fn notif_industry() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-industry.svg"))
  }

  pub fn notif_insurance() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-insurance.svg"))
  }

  pub fn notif_market() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-market.svg"))
  }

  pub fn notif_mission() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-mission.svg"))
  }

  pub fn notif_reward() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-reward.svg"))
  }

  pub fn notif_standing() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-standing.svg"))
  }

  pub fn notif_structure() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-structure.svg"))
  }

  pub fn notif_system() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-system.svg"))
  }

  pub fn notif_war() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-war.svg"))
  }

  pub fn pencil() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/pencil.svg"))
  }

  #[allow(dead_code)]
  pub fn personal() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/personal.svg"))
  }

  pub fn pin() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/pin.svg"))
  }

  pub fn plus() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/plus.svg"))
  }

  pub fn reply() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/reply.svg"))
  }

  pub fn reply_all() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/reply-all.svg"))
  }

  pub fn search() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/search.svg"))
  }

  pub fn send() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/send.svg"))
  }

  #[allow(dead_code)]
  pub fn settings() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/settings.svg"))
  }

  #[allow(dead_code)]
  pub fn skills() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/skills.svg"))
  }

  pub fn snooze() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/snooze.svg"))
  }

  pub fn star() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/star.svg"))
  }

  pub fn tag() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tag.svg"))
  }

  pub fn tier_all() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tier-all.svg"))
  }

  pub fn tier_constellation() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tier-constellation.svg"))
  }

  pub fn tier_region() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tier-region.svg"))
  }

  pub fn tier_station() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tier-station.svg"))
  }

  pub fn tier_system() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tier-system.svg"))
  }

  #[allow(dead_code)]
  pub fn tilde() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tilde.svg"))
  }

  pub fn trash() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/trash.svg"))
  }

  #[allow(dead_code)]
  pub fn users() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/users.svg"))
  }

  #[allow(dead_code)]
  pub fn wallet() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/wallet.svg"))
  }

  fn from_bytes(bytes: &'static [u8]) -> Self {
    Self {
      color: color::text::secondary(),
      handle: svg::Handle::from_memory(bytes),
      size: DEFAULT_SIZE,
    }
  }

  pub fn color(mut self, color: Color) -> Self {
    self.color = color;
    self
  }

  pub fn render<'a, M: 'static>(self) -> Element<'a, M> {
    let tint = self.color;
    svg(self.handle)
      .width(self.size)
      .height(self.size)
      .style(move |_, _| svg::Style {
        color: Some(tint),
      })
      .into()
  }

  pub fn size(mut self, size: f32) -> Self {
    self.size = size;
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod render {
    use super::*;

    #[test]
    fn it_builds_a_default_icon() {
      let _el: Element<'_, ()> = Icon::search().render();
    }

    #[test]
    fn it_builds_a_sized_and_tinted_icon() {
      let _el: Element<'_, ()> = Icon::pencil().size(14.0).color(color::text::PRIMARY).render();
    }
  }
}
