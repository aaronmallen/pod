mod deep_link;
mod session;

use std::{collections::BTreeSet, sync::Arc};

use iced::{
  Color, Element, Length, Subscription, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, text},
};
pub use session::{CorporationAdded, SignedIn};

use crate::{
  clients::{esi, esi::scopes, eve_sso},
  config::{FeatureFlags, SubFeature},
  features::shell::registry,
  services::corp_eligibility,
  store::Database,
  sync::Subject,
  ui::{
    components::button::Button,
    style::{color, control, spacing, typography},
  },
};

const PANEL_MAX_WIDTH: f32 = 520.0;

pub enum Event {
  CorporationAdded(CorporationAdded),
  SignedIn(SignedIn),
}

#[derive(Clone, Debug)]
pub enum Message {
  BrowserOpened(Result<(), String>),
  CallbackReceived(String),
  Cancel,
  Completed(Result<SignedIn, String>),
  CorporationCompleted(Result<CorporationAdded, String>),
  Start(FeatureFlags),
  StartAddCorporation(FeatureFlags),
}

#[derive(Debug, Default)]
pub struct State {
  flow: Option<Flow>,
}

impl State {
  pub fn is_active(&self) -> bool {
    self.flow.is_some()
  }
}

#[derive(Debug)]
struct Flow {
  features: FeatureFlags,
  kind: Kind,
  pending: eve_sso::PendingAuth,
  status: Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
  AddCorporation,
  SignIn,
}

#[derive(Debug)]
enum Status {
  Completing,
  Failed(String),
  Waiting,
}

fn sub_corp_scopes(sub: SubFeature) -> impl Iterator<Item = &'static str> {
  registry::sub_descriptor(sub)
    .jobs
    .iter()
    // `gating_scope` matches on the subject variant only, so the id here is an unused placeholder.
    .filter_map(|job| job.gating_scope(Subject::Corporation(0)))
}

fn sub_scopes(sub: SubFeature) -> &'static [&'static str] {
  registry::sub_descriptor(sub).scopes
}

pub fn corp_scopes_for(features: &FeatureFlags) -> Vec<&'static str> {
  scopes::BASELINE_CORP_SCOPES
    .iter()
    .copied()
    .chain(features.enabled_sub_features().into_iter().flat_map(sub_corp_scopes))
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

pub fn scopes_for(features: &FeatureFlags) -> Vec<&'static str> {
  features
    .enabled_sub_features()
    .into_iter()
    .flat_map(|sub| sub_scopes(sub).iter().copied())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

pub fn subscription() -> Subscription<Message> {
  deep_link::subscription().map(Message::CallbackReceived)
}

pub fn focus_subscription() -> Subscription<()> {
  deep_link::focus_subscription()
}

pub fn install() {
  deep_link::install();
}

pub fn forward_or_claim() -> bool {
  deep_link::forward_or_claim()
}

pub fn release_lock() {
  deep_link::release_lock();
}

