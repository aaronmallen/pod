use iced::{
  Background, Border, Color, Element, Length, Padding, Theme, gradient,
  widget::{button, column, container, image, row, scrollable, text},
};

use crate::style::{color, spacing, typography as font};

/// A single selectable character (or "All" sentinel).
#[derive(Clone, Debug)]
pub struct CharacterEntry {
  /// `None` means the "All Wallets / All Characters" sentinel.
  pub id: Option<i64>,
  pub name: String,
  pub corp_name: String,
  /// Portrait hue 0–360, used for the gradient background.
  pub tone: u16,
  /// Pre-loaded portrait image. When `Some`, renders the actual
  /// portrait instead of initials.
  pub portrait_handle: Option<image::Handle>,
}

/// A single selectable corporation.
#[derive(Clone, Debug)]
pub struct CorporationEntry {
  /// Corporation icon PNG pre-loaded as an image handle.
  pub icon_handle: Option<image::Handle>,
  /// EVE corporation ID.
  pub id: i64,
  pub name: String,
  pub ticker: String,
}

/// The current selection state of the picker.
#[derive(Clone, Debug, Default)]
pub enum PickerSelection {
  /// No specific entity selected — show aggregate data.
  #[default]
  All,
  /// A specific character is selected.
  Character(i64),
  /// A specific corporation is selected.
  Corporation(i64),
}

/// Messages emitted by the character picker.
#[derive(Clone, Debug)]
pub enum Message {
  CloseRequested,
  Select(PickerSelection),
  ToggleOpen,
}

/// Stateful character picker organism.
///
/// Holds open/closed state, the selected ID, and the entry list so
/// callers do not need to duplicate that bookkeeping.
#[derive(Debug)]
pub struct Component {
  pub all_label: String,
  pub corp_entries: Vec<CorporationEntry>,
  pub entries: Vec<CharacterEntry>,
  pub is_open: bool,
  pub selected: PickerSelection,
  pub show_all: bool,
}

impl Component {
  /// Create a new picker with default (closed, nothing selected) state.
  pub fn new() -> Self {
    Self {
      all_label: "All Wallets".to_string(),
      corp_entries: Vec::new(),
      entries: Vec::new(),
      is_open: false,
      selected: PickerSelection::All,
      show_all: false,
    }
  }

  /// Builder: set the label for the "show all" sentinel row.
  pub fn all_label(mut self, label: impl Into<String>) -> Self {
    self.all_label = label.into();
    self
  }

  /// Builder: replace the corporation entry list.
  pub fn corp_entries(mut self, v: Vec<CorporationEntry>) -> Self {
    self.corp_entries = v;
    self
  }

  /// Builder: replace the character entry list.
  pub fn entries(mut self, v: Vec<CharacterEntry>) -> Self {
    self.entries = v;
    self
  }

  /// Returns the selected character ID, or `None` when a corporation
  /// or "All" is selected.
  pub fn selected_character_id(&self) -> Option<i64> {
    match self.selected {
      PickerSelection::Character(id) => Some(id),
      _ => None,
    }
  }

  /// Returns the selected corporation ID, or `None` when a character
  /// or "All" is selected.
  pub fn selected_corporation_id(&self) -> Option<i64> {
    match self.selected {
      PickerSelection::Corporation(id) => Some(id),
      _ => None,
    }
  }

  /// Builder: set the initial selection.
  pub fn selected(mut self, sel: PickerSelection) -> Self {
    self.selected = sel;
    self
  }

  /// Builder: enable the "All" sentinel row.
  pub fn show_all(mut self, v: bool) -> Self {
    self.show_all = v;
    self
  }

  /// Process a picker message, mutating internal state.
  pub fn update(&mut self, msg: Message) {
    match msg {
      Message::CloseRequested => self.is_open = false,
      Message::Select(sel) => {
        self.selected = sel;
        self.is_open = false;
      }
      Message::ToggleOpen => self.is_open = !self.is_open,
    }
  }

  /// Render the trigger button (always shown).
  pub fn render(&self) -> Element<'_, Message> {
    let (name, subtitle, tone, portrait_handle) = selected_display(&self.entries, &self.corp_entries, &self.selected);
    let swatch = portrait_swatch(&name, tone, 38.0, 8.0, portrait_handle);
    let label_col = trigger_label_col(name, subtitle);

    let caret: Element<'_, Message> = text("⌄")
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into();

    let inner: Element<'_, Message> = row([swatch, label_col, caret])
      .spacing(spacing::SPACE_3)
      .align_y(iced::alignment::Vertical::Center)
      .into();

