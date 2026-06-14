use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, Stack, button, container, text},
};

use super::{
  Folder, Message, StandardFolder, State, compose, folder_pane, labels, message_list, outbox_indicator, reading_pane,
  snooze, switcher,
};
use crate::{
  config::Feature,
  ui::{
    components::{
      backdrop,
      confirm_modal::confirm_modal,
      eyebrow::eyebrow_text,
      forbidden,
      icon::Icon,
      modal_overlay::modal_overlay,
      positioned_dropdown::{positioned_dropdown, positioned_dropdown_right},
      resizable_pane::pane_handle,
      rule,
    },
    style::{color, radius, spacing, typography},
  },
};

const HEADER_SIDE_PADDING: f32 = 28.0;
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const SNOOZE_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 56.0;
const SNOOZE_OVERLAY_RIGHT: f32 = 120.0;
const LABEL_PICKER_TOP: f32 = spacing::layout::HEADER_HEIGHT + 52.0;
const LABEL_PICKER_READING_OFFSET: f32 = 200.0;

pub(super) fn shell(state: &State) -> Element<'_, Message> {
  let body = Column::with_children(vec![header(state), panes(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  let base = container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  if state.picker_open() {
    let dropdown = positioned_dropdown(switcher::dropdown(state), PICKER_OVERLAY_TOP, PICKER_OVERLAY_LEFT);

    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::PickerToggled),
      dropdown,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  let snooze_overlay: Option<Element<'_, Message>> = if state.snooze_presets_open() {
    Some(snooze::presets_menu(state.open_mail_snoozed(), state.selected()))
  } else {
    state.snooze_calendar().map(snooze::calendar_menu)
  };
  if let Some(menu) = snooze_overlay {
    let positioned = positioned_dropdown_right(menu, SNOOZE_OVERLAY_TOP, SNOOZE_OVERLAY_RIGHT);
    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::SnoozeMenuToggled),
      positioned,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  if let Some(draft) = state.label_modal() {
    return modal_overlay(
      base.into(),
      Some(Message::LabelModalClosed),
      labels::create_modal(draft),
    );
  }

  if let Some(label) = state.pending_label_delete() {
    let body = format!(
      "Deleting \u{201c}{}\u{201d} removes it from every message, here and in EVE.",
      label.name
    );
    let confirm = confirm_modal(
      "Mail label",
      "Delete label?",
      body,
      "Delete",
      Message::LabelDeleteConfirmed,
      Message::LabelDeleteCancelled,
    );
    return modal_overlay(base.into(), Some(Message::LabelDeleteCancelled), confirm);
  }

  if let Some((mail_id, anchor, applied)) = state.label_picker_view() {
    let menu = labels::toggle_picker(mail_id, &state.folder_data().labels, &applied);
    let (top, left) = match anchor {
      Some(point) => (point.y.max(0.0), point.x.max(0.0)),
      None => (
        LABEL_PICKER_TOP,
        state.folder_pane_width() + state.message_list_pane_width() + LABEL_PICKER_READING_OFFSET,
      ),
    };
    let positioned = positioned_dropdown(menu, top, left);
    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::LabelPickerClosed),
      positioned,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  if let Some(draft) = state.compose() {
    return modal_overlay(base.into(), None, compose::panel(draft, state.roster()));
  }

  base.into()
}

fn header(state: &State) -> Element<'_, Message> {
  let mut band_children: Vec<Element<'_, Message>> = vec![
    switcher::trigger(state),
    rule::vertical(44.0),
    count_column(state),
    Space::new().width(Length::Fill).into(),
  ];
  if let Some(indicator) = outbox_indicator::indicator(state.outbox_indicator()) {
    band_children.push(indicator);
    band_children.push(Space::new().width(Length::Fixed(spacing::SPACE_3)).into());
  }
  band_children.push(compose_button());

  let band = Row::with_children(band_children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3 + spacing::SPACE_2);

  container(band)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      right: HEADER_SIDE_PADDING,
      bottom: 0.0,
      left: HEADER_SIDE_PADDING,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn count_column(state: &State) -> Element<'_, Message> {
  let total = state.messages().len();
  let unread = state.messages().iter().filter(|row| !row.is_read).count();

  let caption = eyebrow_text(&folder_caption(state), None);

  let counts = Row::with_children(vec![
    text(format!("{total} messages · "))
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(format!("{unread} unread"))
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::accent::PLASMA))
      .into(),
  ]);

  Column::with_children(vec![caption.into(), counts.into()])
    .spacing(spacing::UNIT)
    .into()
}

fn folder_caption(state: &State) -> String {
  match state.folder() {
    Folder::Unified => "All Inboxes".to_owned(),
    Folder::Standard(StandardFolder::Archive) => "Archive".to_owned(),
    Folder::Standard(StandardFolder::Drafts) => "Drafts".to_owned(),
    Folder::Standard(StandardFolder::Inbox) => "Inbox".to_owned(),
    Folder::Standard(StandardFolder::Sent) => "Sent".to_owned(),
    Folder::Standard(StandardFolder::Snoozed) => "Snoozed".to_owned(),
    Folder::Standard(StandardFolder::Starred) => "Starred".to_owned(),
    Folder::Standard(StandardFolder::Trash) => "Trash".to_owned(),
    Folder::Label(label_id) => state
      .folder_data()
      .labels
      .iter()
      .find(|label| label.label_id == label_id)
      .map(|label| label.name.clone())
      .unwrap_or_else(|| "Folder".to_owned()),
  }
}

fn compose_button<'a>() -> Element<'a, Message> {
  let content = Row::with_children(vec![
    Icon::pencil()
      .size(14.0)
      .color(color::text::secondary())
      .render::<Message>(),
    text("Compose")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(content)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3 + 2.0,
      right: spacing::SPACE_3 + 2.0,
    })
    .on_press(Message::ComposeOpened)
    .style(|_, status| compose_button_style(status))
    .into()
}

fn compose_button_style(status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, if hovered { 0.18 } else { 0.1 }),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    text_color: if hovered {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    },
    ..button::Style::default()
  }
}

fn panes(state: &State) -> Element<'_, Message> {
  if let Some((id, name, missing)) = state.scope_gate() {
    return forbidden::forbidden(Feature::Mail.noun(), name, &missing, Message::ReauthRequested(id));
  }

  Row::with_children(vec![
    folder_pane(state),
    pane_handle(Message::FolderPaneDragStart),
    message_list_pane(state),
    pane_handle(Message::ListPaneDragStart),
    reading_pane(state),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn folder_pane(state: &State) -> Element<'_, Message> {
  folder_pane::pane(state, state.folder_pane_width())
}

fn message_list_pane(state: &State) -> Element<'_, Message> {
  message_list::pane(state, state.message_list_pane_width())
}

fn reading_pane(state: &State) -> Element<'_, Message> {
  reading_pane::pane(state.render(), state.open_mail_snoozed())
}
