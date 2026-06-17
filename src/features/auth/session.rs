use crate::{
  clients::{
    self,
    eve_sso::{self, Grant, PendingAuth},
  },
  store::{Database, model::OwnerType, repo::infra},
};

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

pub async fn exchange_grant(
  sso: &eve_sso::Client,
  pending: &PendingAuth,
  callback: &Callback,
) -> Result<Grant, clients::Error> {
  pending.validate_state(&callback.state)?;
  sso
    .exchange_code(&callback.code, &redirect_uri(), &pending.verifier)
    .await
}

pub async fn persist_corporation(
  db: &Database,
  grant: &Grant,
  corporation_id: i64,
) -> Result<CorporationAdded, clients::Error> {
  let character_id = *grant.character_id();
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

  mod exchange_grant {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

    const CHARACTER_ID: i64 = 42;

    async fn sso_for(server: &MockServer) -> eve_sso::Client {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()))
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

    #[tokio::test]
    async fn it_exchanges_the_code_for_the_grant() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      let sso = sso_for(&server).await;
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = Callback {
        code: "code".to_owned(),
        state: pending.state.clone(),
      };

      let grant = exchange_grant(&sso, &pending, &callback).await.unwrap();

      assert_eq!(*grant.character_id(), CHARACTER_ID);
    }

    #[tokio::test]
    async fn it_rejects_a_mismatched_state() {
      let server = MockServer::start().await;
      let sso = sso_for(&server).await;
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &redirect_uri());
      let callback = Callback {
        code: "code".to_owned(),
        state: "tampered".to_owned(),
      };

      let result = exchange_grant(&sso, &pending, &callback).await;

      assert!(result.is_err());
    }
  }

  mod persist_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    const CHARACTER_ID: i64 = 42;
    const CORPORATION_ID: i64 = 2000;

    fn grant() -> Grant {
      Grant::from_stored("at", CHARACTER_ID, chrono::Utc::now(), "rt", Vec::new())
    }

    #[tokio::test]
    async fn it_persists_the_corporation_credential_authorized_by_the_character() {
      let db = store::open_test().await.unwrap();

      let added = persist_corporation(&db, &grant(), CORPORATION_ID).await.unwrap();

      assert_eq!(added.corporation_id, CORPORATION_ID);
      assert_eq!(added.authorizing_character_id, CHARACTER_ID);
      let credential = infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
        .await
        .unwrap()
        .unwrap();
      assert_eq!(credential.authorized_by(), Some(CHARACTER_ID));
    }

    #[tokio::test]
    async fn it_clears_a_previously_set_needs_reauth_flag_on_success() {
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

      persist_corporation(&db, &grant(), CORPORATION_ID).await.unwrap();

      let credential = infra::get(&db, CORPORATION_ID, OwnerType::Corporation)
        .await
        .unwrap()
        .unwrap();
      assert!(
        !credential.needs_reauth(),
        "a successful corp re-auth must clear the persisted needs-reauth flag"
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
