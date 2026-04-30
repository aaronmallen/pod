//! Corporation member endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{MemberRoles, MemberTitleEntry, MemberTracking, RoleHistoryEntry},
};

impl AuthenticatedClient<'_> {
  /// Returns the list of member character IDs for this corporation.
  pub async fn members(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v4/corporations/{}/members/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the member limit for this corporation.
  pub async fn member_limit(&self) -> Result<i32, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/members/limit/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the roles assigned to each member.
  pub async fn member_roles(&self) -> Result<Vec<MemberRoles>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/roles/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the role history for members of this corporation (paginated).
  pub async fn member_role_history(&self) -> Result<Vec<RoleHistoryEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/roles/history/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the titles held by each member.
  pub async fn member_titles(&self) -> Result<Vec<MemberTitleEntry>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/members/titles/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns tracking data for all members (paginated).
  pub async fn member_tracking(&self) -> Result<Vec<MemberTracking>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/membertracking/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the shareholders of this corporation (paginated).
  pub async fn shareholders(&self) -> Result<Vec<crate::models::corporation::Shareholder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/shareholders/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
