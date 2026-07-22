use crate::clients::{self, esi, eve_sso::Grant};

const DIRECTOR_ROLE: &str = "Director";

pub async fn eligible_corporation(esi: &esi::Client, grant: &Grant, character_id: i64) -> Result<i64, clients::Error> {
  let corporation_id = esi.character().public_info(character_id).await?.corporation_id;

  if !is_director_or_ceo(esi, grant, character_id, corporation_id).await? {
    return Err(clients::Error::Auth(
      "This character must be a Director or CEO of the corporation.".to_owned(),
    ));
  }

  Ok(corporation_id)
}

async fn is_director_or_ceo(
  esi: &esi::Client,
  grant: &Grant,
  character_id: i64,
  corporation_id: i64,
) -> Result<bool, clients::Error> {
  let roles = esi
    .corporation_authenticated(grant)
    .member_roles(corporation_id)
    .await?;
  let is_director = roles
    .iter()
    .any(|member| member.character_id == character_id && member.roles.iter().any(|role| role == DIRECTOR_ROLE));
  if is_director {
    return Ok(true);
  }

  let info = esi.corporation().info(corporation_id).await?;
  Ok(info.ceo_id == character_id)
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi::Client as EsiClient, http},
    store,
  };

  const CHARACTER_ID: i64 = 42;

  const CORPORATION_ID: i64 = 2000;

  async fn esi_for(server: &MockServer) -> EsiClient {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db)).build();
    EsiClient::with_base_url(http, server.uri())
  }

  fn grant() -> Grant {
    Grant::from_stored("at", CHARACTER_ID, chrono::Utc::now(), "rt", Vec::new())
  }

  async fn mount_public_info(server: &MockServer) {
    let body = format!(
      r#"{{"birthday":"2020-01-01T00:00:00Z","bloodline_id":1,"corporation_id":{CORPORATION_ID},"gender":"male","name":"Test Pilot","race_id":1}}"#
    );
    Mock::given(method("GET"))
      .and(path(format!("/characters/{CHARACTER_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
      .mount(server)
      .await;
  }

  async fn mount_roles(server: &MockServer, roles: &str) {
    let body = format!(r#"[{{"character_id":{CHARACTER_ID},"roles":{roles}}}]"#);
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORPORATION_ID}/roles/")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
      .mount(server)
      .await;
  }

  async fn mount_corporation_info(server: &MockServer, ceo_id: i64) {
    let body = format!(
      r#"{{"ceo_id":{ceo_id},"creator_id":{ceo_id},"member_count":1,"name":"Test Corp","tax_rate":0.0,"ticker":"TEST"}}"#
    );
    Mock::given(method("GET"))
      .and(path(format!("/corporations/{CORPORATION_ID}/")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
      .mount(server)
      .await;
  }

  mod eligible_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rejects_a_character_who_is_neither_director_nor_ceo() {
      let server = MockServer::start().await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Accountant"]"#).await;
      mount_corporation_info(&server, 999).await;
      let esi = esi_for(&server).await;

      let result = eligible_corporation(&esi, &grant(), CHARACTER_ID).await;

      assert!(matches!(result, Err(clients::Error::Auth(_))));
    }

    #[tokio::test]
    async fn it_returns_the_corporation_id_for_a_director() {
      let server = MockServer::start().await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Director","Accountant"]"#).await;
      let esi = esi_for(&server).await;

      let corporation_id = eligible_corporation(&esi, &grant(), CHARACTER_ID).await.unwrap();

      assert_eq!(corporation_id, CORPORATION_ID);
    }

    #[tokio::test]
    async fn it_returns_the_corporation_id_for_the_ceo() {
      let server = MockServer::start().await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Accountant"]"#).await;
      mount_corporation_info(&server, CHARACTER_ID).await;
      let esi = esi_for(&server).await;

      let corporation_id = eligible_corporation(&esi, &grant(), CHARACTER_ID).await.unwrap();

      assert_eq!(corporation_id, CORPORATION_ID);
    }
  }
}
