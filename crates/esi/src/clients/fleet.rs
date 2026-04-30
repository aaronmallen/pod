//! Client for EVE fleet ESI endpoints.

use serde::Serialize;

use crate::{
  Client as EsiClient, Error,
  models::{
    auth::Grant,
    fleet::{Fleet, FleetMember, FleetSquadCreated, FleetWing, FleetWingCreated},
  },
};

/// Request body for inviting a character to a fleet.
#[derive(Serialize)]
pub struct FleetInviteBody {
  pub character_id: i64,
  pub role: String,
  pub squad_id: Option<i64>,
  pub wing_id: Option<i64>,
}

/// Request body for moving a fleet member.
#[derive(Serialize)]
pub struct FleetMoveMemberBody {
  pub role: String,
  pub squad_id: Option<i64>,
  pub wing_id: Option<i64>,
}

/// Request body for renaming a fleet wing or squad.
#[derive(Serialize)]
pub struct FleetRenameBody {
  pub name: String,
}

/// Request body for updating fleet settings.
#[derive(Serialize)]
pub struct FleetUpdateBody {
  pub is_free_move: Option<bool>,
  pub motd: Option<String>,
}

/// Client for fleet ESI endpoints.
pub struct Client<'a> {
  esi: &'a EsiClient,
  id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the fleet with the given `id`.
  pub(crate) fn new(esi: &'a EsiClient, id: i64) -> Self {
    Self {
      esi,
      id,
    }
  }

  /// Returns an authenticated client bound to the given grant.
  pub fn auth<'b>(&'b self, grant: &'b Grant) -> AuthenticatedClient<'b> {
    AuthenticatedClient {
      esi: self.esi,
      grant,
      id: self.id,
    }
  }
}

/// Authenticated client for fleet ESI endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedClient<'a> {
  esi: &'a EsiClient,
  grant: &'a Grant,
  id: i64,
}

impl AuthenticatedClient<'_> {
  /// Creates a new squad in the given wing.
  pub async fn create_squad(&self, wing_id: i64) -> Result<FleetSquadCreated, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/wings/{wing_id}/squads/", self.id))
          .build(),
        &(),
        self.grant.access_token(),
      )
      .await
  }

  /// Creates a new fleet wing.
  pub async fn create_wing(&self) -> Result<FleetWingCreated, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/wings/", self.id))
          .build(),
        &(),
        self.grant.access_token(),
      )
      .await
  }

  /// Deletes a fleet squad.
  pub async fn delete_squad(&self, squad_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .delete_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/squads/{squad_id}/", self.id))
          .build(),
        self.grant.access_token(),
      )
      .await
  }

  /// Deletes a fleet wing.
  pub async fn delete_wing(&self, wing_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .delete_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/wings/{wing_id}/", self.id))
          .build(),
        self.grant.access_token(),
      )
      .await
  }

  /// Returns information about this fleet.
  pub async fn detail(&self) -> Result<Fleet, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path(format!("v1/fleets/{}/", self.id)).build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Invites a character to the fleet.
  pub async fn invite_member(&self, body: &FleetInviteBody) -> Result<(), Error> {
    self
      .esi
      .http()
      .post_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/members/", self.id))
          .build(),
        body,
        self.grant.access_token(),
      )
      .await
  }

  /// Kicks a member from the fleet.
  pub async fn kick_member(&self, member_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .delete_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/members/{member_id}/", self.id))
          .build(),
        self.grant.access_token(),
      )
      .await
  }

  /// Returns all fleet members (paginated).
  pub async fn members(&self) -> Result<Vec<FleetMember>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/members/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Moves a fleet member to a different wing, squad, or role.
  pub async fn move_member(&self, member_id: i64, body: &FleetMoveMemberBody) -> Result<(), Error> {
    self
      .esi
      .http()
      .put_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/members/{member_id}/", self.id))
          .build(),
        body,
        self.grant.access_token(),
      )
      .await
  }

  /// Renames a fleet squad.
  pub async fn rename_squad(&self, squad_id: i64, name: &str) -> Result<(), Error> {
    self
      .esi
      .http()
      .put_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/squads/{squad_id}/", self.id))
          .build(),
        &FleetRenameBody {
          name: name.to_owned(),
        },
        self.grant.access_token(),
      )
      .await
  }

  /// Renames a fleet wing.
  pub async fn rename_wing(&self, wing_id: i64, name: &str) -> Result<(), Error> {
    self
      .esi
      .http()
      .put_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/wings/{wing_id}/", self.id))
          .build(),
        &FleetRenameBody {
          name: name.to_owned(),
        },
        self.grant.access_token(),
      )
      .await
  }

  /// Updates fleet settings.
  pub async fn update(&self, body: &FleetUpdateBody) -> Result<(), Error> {
    self
      .esi
      .http()
      .put_empty(
        &self.esi.url_builder().path(format!("v1/fleets/{}/", self.id)).build(),
        body,
        self.grant.access_token(),
      )
      .await
  }

  /// Returns all fleet wings with their squads.
  pub async fn wings(&self) -> Result<Vec<FleetWing>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/fleets/{}/wings/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