pub fn update(
  state: &mut State,
  message: Message,
  sso: &Arc<eve_sso::Client>,
  esi: &Arc<esi::Client>,
  db: &Database,
) -> (Task<Message>, Option<Event>) {
  match message {
    Message::Start(features) => {
      let scopes = scopes_for(&features);
      let task = start_flow(state, sso, Kind::SignIn, &scopes, features);
      (task, None)
    }
    Message::StartAddCorporation(features) => {
      let scopes = corp_scopes_for(&features);
      let task = start_flow(state, sso, Kind::AddCorporation, &scopes, features);
      (task, None)
    }
    Message::BrowserOpened(Ok(())) => (Task::none(), None),
    Message::BrowserOpened(Err(error)) => {
      fail_active_flow(
        state,
        format!("Couldn't open your browser ({error}). Open the EVE sign-in page manually to continue."),
      );
      (Task::none(), None)
    }
    Message::CallbackReceived(url) => {
      let Some(flow) = &mut state.flow else {
        return (Task::none(), None);
      };
      let Some(callback) = session::parse_callback(&url) else {
        flow.status = Status::Failed("The sign-in callback was malformed (missing code or state).".to_owned());
        return (Task::none(), None);
      };
      flow.status = Status::Completing;
      let db = db.clone();
      let sso = Arc::clone(sso);
      let pending = flow.pending.clone();
      let task = match flow.kind {
        Kind::AddCorporation => {
          let esi = Arc::clone(esi);
          Task::perform(
            async move { add_corporation(&sso, &esi, &db, &pending, &callback).await },
            Message::CorporationCompleted,
          )
        }
        Kind::SignIn => Task::perform(
          async move {
            session::complete_sign_in(&sso, &db, &pending, &callback)
              .await
              .map_err(|err| err.to_string())
          },
          Message::Completed,
        ),
      };
      (task, None)
    }
    Message::Cancel => {
      state.flow = None;
      (Task::none(), None)
    }
    Message::Completed(Ok(signed)) => {
      tracing::info!(character_id = signed.character_id, name = %signed.character_name, "character signed in");
      state.flow = None;
      (Task::none(), Some(Event::SignedIn(signed)))
    }
    Message::Completed(Err(error)) => {
      fail_active_flow(state, error);
      (Task::none(), None)
    }
    Message::CorporationCompleted(Ok(added)) => {
      tracing::info!(
        corporation_id = added.corporation_id,
        authorizing_character_id = added.authorizing_character_id,
        "corporation added"
      );
      state.flow = None;
      (Task::none(), Some(Event::CorporationAdded(added)))
    }
    Message::CorporationCompleted(Err(error)) => {
      fail_active_flow(state, error);
      (Task::none(), None)
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  match &state.flow {
    Some(flow) => panel(flow),
    None => container(Space::new()).width(Length::Fill).height(Length::Fill).into(),
  }
}

fn panel(flow: &Flow) -> Element<'_, Message> {
  let title = match flow.kind {
    Kind::AddCorporation => "Add a corporation",
    Kind::SignIn => "Add a character",
  };
  let mut children: Vec<Element<'_, Message>> = vec![
    text(title)
      .font(typography::body::REGULAR)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];

  let completing = match flow.kind {
    Kind::AddCorporation => "Verifying your role\u{2026}",
    Kind::SignIn => "Signing you in\u{2026}",
  };
  match &flow.status {
    Status::Waiting => children.push(body_text(
      "A browser window opened to EVE SSO. Authorize there and Pod will finish automatically. You don't need to come back here.",
      color::text::secondary(),
    )),
    Status::Completing => children.push(body_text(completing, color::text::secondary())),
    Status::Failed(error) => children.push(
      text(error)
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
    ),
  }

  let retry = match flow.kind {
    Kind::AddCorporation => Message::StartAddCorporation(flow.features),
    Kind::SignIn => Message::Start(flow.features),
  };
  let mut actions: Vec<Element<'_, Message>> = Vec::new();
  if matches!(flow.status, Status::Failed(_)) {
    actions.push(Button::primary(t!("roster.auth.try_again")).on_press(retry).into());
  }
  actions.push(Button::ghost(t!("roster.auth.cancel")).on_press(Message::Cancel).into());
  children.push(Row::with_children(actions).spacing(spacing::SPACE_3).into());

  let panel = container(Column::with_children(children).spacing(spacing::SPACE_3))
    .max_width(PANEL_MAX_WIDTH)
    .padding(spacing::SPACE_6)
    .style(control::card);

  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

async fn add_corporation(
  sso: &eve_sso::Client,
  esi: &esi::Client,
  db: &Database,
  pending: &eve_sso::PendingAuth,
  callback: &session::Callback,
) -> Result<CorporationAdded, String> {
  let grant = session::exchange_grant(sso, pending, callback)
    .await
    .map_err(|err| err.to_string())?;
  let corporation_id = corp_eligibility::eligible_corporation(esi, &grant, *grant.character_id())
    .await
    .map_err(|err| err.to_string())?;
  session::persist_corporation(db, &grant, corporation_id)
    .await
    .map_err(|err| err.to_string())
}

fn body_text<'a>(content: &'a str, fill: Color) -> Element<'a, Message> {
  text(content)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(fill),
    })
    .into()
}

fn fail_active_flow(state: &mut State, error: String) {
  if let Some(flow) = &mut state.flow {
    flow.status = Status::Failed(error);
  }
}

