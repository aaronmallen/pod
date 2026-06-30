use iced::{
  Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, button, container, scrollable, text},
};

use super::Outcome;
use crate::ui::{
  components::{icon::Icon, rule},
  style::{color, spacing, typography},
};

const PANEL_SIDE_PADDING: f32 = 36.0;
const NOTICE_MAX_WIDTH: f32 = 620.0;
const SUPPORT_BLURB_MAX_WIDTH: f32 = 620.0;

const SUPPORT_URL: &str = "https://pod.aaronmallen.dev/#support";
const WEBSITE_LABEL: &str = "pod.aaronmallen.dev";
const WEBSITE_URL: &str = "https://pod.aaronmallen.dev";

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_DATE: &str = env!("POD_BUILD_DATE");
const GIT_SHA: &str = env!("POD_GIT_SHA");

/// The EVE Online Developer License trademark/attribution notice, required on a user-visible
/// surface. Resolved once from the active locale as the single source of truth for the in-app
/// "About" tab.
pub fn trademark_notice() -> &'static str {
  super::i18n::tr_static("settings.about.trademark_notice")
}

pub fn trademark_copyright() -> &'static str {
  super::i18n::tr_static("settings.about.trademark_copyright")
}

#[derive(Clone, Debug)]
pub enum Message {
  OpenSupport,
  OpenWebsite,
}

pub fn update(message: Message) -> Outcome {
  match message {
    Message::OpenSupport => {
      open_external(SUPPORT_URL);
      Outcome::None
    }
    Message::OpenWebsite => {
      open_external(WEBSITE_URL);
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
  let title = text(t!("settings.about.title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(t!("settings.about.blurb"))
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
  let name = text(t!("settings.about.app_name"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let version = text(t!("settings.about.version", version => VERSION))
    .font(typography::body::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let identity_row = Row::with_children(vec![name.into(), version.into()])
    .align_y(Vertical::Bottom)
    .spacing(spacing::SPACE_2);

  let build_info = text(t!("settings.about.build", sha => GIT_SHA, date => BUILD_DATE))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let license = text(t!("settings.about.license"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::tertiary()));

  let identity = Column::with_children(vec![
    identity_row.into(),
    build_info.into(),
    license.into(),
    website_link(),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  let notice = container(
    text(trademark_notice())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(NOTICE_MAX_WIDTH);

  let copyright = text(trademark_copyright())
    .font(typography::body::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let inner = container(
    Column::with_children(vec![
      identity.into(),
      rule::horizontal(),
      support_section(),
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

fn support_section<'a>() -> Element<'a, Message> {
  let heading = text(t!("settings.about.support_heading"))
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));

  let blurb = container(
    text(t!("settings.about.support_blurb"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(SUPPORT_BLURB_MAX_WIDTH);

  Column::with_children(vec![heading.into(), blurb.into(), support_link()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn support_link<'a>() -> Element<'a, Message> {
  let label = Row::with_children(vec![
    Icon::heart()
      .size(typography::size::MD)
      .color(color::accent::PLASMA)
      .render(),
    text(t!("settings.about.support_link"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::accent::PLASMA))
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::UNIT);

  button(label)
    .padding(0)
    .on_press(Message::OpenSupport)
    .style(|_, _| button::Style {
      background: None,
      border: Border::default(),
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    })
    .into()
}

fn website_link<'a>() -> Element<'a, Message> {
  button(
    text(WEBSITE_LABEL)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::accent::PLASMA)),
  )
  .padding(0)
  .on_press(Message::OpenWebsite)
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
    fn it_resolves_the_shared_trademark_strings() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);

      assert!(trademark_notice().contains("Fenris Creations"));
      assert!(trademark_copyright().contains("Fenris Creations"));
    }
  }

  mod update {
    use super::*;

    #[test]
    fn opening_support_does_not_persist() {
      assert_eq!(update(Message::OpenSupport), Outcome::None);
    }

    #[test]
    fn opening_the_website_does_not_persist() {
      assert_eq!(update(Message::OpenWebsite), Outcome::None);
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
