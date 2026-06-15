use crate::{
  clients::{
    self, esi,
    eve_sso::{self, PendingAuth},
  },
  store::{Database, model::OwnerType, repo::infra},
};

const DIRECTOR_ROLE: &str = "Director";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callback {
  pub code: String,
  pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorporationAdded {
  pub authorizing_character_id: i64,
  pub corporation_id: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedIn {
  pub character_id: i64,
  pub character_name: String,
}

pub fn redirect_uri() -> String {
  "https://pod.aaronmallen.dev/auth/callback/".to_owned()
}

pub async fn complete_add_corporation(
  sso: &eve_sso::Client,
  esi: &esi::Client,
  db: &Database,
  pending: &PendingAuth,
  callback: &Callback,
) -> Result<CorporationAdded, clients::Error> {
  pending.validate_state(&callback.state)?;
  let grant = sso
    .exchange_code(&callback.code, &redirect_uri(), &pending.verifier)
    .await?;
  let character_id = *grant.character_id();
  let corporation_id = esi.character().public_info(character_id).await?.corporation_id;

  if !is_director_or_ceo(esi, &grant, character_id, corporation_id).await? {
    return Err(clients::Error::Auth(
      "This character must be a Director or CEO of the corporation.".to_owned(),
    ));
  }

  infra::upsert(
    db,
    corporation_id,
    OwnerType::Corporation,
    grant.access_token(),
    grant.refresh_token(),
    grant.expires_at().timestamp(),
    Some(character_id),
    Some(&grant.scopes().join(" ")),
  )
  .await?;
  infra::clear_needs_reauth(db, corporation_id, OwnerType::Corporation).await?;
  Ok(CorporationAdded {
    authorizing_character_id: character_id,
    corporation_id,
  })
}

pub async fn complete_sign_in(
  sso: &eve_sso::Client,
  db: &Database,
  pending: &PendingAuth,
  callback: &Callback,
) -> Result<SignedIn, clients::Error> {
  pending.validate_state(&callback.state)?;
  let grant = sso
    .exchange_code(&callback.code, &redirect_uri(), &pending.verifier)
    .await?;
  infra::upsert(
    db,
    *grant.character_id(),
    OwnerType::Character,
    grant.access_token(),
    grant.refresh_token(),
    grant.expires_at().timestamp(),
    None,
    Some(&grant.scopes().join(" ")),
  )
  .await?;
  infra::clear_needs_reauth(db, *grant.character_id(), OwnerType::Character).await?;
  Ok(SignedIn {
    character_id: *grant.character_id(),
    character_name: grant.character_name().to_owned(),
  })
}

async fn is_director_or_ceo(
  esi: &esi::Client,
  grant: &eve_sso::Grant,
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

pub fn parse_callback(url: &str) -> Option<Callback> {
  let query = url.split_once('?')?.1;
  let query = query.split('#').next().unwrap_or(query);
  let mut code = None;
  let mut state = None;
  for pair in query.split('&') {
    match pair.split_once('=') {
      Some(("code", value)) => code = Some(value.to_owned()),
      Some(("state", value)) => state = Some(value.to_owned()),
      _ => {}
    }
  }
  Some(Callback {
    code: code?,
    state: state?,
  })
}

#[cfg(test)]
mod tests {
  use base64::Engine as _;

  use super::*;
  use crate::{clients::http, store};

  fn jwt(sub: &str) -> String {
    let encode = |raw: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
    format!(
      "{}.{}.sig",
      encode(r#"{"alg":"RS256","typ":"JWT"}"#),
      encode(&format!(r#"{{"sub":"{sub}","name":"Test Pilot","scp":[]}}"#)),
    )
  }

  mod redirect_uri {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_static_https_bounce_url() {
      assert_eq!(redirect_uri(), "https://pod.aaronmallen.dev/auth/callback/");
    }
  }

  mod parse_callback {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_extracts_code_and_state() {
      let parsed = parse_callback("eveauth-pod://callback?code=abc123&state=xyz789");

      assert_eq!(
        parsed,
        Some(Callback {
          code: "abc123".to_owned(),
          state: "xyz789".to_owned(),
        })
      );
    }

    #[test]
    fn it_returns_none_when_a_parameter_is_missing() {
      assert_eq!(parse_callback("eveauth-pod://callback?code=abc123"), None);
      assert_eq!(parse_callback("eveauth-pod://callback"), None);
    }
  }

  mod complete_add_corporation {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::clients::esi::Client as EsiClient;

    const CHARACTER_ID: i64 = 42;
    const CORPORATION_ID: i64 = 2000;

    async fn clients_for(server: &MockServer) -> (eve_sso::Client, EsiClient) {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      let sso = eve_sso::Client::new(http.clone(), "test-client").with_token_url(format!("{}/token", server.uri()));
      let esi = EsiClient::with_base_url(http, server.uri());
      (sso, esi)
    }

    async fn mount_token(server: &MockServer) {
      let body = format!(
        r#"{{"access_token":"{}","expires_in":1200,"refresh_token":"rt"}}"#,
        jwt(&format!("CHARACTER:EVE:{CHARACTER_ID}"))
      );
      Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
        .mount(server)
        .await;
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

    fn callback_for(pending: &PendingAuth) -> Callback {
      Callback {
        code: "code".to_owned(),
        state: pending.state.clone(),
      }
    }

    #[tokio::test]
    async fn it_persists_a_corporation_credential_for_a_director() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Director","Accountant"]"#).await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = callback_for(&pending);

      let added = complete_add_corporation(&sso, &esi, &db, &pending, &callback)
        .await
        .unwrap();

      assert_eq!(added.corporation_id, CORPORATION_ID);
      assert_eq!(added.authorizing_character_id, CHARACTER_ID);
      let credential = infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
        .await
        .unwrap()
        .unwrap();
      assert_eq!(credential.authorized_by(), Some(CHARACTER_ID));
    }

    #[tokio::test]
    async fn it_persists_a_corporation_credential_for_the_ceo() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Accountant"]"#).await;
      mount_corporation_info(&server, CHARACTER_ID).await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = callback_for(&pending);

      let added = complete_add_corporation(&sso, &esi, &db, &pending, &callback)
        .await
        .unwrap();

      assert_eq!(added.corporation_id, CORPORATION_ID);
      assert!(
        infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
          .await
          .unwrap()
          .is_some()
      );
    }

    #[tokio::test]
    async fn it_rejects_a_non_director_non_ceo_without_persisting() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Accountant"]"#).await;
      mount_corporation_info(&server, 999).await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = callback_for(&pending);

      let result = complete_add_corporation(&sso, &esi, &db, &pending, &callback).await;

      assert!(matches!(result, Err(clients::Error::Auth(_))));
      assert!(
        infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
          .await
          .unwrap()
          .is_none()
      );
    }

    #[tokio::test]
    async fn it_rejects_a_mismatched_state_without_persisting() {
      let server = MockServer::start().await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = Callback {
        code: "code".to_owned(),
        state: "tampered".to_owned(),
      };

      let result = complete_add_corporation(&sso, &esi, &db, &pending, &callback).await;

      assert!(result.is_err());
      assert!(
        infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
          .await
          .unwrap()
          .is_none()
      );
    }

    #[tokio::test]
    async fn it_clears_a_previously_set_needs_reauth_flag_on_success() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Director"]"#).await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      infra::upsert(
        &db,
        CORPORATION_ID,
        OwnerType::Corporation,
        "at",
        "rt",
        0,
        Some(CHARACTER_ID),
        Some(""),
      )
      .await
      .unwrap();
      infra::mark_needs_reauth(&db, CORPORATION_ID, OwnerType::Corporation)
        .await
        .unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = callback_for(&pending);

      complete_add_corporation(&sso, &esi, &db, &pending, &callback)
        .await
        .unwrap();

      let credential = infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
        .await
        .unwrap()
        .unwrap();
      assert!(
        !credential.needs_reauth(),
        "a successful corp re-auth must clear the persisted needs-reauth flag"
      );
    }

    #[tokio::test]
    async fn it_leaves_the_flag_set_when_the_re_auth_fails() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Accountant"]"#).await;
      mount_corporation_info(&server, 999).await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      infra::upsert(
        &db,
        CORPORATION_ID,
        OwnerType::Corporation,
        "at",
        "rt",
        0,
        Some(CHARACTER_ID),
        Some(""),
      )
      .await
      .unwrap();
      infra::mark_needs_reauth(&db, CORPORATION_ID, OwnerType::Corporation)
        .await
        .unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = callback_for(&pending);

      let result = complete_add_corporation(&sso, &esi, &db, &pending, &callback).await;

      assert!(matches!(result, Err(clients::Error::Auth(_))));
      let credential = infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
        .await
        .unwrap()
        .unwrap();
      assert!(
        credential.needs_reauth(),
        "a failed corp re-auth must NOT clear the persisted needs-reauth flag"
      );
    }
  }

  mod complete_sign_in {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

    async fn sso_for(server: &MockServer) -> eve_sso::Client {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()))
    }

    #[tokio::test]
    async fn it_exchanges_the_code_and_persists_the_credential() {
      let server = MockServer::start().await;
      let body = format!(
        r#"{{"access_token":"{}","expires_in":1200,"refresh_token":"rt"}}"#,
        jwt("CHARACTER:EVE:42")
      );
      Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
        .mount(&server)
        .await;
      let sso = sso_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-skills.read_skills.v1"], &redirect_uri());
      let callback = Callback {
        code: "code".to_owned(),
        state: pending.state.clone(),
      };

      let signed = complete_sign_in(&sso, &db, &pending, &callback).await.unwrap();

      assert_eq!(signed.character_id, 42);
      assert_eq!(signed.character_name, "Test Pilot");
      assert!(infra::get(&db, 42, OwnerType::Character).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_rejects_a_mismatched_state_without_persisting() {
      let server = MockServer::start().await;
      let sso = sso_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-skills.read_skills.v1"], &redirect_uri());
      let callback = Callback {
        code: "code".to_owned(),
        state: "tampered".to_owned(),
      };

      let result = complete_sign_in(&sso, &db, &pending, &callback).await;

      assert!(result.is_err());
      assert!(infra::get(&db, 42, OwnerType::Character).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_clears_a_previously_set_needs_reauth_flag_on_success() {
      let server = MockServer::start().await;
      let body = format!(
        r#"{{"access_token":"{}","expires_in":1200,"refresh_token":"rt"}}"#,
        jwt("CHARACTER:EVE:42")
      );
      Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
        .mount(&server)
        .await;
      let sso = sso_for(&server).await;
      let db = store::open_test().await.unwrap();
      infra::upsert(&db, 42, OwnerType::Character, "at", "rt", 0, None, Some(""))
        .await
        .unwrap();
      infra::mark_needs_reauth(&db, 42, OwnerType::Character).await.unwrap();
      let pending = sso.sign_in(&["esi-skills.read_skills.v1"], &redirect_uri());
      let callback = Callback {
        code: "code".to_owned(),
        state: pending.state.clone(),
      };

      complete_sign_in(&sso, &db, &pending, &callback).await.unwrap();

      let credential = infra::get(&db, 42, OwnerType::Character).await.unwrap().unwrap();
      assert!(
        !credential.needs_reauth(),
        "a successful character re-auth must clear the persisted needs-reauth flag"
      );
    }
  }
}
