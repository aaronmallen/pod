use iced::{
  Background, Border, Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, text},
};

use crate::ui::{
  components::rule,
  style::{color, spacing, typography},
};

const GITHUB_URL: &str = "https://github.com/aaronmallen/pod";

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_DATE: &str = env!("POD_BUILD_DATE");
const GIT_SHA: &str = env!("POD_GIT_SHA");

#[derive(Clone, Debug)]
pub enum Message {
  OpenGithub,
}

#[must_use]
pub fn update(message: Message) -> bool {
  match message {
    Message::OpenGithub => {
      open_external(GITHUB_URL);
      false
    }
  }
}

fn open_external(url: &str) {
  #[cfg(test)]
  let _ = url;
  #[cfg(not(test))]
  if let Err(error) = open::that_detached(url) {
    tracing::warn!(target: "pod::ui", %error, url, "could not open external link");
  }
}

pub fn view<'a>() -> Element<'a, Message> {
  let title = text("Pod")
    .font(typography::body::MEDIUM)
    .size(24.0)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let version = text(format!("v{VERSION}"))
    .font(typography::body::MEDIUM)
    .size(14.0)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    });

  let build_info = text(format!("Build {GIT_SHA} · {BUILD_DATE}"))
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::TERTIARY),
    });

  let separator = container(rule::horizontal::<Message>()).width(Length::Fixed(240.0));

  let license = text("MIT License").size(typography::size::SM).style(|_| text::Style {
    color: Some(color::text::TERTIARY),
  });

  let content = column([
    title.into(),
    Space::new().height(Length::Fixed(spacing::UNIT)).into(),
    version.into(),
    Space::new().height(Length::Fixed(spacing::UNIT)).into(),
    build_info.into(),
    Space::new().height(Length::Fixed(spacing::SPACE_3)).into(),
    separator.into(),
    Space::new().height(Length::Fixed(spacing::SPACE_3)).into(),
    license.into(),
    Space::new().height(Length::Fixed(spacing::UNIT)).into(),
    github_link(),
  ])
  .align_x(Horizontal::Center);

  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn github_link<'a>() -> Element<'a, Message> {
  button(
    text("github.com/aaronmallen/pod")
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(0)
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .on_press(Message::OpenGithub)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod update {
    use super::*;

    #[test]
    fn it_keeps_the_window_open_when_following_a_link() {
      assert!(!update(Message::OpenGithub));
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_about_card() {
      let _el: Element<'_, Message> = super::view();
    }
  }

  mod identity {
    use super::*;

    #[test]
    fn it_exposes_the_cargo_package_version() {
      assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn it_captures_a_build_date_and_git_sha_at_compile_time() {
      assert!(!BUILD_DATE.is_empty());
      assert!(!GIT_SHA.is_empty());
    }
  }
}