    let is_open = self.is_open;
    button(inner)
      .padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 6.0,
        right: spacing::SPACE_2_5,
      })
      .style(move |_, status| {
        let bg = match (is_open, status) {
          (true, _) => Some(Color::from_rgba(0.957, 0.949, 0.925, 0.06)),
          (false, button::Status::Hovered | button::Status::Pressed) => {
            Some(Color::from_rgba(0.957, 0.949, 0.925, 0.04))
          }
          _ => None,
        };
        button::Style {
          background: bg.map(Background::Color),
          border: Border {
            color: if is_open {
              color::border::DEFAULT
            } else {
              Color::TRANSPARENT
            },
            radius: 10.0.into(),
            width: 1.0,
          },
          text_color: color::text::PRIMARY,
          ..button::Style::default()
        }
      })
      .on_press(Message::ToggleOpen)
      .into()
  }

  /// Render the dropdown popover panel.
  pub fn dropdown(&self) -> Element<'_, Message> {
    let mut rows: Vec<Element<'_, Message>> = vec![dropdown_header()];

    if self.show_all {
      rows.push(picker_row_all(
        &self.all_label,
        matches!(self.selected, PickerSelection::All),
      ));
    }
    for entry in &self.entries {
      if entry.id.is_none() {
        continue;
      }
      let is_selected = match &self.selected {
        PickerSelection::Character(id) => entry.id == Some(*id),
        _ => false,
      };
      rows.push(picker_row(entry, is_selected));
    }

    if !self.corp_entries.is_empty() {
      rows.push(corp_section_header());
      for entry in &self.corp_entries {
        let is_selected = match &self.selected {
          PickerSelection::Corporation(id) => *id == entry.id,
          _ => false,
        };
        rows.push(picker_row_corp(entry, is_selected));
      }
    }

    let list = scrollable(column(rows).width(Length::Fill)).height(Length::Shrink);

    container(list)
      .width(Length::Fixed(360.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::DEFAULT,
          radius: 10.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

fn trigger_label_col(name: String, subtitle: String) -> Element<'static, Message> {
  column([
    text(name)
      .font(font::body::MEDIUM)
      .size(17.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(subtitle.to_uppercase())
      .font(font::mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(3.0)
  .into()
}

fn selected_display(
  entries: &[CharacterEntry],
  corp_entries: &[CorporationEntry],
  selected: &PickerSelection,
) -> (String, String, u16, Option<image::Handle>) {
  match selected {
    PickerSelection::All => {
      let e = entries.iter().find(|e| e.id.is_none()).or_else(|| entries.first());
      e.map(|e| (e.name.clone(), e.corp_name.clone(), e.tone, e.portrait_handle.clone()))
        .unwrap_or_else(|| ("—".to_string(), String::new(), 220, None))
    }
    PickerSelection::Character(id) => {
      let e = entries.iter().find(|e| e.id == Some(*id)).or_else(|| entries.first());
      e.map(|e| (e.name.clone(), e.corp_name.clone(), e.tone, e.portrait_handle.clone()))
        .unwrap_or_else(|| ("—".to_string(), String::new(), 220, None))
    }
    PickerSelection::Corporation(id) => {
      let e = corp_entries.iter().find(|e| e.id == *id);
      e.map(|e| (e.name.clone(), e.ticker.clone(), 220, e.icon_handle.clone()))
        .unwrap_or_else(|| ("—".to_string(), String::new(), 220, None))
    }
  }
}

fn dropdown_header() -> Element<'static, Message> {
  container(
    row([
      text("Switch character")
        .font(font::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      iced::widget::Space::new().width(Length::Fill).into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 14.0,
    right: 14.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn corp_section_header() -> Element<'static, Message> {
  container(
    row([
      text("Corporations")
        .font(font::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      iced::widget::Space::new().width(Length::Fill).into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 14.0,
    right: 14.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn picker_row(entry: &CharacterEntry, selected: bool) -> Element<'static, Message> {
  let swatch = portrait_swatch(&entry.name, entry.tone, 30.0, 6.0, entry.portrait_handle.clone());

  let label_col: Element<'static, Message> = column([
    text(entry.name.clone())
      .font(font::body::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(entry.corp_name.to_uppercase())
      .font(font::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill)
  .into();

  let inner: Element<'static, Message> = row([swatch, label_col])
    .spacing(spacing::SPACE_3)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into();

  let id = entry.id.unwrap_or(0);
  button(inner)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: if selected { 12.0 } else { 14.0 },
      right: 14.0,
    })
    .width(Length::Fill)
    .style(picker_row_style(selected))
    .on_press(Message::Select(PickerSelection::Character(id)))
    .into()
}

fn picker_row_corp(entry: &CorporationEntry, selected: bool) -> Element<'static, Message> {
  let swatch = portrait_swatch(&entry.ticker, 220, 30.0, 6.0, entry.icon_handle.clone());

  let label_col: Element<'static, Message> = column([
    text(entry.name.clone())
      .font(font::body::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(entry.ticker.to_uppercase())
      .font(font::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill)
  .into();

  let inner: Element<'static, Message> = row([swatch, label_col])
    .spacing(spacing::SPACE_3)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into();

  let id = entry.id;
  button(inner)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: if selected { 12.0 } else { 14.0 },
      right: 14.0,
    })
    .width(Length::Fill)
    .style(picker_row_style(selected))
    .on_press(Message::Select(PickerSelection::Corporation(id)))
    .into()
}

fn picker_row_all(label_str: &str, selected: bool) -> Element<'static, Message> {
  let label: Element<'_, Message> = text(label_str.to_string())
    .font(font::body::MEDIUM)
    .size(14.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into();

  let inner: Element<'_, Message> = row([all_wallets_swatch(), label])
    .spacing(spacing::SPACE_3)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into();

  button(inner)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: if selected { 12.0 } else { 14.0 },
      right: 14.0,
    })
    .width(Length::Fill)
    .style(picker_row_style(selected))
    .on_press(Message::Select(PickerSelection::All))
    .into()
}

fn all_wallets_swatch() -> Element<'static, Message> {
  container(
    text("∑")
      .font(font::mono::REGULAR)
      .size(16.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fixed(30.0))
  .height(Length::Fixed(30.0))
  .style(|_| container::Style {
    background: Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.06))),
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn picker_row_style(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let bg = match (selected, status) {
      (true, _) => Some(Color::from_rgba(0.247, 0.722, 0.859, 0.08)),
      (false, button::Status::Hovered | button::Status::Pressed) => Some(Color::from_rgba(0.957, 0.949, 0.925, 0.04)),
      _ => None,
    };
    button::Style {
      background: bg.map(Background::Color),
      border: Border {
        color: if selected {
          color::accent::PLASMA
        } else {
          Color::TRANSPARENT
        },
        radius: 0.0.into(),
        width: if selected { 2.0 } else { 0.0 },
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  }
}

fn portrait_image_swatch<MSG: 'static>(handle: image::Handle, size: f32, radius: f32) -> Element<'static, MSG> {
  container(
    image(handle)
      .width(Length::Fixed(size))
      .height(Length::Fixed(size))
      .content_fit(iced::ContentFit::Cover),
  )
  .width(Length::Fixed(size))
  .height(Length::Fixed(size))
  .clip(true)
  .style(move |_| container::Style {
    border: Border {
      radius: radius.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn initials_swatch<MSG: 'static>(name: &str, tone: u16, size: f32, radius: f32) -> Element<'static, MSG> {
  let initials = name
    .split_whitespace()
    .filter_map(|w| w.chars().next())
    .take(2)
    .map(|c| c.to_uppercase().next().unwrap_or(c))
    .collect::<String>();
  let h = tone as f32 / 360.0;
  let (r0, g0, b0) = hsl_to_rgb(h, 0.28, 0.28);
  let (r1, g1, b1) = hsl_to_rgb(h, 0.18, 0.16);
  let grad = gradient::Linear::new(std::f32::consts::PI * 0.75)
    .add_stop(0.0, Color::from_rgb(r0, g0, b0))
    .add_stop(1.0, Color::from_rgb(r1, g1, b1));
  container(
    text(initials)
      .font(font::body::MEDIUM)
      .size(size * 0.40)
      .style(|_| iced::widget::text::Style {
        color: Some(Color::from_rgba(0.957, 0.949, 0.925, 0.70)),
      }),
  )
  .width(Length::Fixed(size))
  .height(Length::Fixed(size))
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(iced::alignment::Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Gradient(grad.into())),
    border: Border {
      radius: radius.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn portrait_swatch<MSG: 'static>(
  name: &str,
  tone: u16,
  size: f32,
  radius: f32,
  portrait_handle: Option<image::Handle>,
) -> Element<'static, MSG> {
  if let Some(handle) = portrait_handle {
    return portrait_image_swatch(handle, size, radius);
  }
  initials_swatch(name, tone, size, radius)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
  if s == 0.0 {
    return (l, l, l);
  }
  let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
  let p = 2.0 * l - q;
  (
    hue_to_channel(p, q, h + 1.0 / 3.0),
    hue_to_channel(p, q, h),
    hue_to_channel(p, q, h - 1.0 / 3.0),
  )
}

fn hue_to_channel(p: f32, q: f32, mut t: f32) -> f32 {
  if t < 0.0 {
    t += 1.0;
  }
  if t > 1.0 {
    t -= 1.0;
  }
  if t < 1.0 / 6.0 {
    return p + (q - p) * 6.0 * t;
  }
  if t < 1.0 / 2.0 {
    return q;
  }
  if t < 2.0 / 3.0 {
    return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
  }
  p
}
