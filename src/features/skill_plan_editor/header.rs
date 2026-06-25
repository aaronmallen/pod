use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text, text_input},
};

use super::Message;
use crate::ui::{
  components::{icon::Icon, rule, status},
  style::{color, radius, spacing, typography},
};

const HEADER_HEIGHT: f32 = 52.0;

pub(super) fn header<'a>(plan_name: &'a str, dirty: bool, picker_open: bool) -> Element<'a, Message> {
  let picker_label = if picker_open { "Hide picker" } else { "Add skills" };

  let header_row = row(vec![
    close_btn(),
    Space::new().width(spacing::SPACE_2).into(),
    name_input(plan_name),
    dirty_dot(dirty),
    Space::new().width(Length::Fill).into(),
    inert_trigger("Import", Message::ImportRequested),
    Space::new().width(spacing::SPACE_2).into(),
    inert_trigger("Export", Message::ExportRequested),
    Space::new().width(spacing::SPACE_2).into(),
    ghost_btn(picker_label, Message::PickerToggled),
    Space::new().width(spacing::SPACE_2).into(),
    save_btn(dirty),
  ])
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  });

  container(column(vec![
    container(header_row)
      .height(HEADER_HEIGHT)
      .width(Length::Fill)
      .align_y(Vertical::Center)
      .into(),
    rule::horizontal(),
  ]))
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    ..container::Style::default()
  })
  .into()
}

fn close_btn<'a>() -> Element<'a, Message> {
  button(
    text("\u{2190}")
      .font(typography::mono::REGULAR)
      .size(14.0)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::CloseRequested)
  .style(|_, status| button::Style {
    background: hover_bg(status),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    text_color: color::text::secondary(),
    ..button::Style::default()
  })
  .into()
}

fn dirty_dot<'a>(dirty: bool) -> Element<'a, Message> {
  if dirty {
    status::dot_sized(color::accent::PLASMA, 6.0)
  } else {
    Space::new().width(0.0).height(0.0).into()
  }
}

fn ghost_btn<'a>(label: &'a str, on_press: Message) -> Element<'a, Message> {
  button(text(label).font(typography::body::REGULAR).size(13.0))
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 12.0,
      right: 12.0,
    })
    .on_press(on_press)
    .style(|_, status| button::Style {
      background: hover_bg(status),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    })
    .into()
}

fn hover_bg(status: button::Status) -> Option<Background> {
  match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
    }
    _ => None,
  }
}

fn inert_trigger<'a>(label: &'a str, on_press: Message) -> Element<'a, Message> {
  button(
    row(vec![
      text(label).font(typography::body::REGULAR).size(13.0).into(),
      Space::new().width(spacing::SPACE_2).into(),
      Icon::chevron_down().size(13.0).color(color::text::secondary()).render(),
    ])
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(on_press)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if active { 0.25 } else { 0.12 }),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn name_input<'a>(plan_name: &'a str) -> Element<'a, Message> {
  text_input("Untitled plan", plan_name)
    .on_input(Message::NameChanged)
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 8.0,
      right: 8.0,
    })
    .size(15.0)
    .font(typography::body::MEDIUM)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border {
        color: Color::TRANSPARENT,
        radius: 4.0.into(),
        width: 0.0,
      },
      icon: color::text::secondary(),
      placeholder: color::text::tertiary(),
      value: color::text::PRIMARY,
      selection: color::accent::PLASMA_MUTED,
    })
    .into()
}

fn save_btn<'a>(dirty: bool) -> Element<'a, Message> {
  let (label_color, bg) = if dirty {
    (color::surface::BASE, color::accent::PLASMA)
  } else {
    (color::text::tertiary(), color::with_alpha(color::accent::PLASMA, 0.18))
  };

  button(
    text("Save")
      .font(typography::body::MEDIUM)
      .size(13.0)
      .style(move |_| text::Style {
        color: Some(label_color),
      }),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
  })
  .on_press(Message::SaveRequested)
  .style(move |_, _| button::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    text_color: label_color,
    ..button::Style::default()
  })
  .into()
}
