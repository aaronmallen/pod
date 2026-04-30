use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{container, row, text},
};

use crate::style::{color, spacing, typography};

pub fn view(is_syncing: bool, progress: f32, secs_since_sync: u64) -> Element<'static, super::Message> {
  let dot_color = if is_syncing {
    color::accent::PLASMA
  } else {
    color::status::ONLINE
  };
  let dot = container(iced::widget::Space::new().width(6.0).height(6.0))
    .width(6.0)
    .height(6.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(dot_color)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let label = if is_syncing {
    syncing_label(progress)
  } else {
    synced_label(secs_since_sync)
  };

  container(
    row([dot.into(), label])
      .spacing(10.0)
      .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 14.0,
    right: 14.0,
  })
  .width(Length::Fill)
  .height(Length::Fill)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn syncing_label(progress: f32) -> Element<'static, super::Message> {
  let pct = (progress * 100.0).round() as u16;
  let bar_fill = pct.max(1);
  let bar_rest = 100u16.saturating_sub(bar_fill);
  row([
    text("Syncing")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    sync_progress_bar(bar_fill, bar_rest),
    text(format!("{pct}%"))
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(10.0)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn sync_progress_bar(bar_fill: u16, bar_rest: u16) -> Element<'static, super::Message> {
  let bar_inner: Element<'static, super::Message> = if bar_rest == 0 {
    container(iced::widget::Space::new().width(Length::Fill).height(2.0))
      .width(Length::Fill)
      .height(2.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        ..container::Style::default()
      })
      .into()
  } else {
    row([
      container(iced::widget::Space::new().width(Length::Fill).height(2.0))
        .width(Length::FillPortion(bar_fill))
        .height(2.0)
        .style(|_| container::Style {
          background: Some(Background::Color(color::accent::PLASMA)),
          ..container::Style::default()
        })
        .into(),
      iced::widget::Space::new()
        .width(Length::FillPortion(bar_rest))
        .height(2.0)
        .into(),
    ])
    .height(2.0)
    .into()
  };
  container(bar_inner)
    .width(200.0)
    .height(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      border: Border {
        radius: 1.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn synced_label(secs_since_sync: u64) -> Element<'static, super::Message> {
  let since = format_since(secs_since_sync);
  row([
    text("Synced")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text("·")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    text(since)
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| text::Style {
        color: Some(Color::from_rgba(0.957, 0.949, 0.925, 0.45)),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn format_since(secs: u64) -> String {
  if secs < 60 {
    format!("{secs}s ago")
  } else {
    let m = secs / 60;
    if m < 60 {
      format!("{m}m ago")
    } else {
      format!("{}h ago", m / 60)
    }
  }
}
