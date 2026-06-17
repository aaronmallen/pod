use iced::{
  Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, button, container, scrollable, text},
};

use super::Outcome;
use crate::{
  features::about::{TRADEMARK_COPYRIGHT, TRADEMARK_NOTICE},
  ui::{
    components::rule,
    style::{color, spacing, typography},
  },
};

const GITHUB_URL: &str = "https://github.com/aaronmallen/pod";
const PANEL_SIDE_PADDING: f32 = 36.0;
const NOTICE_MAX_WIDTH: f32 = 620.0;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_DATE: &str = env!("POD_BUILD_DATE");
const GIT_SHA: &str = env!("POD_GIT_SHA");

#[derive(Clone, Debug)]
pub enum Message {
  OpenGithub,
}

pub fn update(message: Message) -> Outcome {
  match message {
    Message::OpenGithub => {
      open_external(GITHUB_URL);
      Outcome::None
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
  Column::with_children(vec![panel_header(), body()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header<'a>() -> Element<'a, Message> {
  let title = text("About")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text("Pod's identity and the EVE Online Developer License trademark notice.")
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let band = container(identity).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn body<'a>() -> Element<'a, Message> {
  let name = text("Pod")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let version = text(format!("v{VERSION}"))
    .font(typography::body::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let identity_row = Row::with_children(vec![name.into(), version.into()])
    .align_y(Vertical::Bottom)
    .spacing(spacing::SPACE_2);

  let build_info = text(format!("Build {GIT_SHA} \u{00b7} {BUILD_DATE}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let license = text("MIT License")
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::tertiary()));

  let identity = Column::with_children(vec![
    identity_row.into(),
    build_info.into(),
    license.into(),
    github_link(),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  let notice = container(
    text(TRADEMARK_NOTICE)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(NOTICE_MAX_WIDTH);

  let copyright = text(TRADEMARK_COPYRIGHT)
    .font(typography::body::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let inner = container(
    Column::with_children(vec![
      identity.into(),
      rule::horizontal(),
      notice.into(),
      copyright.into(),
    ])
    .spacing(spacing::SPACE_6)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn github_link<'a>() -> Element<'a, Message> {
  button(
    text("github.com/aaronmallen/pod")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::accent::PLASMA)),
  )
  .padding(0)
  .on_press(Message::OpenGithub)
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod notice {
    use super::*;

    #[test]
    fn it_reuses_the_shared_trademark_constants() {
      assert!(TRADEMARK_NOTICE.contains("Fenris Creations"));
      assert!(TRADEMARK_COPYRIGHT.contains("Fenris Creations"));
    }
  }

  mod update {
    use super::*;

    #[test]
    fn opening_github_does_not_persist() {
      assert_eq!(update(Message::OpenGithub), Outcome::None);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_about_panel() {
      let _el: Element<'_, Message> = super::view();
    }
  }
}
