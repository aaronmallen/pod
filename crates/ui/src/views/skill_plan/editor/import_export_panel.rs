//! Import and export dropdown overlay panels.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Horizontal,
  widget::{button, column, container, text},
};

use super::super::Message;
use crate::style::{color, spacing, typography::body};

/// Renders the import or export dropdown overlay based on which panel is open.
pub struct ImportExportPanel {
  export_open: bool,
  import_open: bool,
}

impl ImportExportPanel {
  /// Creates a new `ImportExportPanel`.
  pub fn new(import_open: bool, export_open: bool) -> Self {
    Self {
      export_open,
      import_open,
    }
  }

  /// Returns the overlay element if either panel is open, or `None` if both are closed.
  pub fn render(self) -> Option<Element<'static, Message>> {
    if self.import_open {
      Some(import_dropdown_overlay().into())
    } else if self.export_open {
      Some(export_dropdown_overlay().into())
    } else {
      None
    }
  }
}

fn dropdown_menu_btn(label: &'static str, on_press: Message) -> button::Button<'static, Message> {
  button(text(label).font(body::REGULAR).size(13.0))
    .width(Length::Fill)
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 14.0,
      right: 14.0,
    })
    .on_press(on_press)
    .style(|_, status| button::Style {
      background: match status {
        button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::SUBTLE_FILL)),
        _ => None,
      },
      border: Border::default(),
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
}

fn dropdown_panel<'a>(items: Vec<Element<'a, Message>>) -> iced::widget::Container<'a, Message> {
  container(column(items).width(Length::Fixed(180.0))).style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
}

fn export_dropdown_overlay<'a>() -> iced::widget::Container<'a, Message> {
  let to_clipboard_btn = dropdown_menu_btn("To clipboard", Message::ExportToClipboard);
  let to_file_btn = dropdown_menu_btn("To file\u{2026}", Message::ExportToFile);
  let dropdown = dropdown_panel(vec![to_clipboard_btn.into(), to_file_btn.into()]);
  container(dropdown)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 52.0,
      right: spacing::SPACE_4 + 70.0,
      ..Padding::ZERO
    })
}

fn import_dropdown_overlay<'a>() -> iced::widget::Container<'a, Message> {
  let from_clipboard_btn = dropdown_menu_btn("From clipboard", Message::ImportFromClipboard);
  let from_file_btn = dropdown_menu_btn("From file\u{2026}", Message::ImportFromFile);
  let dropdown = dropdown_panel(vec![from_clipboard_btn.into(), from_file_btn.into()]);
  container(dropdown)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 52.0,
      right: spacing::SPACE_4 + 180.0 + spacing::SPACE_2 * 3.0 + 70.0,
      ..Padding::ZERO
    })
}
