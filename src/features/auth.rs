mod deep_link;
mod session;

use std::{collections::BTreeSet, sync::Arc};

use iced::{
  Color, Element, Length, Subscription, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};
pub use session::{CorporationAdded, SignedIn};

use crate::{
  clients::{esi, esi::scopes, eve_sso},
  config::Feature,
  store::Database,
  ui::style::{color, control, spacing, typography},
};

const PANEL_MAX_WIDTH: f32 = 520.0;

pub enum Event {
  CorporationAdded(CorporationAdded),
  SignedIn(SignedIn),
}

#[derive(Clone, Debug)]
pub enum Message {
  CallbackReceived(String),
  Cancel,
  Completed(Result<SignedIn, String>),
  CorporationCompleted(Result<CorporationAdded, String>),
  Start(Vec<Feature>),
  StartAddCorporation,
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
  features: Vec<Feature>,
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

fn feature_scopes(feature: Feature) -> &'static [&'static str] {
  match feature {
    Feature::AssetTracking => &[scopes::CHARACTER_ASSETS],
    Feature::CloneMonitoring => &[scopes::CHARACTER_CLONES],
    Feature::CombatLog => &[scopes::CHARACTER_KILLMAILS],
    Feature::Contacts => &[scopes::CHARACTER_CONTACTS],
    Feature::EveNotifications => &[scopes::CHARACTER_NOTIFICATIONS],
    Feature::LocationTracking => &[
      scopes::CHARACTER_LOCATION,
      scopes::CHARACTER_ONLINE,
      scopes::CHARACTER_SHIP,
      scopes::UNIVERSE_STRUCTURES,
    ],
    Feature::Mail => &[
      scopes::CHARACTER_MAIL,
      scopes::CHARACTER_MAIL_SEND,
      scopes::CHARACTER_MAIL_ORGANIZE,
      scopes::CHARACTER_SEARCH,
    ],
    Feature::SkillMonitoring => &[
      scopes::CHARACTER_SKILLS,
      scopes::CHARACTER_SKILLQUEUE,
      scopes::CHARACTER_IMPLANTS,
    ],
    Feature::Standings => &[scopes::CHARACTER_STANDINGS],
    Feature::Wallet => &[scopes::CHARACTER_WALLET, scopes::CHARACTER_CONTRACTS],
  }
}

pub fn scopes_for(features: &[Feature]) -> Vec<&'static str> {
  features
    .iter()
    .flat_map(|&feature| feature_scopes(feature).iter().copied())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

pub fn subscription() -> Subscription<Message> {
  deep_link::subscription().map(Message::CallbackReceived)
}

pub fn install() {
  deep_link::install();
}

pub fn forward_or_claim() -> bool {
  deep_link::forward_or_claim()
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
      start_flow(state, sso, Kind::SignIn, &scopes, features);
      (Task::none(), None)
    }
    Message::StartAddCorporation => {
      start_flow(
        state,
        sso,
        Kind::AddCorporation,
        scopes::CORP_SIGN_IN_SCOPES,
        Vec::new(),
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
            async move {
              session::complete_add_corporation(&sso, &esi, &db, &pending, &callback)
                .await
                .map_err(|err| err.to_string())
            },
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
      "A browser window opened to EVE SSO. Authorize there and Pod will finish automatically — you don't need to come back here.",
      color::text::SECONDARY,
    )),
    Status::Completing => children.push(body_text(completing, color::text::SECONDARY)),
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
    Kind::AddCorporation => Message::StartAddCorporation,
    Kind::SignIn => Message::Start(flow.features.clone()),
  };
  let mut actions: Vec<Element<'_, Message>> = Vec::new();
  if matches!(flow.status, Status::Failed(_)) {
    actions.push(
      button(text("Try again").size(typography::size::MD))
        .padding(control::padding())
        .on_press(retry)
        .style(control::primary_button)
        .into(),
    );
  }
  actions.push(
    button(text("Cancel").size(typography::size::MD))
      .padding(control::padding())
      .on_press(Message::Cancel)
      .style(control::ghost_button)
      .into(),
  );
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

fn start_flow(state: &mut State, sso: &eve_sso::Client, kind: Kind, scopes: &[&str], features: Vec<Feature>) {
  let pending = sso.sign_in(scopes, &session::redirect_uri());
  let status = match open::that_detached(&pending.url) {
    Ok(()) => Status::Waiting,
    Err(err) => Status::Failed(format!(
      "Couldn't open your browser ({err}). Open the EVE sign-in page manually to continue."
    )),
  };
  state.flow = Some(Flow {
    kind,
    pending,
    status,
    features,
  });
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;
  use crate::clients::{esi, eve_sso, http};

  impl State {
    fn completing() -> Self {
      State {
        flow: Some(Flow {
          kind: Kind::SignIn,
          pending: pending(),
          status: Status::Completing,
          features: Vec::new(),
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
          features: Vec::new(),
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
        features: Vec::new(),
      }),
    };

    let _idle = view(&idle);
    let _waiting = view(&waiting);
    let _completing = view(&completing);
    let _corp_waiting = view(&corp_waiting);
    let _failed = view(&failed);
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
    fn no_features_requests_no_scopes() {
      assert!(scopes_for(&[]).is_empty());
    }

    #[test]
    fn all_features_on_is_a_superset_of_the_legacy_set() {
      let requested: BTreeSet<&str> = scopes_for(&Feature::ALL).into_iter().collect();

      for scope in LEGACY_SIGN_IN_SCOPES {
        assert!(
          requested.contains(scope),
          "all-features union must still request {scope}"
        );
      }
    }

    #[test]
    fn the_union_is_deduplicated_and_sorted() {
      let requested = scopes_for(&Feature::ALL);
      let mut sorted = requested.clone();
      sorted.sort_unstable();
      sorted.dedup();

      assert_eq!(requested, sorted, "the union must be deduplicated and ordered");
    }

    #[test]
    fn a_representative_config_requests_exactly_its_features_union() {
      let features = [Feature::Wallet, Feature::SkillMonitoring, Feature::LocationTracking];

      let requested = scopes_for(&features);

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
    fn disabling_a_feature_drops_its_scopes() {
      let with_mail = scopes_for(&[Feature::Mail, Feature::Wallet]);
      let without_mail = scopes_for(&[Feature::Wallet]);

      assert!(with_mail.contains(&scopes::CHARACTER_MAIL));
      assert!(!without_mail.contains(&scopes::CHARACTER_MAIL));
      assert!(without_mail.contains(&scopes::CHARACTER_WALLET));
    }

    #[test]
    fn mail_requests_the_send_and_organize_scopes() {
      let mail = scopes_for(&[Feature::Mail]);

      assert!(mail.contains(&scopes::CHARACTER_MAIL));
      assert!(mail.contains(&scopes::CHARACTER_MAIL_SEND));
      assert!(mail.contains(&scopes::CHARACTER_MAIL_ORGANIZE));
    }

    #[test]
    fn mail_requests_the_search_scope_for_recipient_lookup() {
      let mail = scopes_for(&[Feature::Mail]);

      assert!(mail.contains(&scopes::CHARACTER_SEARCH));
    }

    #[test]
    fn each_feature_maps_to_a_nonempty_scope_set() {
      for feature in Feature::ALL {
        assert!(
          !feature_scopes(feature).is_empty(),
          "{feature:?} must map to at least one scope"
        );
      }
    }
  }

  mod update {
    use super::*;

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
  }
}
