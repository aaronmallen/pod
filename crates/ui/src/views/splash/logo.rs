use iced::Element;

use crate::components::SplashLogo;

pub struct Component {
  expand: f32,
  pulse: f32,
  rotation: f32,
}

impl Component {
  pub fn new(rotation: f32, pulse: f32, expand: f32) -> Self {
    Self {
      expand,
      pulse,
      rotation,
    }
  }

  pub fn render<'a, MSG: 'static>(self) -> Element<'a, MSG> {
    SplashLogo::new(self.rotation, self.pulse, self.expand).render()
  }
}
