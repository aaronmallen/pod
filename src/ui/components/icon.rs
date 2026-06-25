use std::f32::consts::{FRAC_PI_2, PI};

use iced::{Color, Element, Radians, Rotation, widget::svg};

use crate::ui::style::color;

const DEFAULT_SIZE: f32 = 20.0;

pub struct Icon {
  color: Color,
  handle: svg::Handle,
  rotation: Radians,
  size: f32,
}

impl Icon {
  pub fn abyssals() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/abyssals.svg"))
  }

  pub fn archive() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/archive.svg"))
  }

  pub fn arrow_out() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/arrow-out.svg"))
  }

  pub fn assets() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/assets.svg"))
  }

  pub fn bold() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/bold.svg"))
  }

  pub fn budget() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/budget.svg"))
  }

  pub fn calendar() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/calendar.svg"))
  }

  pub fn characters() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/characters.svg"))
  }

  pub fn check() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/check.svg"))
  }

  pub fn chevron() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/chevron.svg"))
  }

  #[allow(dead_code)]
  pub fn chevron_down() -> Self {
    Self::chevron()
  }

  #[allow(dead_code)]
  pub fn chevron_up() -> Self {
    Self::chevron().rotation(Radians(PI))
  }

  pub fn chevron_left() -> Self {
    Self::chevron().rotation(Radians(FRAC_PI_2))
  }

  pub fn chevron_right() -> Self {
    Self::chevron().rotation(Radians(-FRAC_PI_2))
  }

  pub fn clock() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/clock.svg"))
  }

  pub fn close() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/close.svg"))
  }

  pub fn compare() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/compare.svg"))
  }

  pub fn contracts() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/contracts.svg"))
  }

  pub fn copy() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/copy.svg"))
  }

  pub fn corp() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/corp.svg"))
  }

  pub fn cross() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/cross.svg"))
  }

  pub fn doc() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/doc.svg"))
  }

  pub fn draft() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/draft.svg"))
  }

  pub fn faction() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/faction.svg"))
  }

  pub fn filter() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/filter.svg"))
  }

  pub fn flask() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/flask.svg"))
  }

  pub fn fleet() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/fleet.svg"))
  }

  pub fn forward() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/forward.svg"))
  }

  pub fn heart() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/heart.svg"))
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

  pub fn inventory() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/inventory.svg"))
  }

  pub fn italic() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/italic.svg"))
  }

  pub fn journal() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/journal.svg"))
  }

  pub fn link() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/link.svg"))
  }

  pub fn live() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/live.svg"))
  }

  pub fn lock() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/lock.svg"))
  }

  pub fn market() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/market.svg"))
  }

  pub fn moon() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/moon.svg"))
  }

  pub fn mutamarket() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/mutamarket.svg"))
  }

  pub fn notif_clone() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-clone.svg"))
  }

  pub fn notif_combat() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-combat.svg"))
  }

  pub fn notif_contact() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-contact.svg"))
  }

  pub fn notif_contract() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-contract.svg"))
  }

  pub fn notif_corp() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-corp.svg"))
  }

  pub fn notif_fw() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/notif-fw.svg"))
  }

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

  pub fn personal() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/personal.svg"))
  }

  pub fn plans() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/plans.svg"))
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

  pub fn skills() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/skills.svg"))
  }

  pub fn snooze() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/snooze.svg"))
  }

  pub fn star() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/star.svg"))
  }

  pub fn stockpiles() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/stockpiles.svg"))
  }

  pub fn tack() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tack.svg"))
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

  pub fn tilde() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tilde.svg"))
  }

  pub fn tracker() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/tracker.svg"))
  }

  pub fn trash() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/trash.svg"))
  }

  pub fn values() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/values.svg"))
  }

  pub fn wallet() -> Self {
    Self::from_bytes(include_bytes!("../../../assets/images/icons/wallet.svg"))
  }

  fn from_bytes(bytes: &'static [u8]) -> Self {
    Self {
      color: color::text::secondary(),
      handle: svg::Handle::from_memory(bytes),
      rotation: Radians(0.0),
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
      .rotation(Rotation::Floating(self.rotation))
      .style(move |_, _| svg::Style {
        color: Some(tint),
      })
      .into()
  }

  pub fn rotation(mut self, rotation: Radians) -> Self {
    self.rotation = rotation;
    self
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

  mod chevron {
    use super::*;

    #[test]
    fn down_uses_the_unrotated_base() {
      assert_eq!(Icon::chevron_down().rotation, Radians(0.0));
      assert_eq!(Icon::chevron().rotation, Radians(0.0));
    }

    #[test]
    fn up_is_a_half_turn() {
      assert_eq!(Icon::chevron_up().rotation, Radians(PI));
    }

    #[test]
    fn left_is_a_quarter_turn() {
      assert_eq!(Icon::chevron_left().rotation, Radians(FRAC_PI_2));
    }

    #[test]
    fn right_is_the_opposite_quarter_turn() {
      assert_eq!(Icon::chevron_right().rotation, Radians(-FRAC_PI_2));
    }

    #[test]
    fn directional_helpers_are_chainable_and_render() {
      let _up: Element<'_, ()> = Icon::chevron_up().size(14.0).color(color::text::PRIMARY).render();
      let _down: Element<'_, ()> = Icon::chevron_down().render();
      let _left: Element<'_, ()> = Icon::chevron_left().render();
      let _right: Element<'_, ()> = Icon::chevron_right().render();
    }
  }
}
