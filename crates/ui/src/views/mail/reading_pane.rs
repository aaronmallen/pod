//! Right pane: full message view.

pub mod action_bar;
pub mod attachment_row;
pub mod empty_state;
pub mod message_body;
pub mod message_header;
pub mod reading_content;

use iced::Element;

use super::State;

/// Messages produced by the reading pane.
#[derive(Clone, Debug)]
pub enum Message {
  ArchivePressed,
  CheckSnoozed,
  DeletePressed,
  ForwardPressed,
  ReplyAllPressed,
  ReplyPressed,
  SnoozeFailed(String),
  /// Closes the calendar widget and returns to the preset list.
  SnoozeCalendarClose,
  /// Confirms the calendar selection and emits its ISO timestamp via `SnoozeSet`.
  SnoozeCalendarConfirm,
  /// Sets the time stepper to the "downtime" preset (11:00 UTC).
  SnoozeCalendarChipDowntime,
  /// Sets the time stepper to the "evening" preset (19:00 UTC).
  SnoozeCalendarChipEvening,
  /// Sets the time stepper to the "morning" preset (09:00 UTC).
  SnoozeCalendarChipMorning,
  /// Steps the hour field down by one.
  SnoozeCalendarHourDown,
  /// Steps the hour field up by one.
  SnoozeCalendarHourUp,
  /// Steps the minute field down by five.
  SnoozeCalendarMinuteDown,
  /// Steps the minute field up by five.
  SnoozeCalendarMinuteUp,
  /// Advances the calendar grid to the next month.
  SnoozeCalendarNextMonth,
  /// Opens the calendar widget in place of the preset list.
  SnoozeCalendarOpen,
  /// Moves the calendar grid to the previous month.
  SnoozeCalendarPrevMonth,
  /// Selects the given day in the calendar (year, month 0-based, day).
  SnoozeCalendarSelectDay(i32, u32, u32),
  /// Emits the ISO 8601 UTC snooze timestamp, or empty string to unsnooze.
  SnoozeSet(String),
  SnoozedExpired(Vec<(i64, i64)>),
  SnoozeToggle,
  StarToggle,
}

/// Builder for the reading pane.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Create a new reading pane builder.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let msg = state
      .selected_message_id
      .as_ref()
      .and_then(|id| state.messages.iter().find(|m| &m.id == id));

    match msg {
      None => empty_state::Component::new().render(),
      Some(msg) => {
        let to_name: &str = if msg.folder == "sent" && !msg.recipients_display.is_empty() {
          &msg.recipients_display
        } else {
          state
            .accounts
            .iter()
            .find(|a| a.id == msg.character_id)
            .map(|a| a.name.as_str())
            .unwrap_or("me")
        };
        reading_content::Component::new(msg, to_name, state.snooze_popover_open, state).render()
      }
    }
  }
}
