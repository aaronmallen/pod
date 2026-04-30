//! Character calendar endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{CalendarAttendee, CalendarEvent, CalendarEventDetail, CalendarResponse},
};

impl AuthenticatedClient<'_> {
  /// Returns upcoming calendar events for this character.
  pub async fn calendar(&self) -> Result<Vec<CalendarEvent>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/calendar/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the attendees for a calendar event.
  pub async fn calendar_event_attendees(&self, event_id: i64) -> Result<Vec<CalendarAttendee>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/calendar/{event_id}/attendees/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns full details for a calendar event.
  pub async fn calendar_event(&self, event_id: i64) -> Result<CalendarEventDetail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/calendar/{event_id}/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Responds to a calendar event (accepted/declined/tentative).
  pub async fn respond_calendar_event(&self, event_id: i64, response: CalendarResponse) -> Result<(), Error> {
    self
      .esi
      .http()
      .put_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/calendar/{event_id}/", self.id))
          .build(),
        &response,
        self.grant.access_token(),
      )
      .await
  }
}
