use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{column, container, row, text},
};

use crate::style::{color, typography as font};

fn utc_time() -> String {
  let secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let h = (secs / 3600) % 24;
  let m = (secs / 60) % 60;
  let s = secs % 60;
  format!("{h:02}:{m:02}:{s:02}")
}

/// Renders the 24-px footer strip at the bottom of the splash window.
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

  pub fn render<M: 'static>(&self) -> Element<'static, M> {
    let (dot_color, status_label) = match self.phase {
      crate::views::splash::Phase::Loading => (color::accent::PLASMA, "SYNCING"),
      crate::views::splash::Phase::Expanding => (color::accent::PLASMA, "FINALIZING"),
      crate::views::splash::Phase::Done => (color::text::SUCCESS, "READY"),
    };

    let version = format!("v{}", self.version);
    let content: Element<'static, M> = row([
      footer_eve_segment::<M>(),
      footer_sep::<M>(),
      footer_center_segment::<M>(dot_color, status_label),
      footer_sep::<M>(),
      footer_version_segment::<M>(version),
    ])
    .height(Length::Fill)
    .align_y(iced::alignment::Vertical::Center)
    .into();

    let top_border = container(iced::widget::Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
        ..container::Style::default()
      });

    let bar = container(content)
      .width(Length::Fill)
      .height(Length::Fixed(24.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::NAVIGATION)),
        border: Border {
          radius: iced::border::bottom(14.0),
          ..Border::default()
        },
        ..container::Style::default()
      });

    column([top_border.into(), bar.into()]).width(Length::Fill).into()
  }
}

fn footer_sep<M: 'static>() -> Element<'static, M> {
  container(iced::widget::Space::new().width(1.0).height(Length::Fill))
    .width(1.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn footer_eve_segment<M: 'static>() -> Element<'static, M> {
  container(
    row([
      text("EVE")
        .font(font::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      text(utc_time())
        .font(font::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(4.0)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding::from([0, 12]))
  .height(Length::Fill)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn footer_center_segment<M: 'static>(dot_color: Color, status_label: &'static str) -> Element<'static, M> {
  let status_dot = container(iced::widget::Space::new().width(5.0).height(5.0))
    .width(5.0)
    .height(5.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(dot_color)),
      border: Border {
        radius: 2.5.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  container(
    row([
      status_dot.into(),
      text(status_label)
        .font(font::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(4.0)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding::from([0, 12]))
  .width(Length::Fill)
  .height(Length::Fill)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn footer_version_segment<M: 'static>(version: String) -> Element<'static, M> {
  container(
    text(version)
      .font(font::mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding::from([0, 12]))
  .height(Length::Fill)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}