fn start_flow(
  state: &mut State,
  sso: &eve_sso::Client,
  kind: Kind,
  scopes: &[&str],
  features: FeatureFlags,
) -> Task<Message> {
  let pending = sso.sign_in(scopes, &session::redirect_uri());
  let url = pending.url.clone();
  state.flow = Some(Flow {
    kind,
    pending,
    status: Status::Waiting,
    features,
  });
  // Open the browser from the returned Task (not eagerly), so `update` stays a pure state
  // transition: unit tests that drive `update` never launch a real browser.
  Task::perform(
    async move { open::that_detached(&url).map_err(|err| err.to_string()) },
    Message::BrowserOpened,
  )
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;
  use crate::{
    clients::{esi, eve_sso, http},
    config::Feature,
  };

  fn flags_with(features: &[Feature]) -> FeatureFlags {
    let mut flags = FeatureFlags::default();
    for feature in Feature::ALL {
      flags.set_enabled(feature, features.contains(&feature));
    }
    flags
  }

  fn flags_all() -> FeatureFlags {
    FeatureFlags::default()
  }

  fn flags_none() -> FeatureFlags {
    flags_with(&[])
  }

  impl State {
    fn completing() -> Self {
      State {
        flow: Some(Flow {
          kind: Kind::SignIn,
          pending: pending(),
          status: Status::Completing,
          features: flags_none(),
        }),
      }
    }

    fn waiting() -> Self {
      State::waiting_for(Kind::SignIn)
    }

    fn waiting_for(kind: Kind) -> Self {
      State {
        flow: Some(Flow {
          kind,
          pending: pending(),
          status: Status::Waiting,
          features: flags_none(),
        }),
      }
    }
  }

  fn pending() -> eve_sso::PendingAuth {
    eve_sso::PendingAuth {
      state: "test-state".to_owned(),
      url: "https://example.com/auth".to_owned(),
      verifier: "test-verifier".to_owned(),
    }
  }

  async fn dummy_esi() -> Arc<esi::Client> {
    let db = crate::store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db)).build();
    Arc::new(esi::Client::builder(http).user_agent("test").build().unwrap())
  }

  async fn dummy_sso() -> Arc<eve_sso::Client> {
    let db = crate::store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db)).build();
    Arc::new(eve_sso::Client::new(http, "test-client"))
  }

  mod add_corporation {
    use base64::Engine as _;
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::store;

    const CHARACTER_ID: i64 = 42;

    const CORPORATION_ID: i64 = 2000;

    fn jwt(sub: &str) -> String {
      let encode = |raw: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
      format!(
        "{}.{}.sig",
        encode(r#"{"alg":"RS256","typ":"JWT"}"#),
        encode(&format!(r#"{{"sub":"{sub}","name":"Test Pilot","scp":[]}}"#)),
      )
    }

    async fn clients_for(server: &MockServer) -> (eve_sso::Client, esi::Client) {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      let sso =
        eve_sso::Client::new(Arc::clone(&http), "test-client").with_token_url(format!("{}/token", server.uri()));
      let esi = esi::Client::with_base_url(http, server.uri());
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

    fn callback_for(pending: &eve_sso::PendingAuth) -> session::Callback {
      session::Callback {
        code: "code".to_owned(),
        state: pending.state.clone(),
      }
    }

    #[tokio::test]
    async fn it_persists_the_corporation_for_an_eligible_director() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Director"]"#).await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &session::redirect_uri());
      let callback = callback_for(&pending);

      let added = add_corporation(&sso, &esi, &db, &pending, &callback).await.unwrap();

      assert_eq!(added.corporation_id, CORPORATION_ID);
      assert_eq!(added.authorizing_character_id, CHARACTER_ID);
      assert!(
        crate::store::repo::infra::get(&db, CORPORATION_ID, crate::store::model::OwnerType::Corporation)
          .await
          .unwrap()
          .is_some(),
        "an eligible add must persist the corporation credential"
      );
    }

    #[tokio::test]
    async fn it_short_circuits_on_a_mismatched_state_before_reaching_esi() {
      let server = MockServer::start().await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &session::redirect_uri());
      let callback = session::Callback {
        code: "code".to_owned(),
        state: "tampered".to_owned(),
      };

      let result = add_corporation(&sso, &esi, &db, &pending, &callback).await;

      assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_short_circuits_without_persisting_when_the_character_is_ineligible() {
      let server = MockServer::start().await;
      mount_token(&server).await;
      mount_public_info(&server).await;
      mount_roles(&server, r#"["Accountant"]"#).await;
      mount_corporation_info(&server, 999).await;
      let (sso, esi) = clients_for(&server).await;
      let db = store::open_test().await.unwrap();
      let pending = sso.sign_in(&["esi-characters.read_corporation_roles.v1"], &session::redirect_uri());
      let callback = callback_for(&pending);

      let result = add_corporation(&sso, &esi, &db, &pending, &callback).await;

      assert!(result.is_err());
      assert!(
        crate::store::repo::infra::get(&db, CORPORATION_ID, crate::store::model::OwnerType::Corporation)
          .await
          .unwrap()
          .is_none(),
        "an ineligible character must not persist any corporation credential"
      );
    }
  }

  mod corp_scopes_for {
    use super::*;

    #[test]
    fn it_always_requests_the_baseline_companions() {
      let requested = corp_scopes_for(&flags_none());

      assert!(requested.contains(&scopes::CORPORATION_DIVISIONS));
      assert!(requested.contains(&scopes::CORPORATION_MEMBERS));
      assert!(requested.contains(&scopes::CORPORATION_ROLES));
    }

    #[test]
    fn it_derives_corp_asset_and_wallet_scopes_from_their_features() {
      let requested = corp_scopes_for(&flags_with(&[Feature::AssetTracking, Feature::Wallet]));

      assert!(requested.contains(&scopes::CORPORATION_ASSETS));
      assert!(requested.contains(&scopes::CORPORATION_WALLET));
    }

    #[test]
    fn it_derives_corp_contracts_when_wallet_is_enabled() {
      let requested = corp_scopes_for(&flags_with(&[Feature::Wallet]));

      assert!(
        requested.contains(&scopes::CORPORATION_CONTRACTS),
        "an enabled Wallet feature must request the corp contracts scope, got {requested:?}"
      );
    }

    #[test]
    fn it_derives_corp_industry_jobs_when_industry_is_enabled() {
      let requested = corp_scopes_for(&flags_with(&[Feature::Industry]));

      assert!(
        requested.contains(&scopes::CORPORATION_INDUSTRY_JOBS),
        "an enabled Industry feature must request the corp industry jobs scope, got {requested:?}"
      );
    }

    #[test]
    fn it_derives_corp_mining_extractions_when_industry_is_enabled() {
      let requested = corp_scopes_for(&flags_with(&[Feature::Industry]));

      assert!(
        requested.contains(&scopes::CORPORATION_MINING_EXTRACTIONS),
        "an enabled Industry feature must request the corp mining extractions scope, got {requested:?}"
      );
    }

    #[test]
    fn it_omits_a_disabled_features_corp_scope() {
      let without_industry = corp_scopes_for(&flags_with(&[Feature::Wallet]));

      assert!(!without_industry.contains(&scopes::CORPORATION_INDUSTRY_JOBS));
    }

    #[test]
    fn disabling_all_asset_sub_features_drops_the_corp_asset_scope() {
      let mut flags = flags_with(&[Feature::AssetTracking]);
      assert!(corp_scopes_for(&flags).contains(&scopes::CORPORATION_ASSETS));

      for &sub in Feature::AssetTracking.sub_features() {
        flags.set_sub_enabled(sub, false);
      }

      assert!(
        !corp_scopes_for(&flags).contains(&scopes::CORPORATION_ASSETS),
        "with every asset sub-feature off, the corp asset scope must be dropped"
      );
    }

    #[test]
    fn keeping_one_asset_sub_feature_retains_the_corp_asset_scope() {
      let mut flags = flags_with(&[Feature::AssetTracking]);
      for &sub in Feature::AssetTracking.sub_features() {
        flags.set_sub_enabled(sub, false);
      }
      flags.set_sub_enabled(SubFeature::Inventory, true);

      assert!(
        corp_scopes_for(&flags).contains(&scopes::CORPORATION_ASSETS),
        "one surviving asset sub-feature must keep the shared corp asset scope"
      );
    }

    #[test]
    fn disabling_extractions_drops_the_corp_structures_scope_independently() {
      let mut flags = flags_with(&[Feature::Industry]);
      assert!(corp_scopes_for(&flags).contains(&scopes::CORPORATION_STRUCTURES));

      flags.set_sub_enabled(SubFeature::Extractions, false);

      let requested = corp_scopes_for(&flags);
      assert!(
        !requested.contains(&scopes::CORPORATION_MINING_EXTRACTIONS),
        "disabling Extractions drops the mining-extractions scope"
      );
      assert!(
        requested.contains(&scopes::CORPORATION_INDUSTRY_JOBS),
        "disabling Extractions leaves the distinct job-monitoring scope intact"
      );
    }

    #[test]
    fn the_union_is_deduplicated_and_sorted() {
      let requested = corp_scopes_for(&flags_all());
      let mut sorted = requested.clone();
      sorted.sort_unstable();
      sorted.dedup();

      assert_eq!(requested, sorted, "the union must be deduplicated and ordered");
    }
  }

  mod scopes_for {
    use std::collections::BTreeSet;

    use pretty_assertions::assert_eq;

    use super::*;

    const LEGACY_SIGN_IN_SCOPES: &[&str] = &[
      scopes::CHARACTER_ASSETS,
      scopes::CHARACTER_CLONES,
      scopes::CHARACTER_LOCATION,
      scopes::CHARACTER_ONLINE,
      scopes::CHARACTER_SHIP,
      scopes::CHARACTER_SKILLQUEUE,
      scopes::CHARACTER_SKILLS,
      scopes::CHARACTER_WALLET,
      scopes::UNIVERSE_STRUCTURES,
    ];

    #[test]
    fn a_representative_config_requests_exactly_its_features_union() {
      let flags = flags_with(&[Feature::Wallet, Feature::SkillMonitoring, Feature::LocationTracking]);

      let requested = scopes_for(&flags);

      let expected: Vec<&str> = [
        scopes::CHARACTER_WALLET,
        scopes::CHARACTER_CONTRACTS,
        scopes::CHARACTER_SKILLS,
        scopes::CHARACTER_SKILLQUEUE,
        scopes::CHARACTER_IMPLANTS,
        scopes::CHARACTER_LOCATION,
        scopes::CHARACTER_ONLINE,
        scopes::CHARACTER_SHIP,
        scopes::UNIVERSE_STRUCTURES,
      ]
      .into_iter()
      .collect::<BTreeSet<_>>()
      .into_iter()
      .collect();

      assert_eq!(requested, expected);
      assert!(requested.contains(&scopes::CHARACTER_WALLET));
      assert!(requested.contains(&scopes::CHARACTER_SKILLS));
      assert!(requested.contains(&scopes::CHARACTER_SKILLQUEUE));
      assert!(requested.contains(&scopes::CHARACTER_IMPLANTS));
    }

    #[test]
    fn all_features_on_is_a_superset_of_the_legacy_set() {
      let requested: BTreeSet<&str> = scopes_for(&flags_all()).into_iter().collect();

      for scope in LEGACY_SIGN_IN_SCOPES {
        assert!(
          requested.contains(scope),
          "all-features union must still request {scope}"
        );
      }
    }

    #[test]
    fn disabling_a_feature_drops_its_scopes() {
      let with_mail = scopes_for(&flags_with(&[Feature::Mail, Feature::Wallet]));
      let without_mail = scopes_for(&flags_with(&[Feature::Wallet]));

      assert!(with_mail.contains(&scopes::CHARACTER_MAIL));
      assert!(!without_mail.contains(&scopes::CHARACTER_MAIL));
      assert!(without_mail.contains(&scopes::CHARACTER_WALLET));
    }

    #[test]
    fn disabling_all_asset_sub_features_drops_the_character_asset_scope() {
      let mut flags = flags_with(&[Feature::AssetTracking]);
      assert!(scopes_for(&flags).contains(&scopes::CHARACTER_ASSETS));

      for &sub in Feature::AssetTracking.sub_features() {
        flags.set_sub_enabled(sub, false);
      }

      assert!(
        !scopes_for(&flags).contains(&scopes::CHARACTER_ASSETS),
        "with every asset sub-feature off, the shared character asset scope is dropped"
      );
    }

    #[test]
    fn keeping_one_asset_sub_feature_retains_the_character_asset_scope() {
      let mut flags = flags_with(&[Feature::AssetTracking]);
      for &sub in Feature::AssetTracking.sub_features() {
        flags.set_sub_enabled(sub, false);
      }
      flags.set_sub_enabled(SubFeature::Stockpiles, true);

      assert!(
        scopes_for(&flags).contains(&scopes::CHARACTER_ASSETS),
        "one surviving asset sub-feature keeps the shared character asset scope"
      );
    }

    #[test]
    fn disabling_contracts_drops_its_scope_while_sibling_wallet_subs_stay_on() {
      let mut flags = flags_with(&[Feature::Wallet]);
      assert!(scopes_for(&flags).contains(&scopes::CHARACTER_CONTRACTS));

      flags.set_sub_enabled(SubFeature::Contracts, false);

      let requested = scopes_for(&flags);
      assert!(
        !requested.contains(&scopes::CHARACTER_CONTRACTS),
        "disabling Contracts drops its distinct scope"
      );
      assert!(
        requested.contains(&scopes::CHARACTER_WALLET),
        "the shared wallet scope survives because other wallet sub-features are still on"
      );
    }

    #[test]
    fn enabling_only_transactions_requests_the_wallet_scope_but_not_contracts() {
      let mut flags = flags_with(&[Feature::Wallet]);
      for &sub in Feature::Wallet.sub_features() {
        flags.set_sub_enabled(sub, false);
      }
      flags.set_sub_enabled(SubFeature::Transactions, true);

      let requested = scopes_for(&flags);
      assert!(requested.contains(&scopes::CHARACTER_WALLET));
      assert!(
        !requested.contains(&scopes::CHARACTER_ORDERS),
        "Transactions must not request the market-orders scope (no breaking re-auth)"
      );
      assert!(
        !requested.contains(&scopes::CHARACTER_CONTRACTS),
        "with Contracts off, its scope is gone even though Transactions shares the wallet scope"
      );
    }

    #[test]
    fn industry_sub_features_map_to_distinct_scopes() {
      let mut flags = flags_with(&[Feature::Industry]);
      let full = scopes_for(&flags);
      assert!(full.contains(&scopes::CHARACTER_INDUSTRY_JOBS));
      assert!(full.contains(&scopes::CHARACTER_BLUEPRINTS));
      assert!(full.contains(&scopes::CHARACTER_SEARCH));

      flags.set_sub_enabled(SubFeature::Blueprints, false);
      let without_blueprints = scopes_for(&flags);
      assert!(
        !without_blueprints.contains(&scopes::CHARACTER_BLUEPRINTS),
        "disabling Blueprints drops only its scope"
      );
      assert!(
        without_blueprints.contains(&scopes::CHARACTER_INDUSTRY_JOBS),
        "Job Monitoring keeps its distinct scope when Blueprints is off"
      );

      flags.set_sub_enabled(SubFeature::Planner, false);
      assert!(
        !scopes_for(&flags).contains(&scopes::CHARACTER_SEARCH),
        "disabling Planner drops the facility-search scope independently"
      );
    }

    #[test]
    fn each_feature_maps_to_a_nonempty_scope_set() {
      for feature in Feature::ALL {
        assert!(
          !scopes_for(&flags_with(&[feature])).is_empty(),
          "{feature:?} must map to at least one scope"
        );
      }
    }

    #[test]
    fn mail_requests_the_search_scope_for_recipient_lookup() {
      let mail = scopes_for(&flags_with(&[Feature::Mail]));

      assert!(mail.contains(&scopes::CHARACTER_SEARCH));
    }

    #[test]
    fn mail_requests_the_send_and_organize_scopes() {
      let mail = scopes_for(&flags_with(&[Feature::Mail]));

      assert!(mail.contains(&scopes::CHARACTER_MAIL));
      assert!(mail.contains(&scopes::CHARACTER_MAIL_SEND));
      assert!(mail.contains(&scopes::CHARACTER_MAIL_ORGANIZE));
    }

    #[test]
    fn no_features_requests_no_scopes() {
      assert!(scopes_for(&flags_none()).is_empty());
    }

    #[test]
    fn the_union_is_deduplicated_and_sorted() {
      let requested = scopes_for(&flags_all());
      let mut sorted = requested.clone();
      sorted.sort_unstable();
      sorted.dedup();

      assert_eq!(requested, sorted, "the union must be deduplicated and ordered");
    }
  }

  mod update {
    use super::*;

    #[tokio::test]
    async fn a_valid_add_corporation_callback_moves_the_flow_to_completing() {
      let mut state = State::waiting_for(Kind::AddCorporation);
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(
        &mut state,
        Message::CallbackReceived("eveauth-pod://callback?code=abc&state=test-state".to_owned()),
        &sso,
        &esi,
        &db,
      );

      assert!(matches!(
        &state.flow,
        Some(Flow {
          kind: Kind::AddCorporation,
          status: Status::Completing,
          ..
        })
      ));
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn a_valid_sign_in_callback_moves_the_flow_to_completing() {
      let mut state = State::waiting();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(
        &mut state,
        Message::CallbackReceived("eveauth-pod://callback?code=abc&state=test-state".to_owned()),
        &sso,
        &esi,
        &db,
      );

      assert!(matches!(
        &state.flow,
        Some(Flow {
          status: Status::Completing,
          ..
        })
      ));
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn callback_with_a_malformed_url_fails_the_flow() {
      let mut state = State::waiting();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(
        &mut state,
        Message::CallbackReceived("eveauth-pod://callback".to_owned()),
        &sso,
        &esi,
        &db,
      );

      assert!(matches!(
        &state.flow,
        Some(Flow {
          status: Status::Failed(_),
          ..
        })
      ));
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn callback_without_a_flow_is_a_noop() {
      let mut state = State::default();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(
        &mut state,
        Message::CallbackReceived("eveauth-pod://callback?code=a&state=b".to_owned()),
        &sso,
        &esi,
        &db,
      );

      assert!(!state.is_active());
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn cancel_clears_the_flow() {
      let mut state = State::waiting();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(&mut state, Message::Cancel, &sso, &esi, &db);

      assert!(!state.is_active());
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn completed_err_fails_an_active_flow() {
      let mut state = State::completing();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(&mut state, Message::Completed(Err("boom".to_owned())), &sso, &esi, &db);

      assert!(matches!(
        &state.flow,
        Some(Flow {
          status: Status::Failed(_),
          ..
        })
      ));
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn completed_err_without_a_flow_is_dropped() {
      let mut state = State::default();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(&mut state, Message::Completed(Err("boom".to_owned())), &sso, &esi, &db);

      assert!(!state.is_active());
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn completed_ok_emits_signed_in_and_clears_the_flow() {
      let mut state = State::completing();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();
      let signed = SignedIn {
        character_id: 42,
        character_name: "Pilot".to_owned(),
      };

      let (_task, event) = update(&mut state, Message::Completed(Ok(signed)), &sso, &esi, &db);

      assert!(!state.is_active());
      assert!(matches!(event, Some(Event::SignedIn(s)) if s.character_id == 42));
    }

    #[tokio::test]
    async fn corporation_completed_err_fails_an_active_flow() {
      let mut state = State::waiting_for(Kind::AddCorporation);
      state.flow.as_mut().unwrap().status = Status::Completing;
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(
        &mut state,
        Message::CorporationCompleted(Err("nope".to_owned())),
        &sso,
        &esi,
        &db,
      );

      assert!(matches!(
        &state.flow,
        Some(Flow {
          status: Status::Failed(_),
          ..
        })
      ));
      assert!(event.is_none());
    }

    #[tokio::test]
    async fn corporation_completed_ok_emits_corporation_added_and_clears_the_flow() {
      let mut state = State::waiting_for(Kind::AddCorporation);
      state.flow.as_mut().unwrap().status = Status::Completing;
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();
      let added = CorporationAdded {
        authorizing_character_id: 42,
        corporation_id: 2000,
      };

      let (_task, event) = update(&mut state, Message::CorporationCompleted(Ok(added)), &sso, &esi, &db);

      assert!(!state.is_active());
      assert!(matches!(event, Some(Event::CorporationAdded(c)) if c.corporation_id == 2000));
    }

    #[tokio::test]
    async fn start_add_corporation_records_the_supplied_features() {
      let mut state = State::default();
      let sso = dummy_sso().await;
      let esi = dummy_esi().await;
      let db = crate::store::open_test().await.unwrap();

      let (_task, event) = update(
        &mut state,
        Message::StartAddCorporation(flags_with(&[Feature::Industry])),
        &sso,
        &esi,
        &db,
      );

      let flow = state.flow.as_ref().expect("a corp flow should be active");
      assert_eq!(flow.kind, Kind::AddCorporation);
      assert_eq!(flow.features, flags_with(&[Feature::Industry]));
      assert!(event.is_none());
    }
  }

  #[test]
  fn view_renders_each_status_without_panicking() {
    let idle = State::default();
    let waiting = State::waiting();
    let completing = State::completing();
    let corp_waiting = State::waiting_for(Kind::AddCorporation);
    let failed = State {
      flow: Some(Flow {
        kind: Kind::SignIn,
        pending: pending(),
        status: Status::Failed("nope".to_owned()),
        features: flags_none(),
      }),
    };

    let _idle = view(&idle);
    let _waiting = view(&waiting);
    let _completing = view(&completing);
    let _corp_waiting = view(&corp_waiting);
    let _failed = view(&failed);
  }
}
