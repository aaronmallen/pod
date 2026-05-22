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

#[cfg(test)]
mod tests {
  fn make_esi(server_uri: &str) -> (crate::Client, crate::models::auth::Grant) {
    let esi = crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap();
    let grant = crate::models::auth::Grant::new(
      "test-token",
      90_000_001i64,
      "Test Char",
      std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
      "refresh",
      vec![],
    );
    (esi, grant)
  }

  mod calendar {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

    #[tokio::test]
    async fn it_returns_calendar_events_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/calendar/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"[{"event_id":1,"importance":0,"title":"Alliance Meeting"}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (esi, grant) = make_esi(&server.uri());
      let auth = esi.character(&grant);

      let result = auth.calendar().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].event_id, 1);
      assert_eq!(result[0].importance, 0);
      assert_eq!(result[0].title, "Alliance Meeting");
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/calendar/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error":"Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let (esi, grant) = make_esi(&server.uri());
      let auth = esi.character(&grant);

      let result = auth.calendar().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 401,
          ..
        })
      ));
    }
  }
}
