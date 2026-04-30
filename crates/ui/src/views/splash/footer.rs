use iced::Element;

use crate::components::splash_footer;

pub struct Component<'a> {
  phase: &'a crate::views::splash::Phase,
  version: &'a str,
}

impl<'a> Component<'a> {
  pub fn new(phase: &'a crate::views::splash::Phase, version: &'a str) -> Self {
    Self {
      phase,
      version,
    }
  }

  pub fn render<'b, MSG: 'static>(self) -> Element<'b, MSG> {
    splash_footer::Component::new(self.phase, self.version).render::<MSG>()
  }
}
