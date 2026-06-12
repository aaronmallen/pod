use std::{fmt, future::Future, pin::Pin, str::FromStr};

use crate::{
  clients::{self, esi, eve_sso::Grant},
  store::Database,
};

pub type HandlerFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait KindHandler: Send + Sync {
  fn kind(&self) -> OutboxKind;

  fn apply<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>>;

  fn execute<'a>(
    &'a self,
    esi: &'a esi::Client,
    grant: &'a Grant,
    payload: &'a str,
  ) -> HandlerFuture<'a, Result<(), clients::Error>>;

  fn compensate<'a>(&'a self, db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>>;
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OutboxKind {
  CalendarRespond,
  MailDelete,
  MailSend,
  MailSetLabels,
  MailSetRead,
}

impl OutboxKind {
  pub const ALL: &'static [OutboxKind] = &[
    OutboxKind::MailSend,
    OutboxKind::MailSetRead,
    OutboxKind::MailSetLabels,
    OutboxKind::MailDelete,
    OutboxKind::CalendarRespond,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::CalendarRespond => "calendar.respond",
      Self::MailDelete => "mail.delete",
      Self::MailSend => "mail.send",
      Self::MailSetLabels => "mail.set_labels",
      Self::MailSetRead => "mail.set_read",
    }
  }
}

impl fmt::Display for OutboxKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown outbox kind: {0:?}")]
pub struct ParseKindError(pub String);

impl FromStr for OutboxKind {
  type Err = ParseKindError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "calendar.respond" => Ok(Self::CalendarRespond),
      "mail.delete" => Ok(Self::MailDelete),
      "mail.send" => Ok(Self::MailSend),
      "mail.set_labels" => Ok(Self::MailSetLabels),
      "mail.set_read" => Ok(Self::MailSetRead),
      other => Err(ParseKindError(other.to_string())),
    }
  }
}

#[derive(Default)]
pub struct Registry {
  handlers: Vec<Box<dyn KindHandler>>,
}

impl Registry {
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub fn extend(mut self, other: Registry) -> Self {
    for handler in other.handlers {
      self = self.with(handler);
    }
    self
  }

  #[must_use]
  pub fn with(mut self, handler: Box<dyn KindHandler>) -> Self {
    let kind = handler.kind();
    if let Some(slot) = self.handlers.iter_mut().find(|h| h.kind() == kind) {
      *slot = handler;
    } else {
      self.handlers.push(handler);
    }
    self
  }

  pub fn handler(&self, kind: OutboxKind) -> Option<&dyn KindHandler> {
    self.handlers.iter().find(|h| h.kind() == kind).map(AsRef::as_ref)
  }

  pub fn resolve(&self, kind: &str) -> Result<&dyn KindHandler, ResolveError> {
    let parsed = kind.parse::<OutboxKind>()?;
    self.handler(parsed).ok_or(ResolveError::Unregistered(parsed))
  }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ResolveError {
  #[error(transparent)]
  Unknown(#[from] ParseKindError),
  #[error("no handler registered for outbox kind: {0}")]
  Unregistered(OutboxKind),
}

#[cfg(test)]
pub mod test_support {

  use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
  };

  use super::*;

  #[derive(Clone, Default)]
  pub struct StubCalls {
    inner: Arc<StubCallsInner>,
  }

  #[derive(Default)]
  struct StubCallsInner {
    apply: AtomicUsize,
    compensate: AtomicUsize,
    execute: AtomicUsize,
    payloads: Mutex<Vec<String>>,
  }

  impl StubCalls {
    pub fn applies(&self) -> usize {
      self.inner.apply.load(Ordering::SeqCst)
    }

    pub fn compensates(&self) -> usize {
      self.inner.compensate.load(Ordering::SeqCst)
    }

    pub fn executes(&self) -> usize {
      self.inner.execute.load(Ordering::SeqCst)
    }

    pub fn payloads(&self) -> Vec<String> {
      self.inner.payloads.lock().unwrap().clone()
    }
  }

  pub struct StubHandler {
    calls: StubCalls,
    execute_result: Box<dyn Fn() -> Result<(), clients::Error> + Send + Sync>,
    kind: OutboxKind,
  }

  impl StubHandler {
    pub fn new(kind: OutboxKind, calls: StubCalls) -> Self {
      Self {
        calls,
        execute_result: Box::new(|| Ok(())),
        kind,
      }
    }

    pub fn failing_execute(mut self, make: impl Fn() -> clients::Error + Send + Sync + 'static) -> Self {
      self.execute_result = Box::new(move || Err(make()));
      self
    }

    fn record(&self, payload: &str) {
      self.calls.inner.payloads.lock().unwrap().push(payload.to_string());
    }
  }

  impl KindHandler for StubHandler {
    fn kind(&self) -> OutboxKind {
      self.kind
    }

    fn apply<'a>(&'a self, _db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
      self.calls.inner.apply.fetch_add(1, Ordering::SeqCst);
      self.record(payload);
      Box::pin(async { Ok(()) })
    }

    fn execute<'a>(
      &'a self,
      _esi: &'a esi::Client,
      _grant: &'a Grant,
      payload: &'a str,
    ) -> HandlerFuture<'a, Result<(), clients::Error>> {
      self.calls.inner.execute.fetch_add(1, Ordering::SeqCst);
      self.record(payload);
      let result = (self.execute_result)();
      Box::pin(async move { result })
    }

    fn compensate<'a>(&'a self, _db: &'a Database, payload: &'a str) -> HandlerFuture<'a, Result<(), clients::Error>> {
      self.calls.inner.compensate.fetch_add(1, Ordering::SeqCst);
      self.record(payload);
      Box::pin(async { Ok(()) })
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{test_support::*, *};
  use crate::store;

  mod kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_kind_through_its_string() {
      for &kind in OutboxKind::ALL {
        assert_eq!(kind.as_str().parse::<OutboxKind>(), Ok(kind));
      }
    }

    #[test]
    fn its_strings_match_the_spec_and_migration_set() {
      assert_eq!(
        OutboxKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
        [
          "mail.send",
          "mail.set_read",
          "mail.set_labels",
          "mail.delete",
          "calendar.respond"
        ]
      );
    }

    #[test]
    fn it_rejects_an_unknown_kind_explicitly() {
      assert_eq!(
        "mail.archive".parse::<OutboxKind>(),
        Err(ParseKindError("mail.archive".to_string()))
      );
    }

    #[test]
    fn it_displays_as_its_discriminant_string() {
      assert_eq!(OutboxKind::MailSetRead.to_string(), "mail.set_read");
    }
  }

  mod registry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_registered_handler_by_kind() {
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, StubCalls::default())));

      let handler = registry.handler(OutboxKind::MailSend).expect("handler registered");

      assert_eq!(handler.kind(), OutboxKind::MailSend);
    }

    #[test]
    fn it_returns_none_for_an_unregistered_kind() {
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, StubCalls::default())));

      assert!(registry.handler(OutboxKind::MailDelete).is_none());
    }

    #[test]
    fn it_resolves_a_raw_kind_string_to_its_handler() {
      let registry = Registry::new().with(Box::new(StubHandler::new(
        OutboxKind::MailSetRead,
        StubCalls::default(),
      )));

      let handler = registry.resolve("mail.set_read").expect("resolved");

      assert_eq!(handler.kind(), OutboxKind::MailSetRead);
    }

    #[test]
    fn it_reports_an_unknown_kind_string_distinctly() {
      let registry = Registry::new();

      let error = registry.resolve("mail.archive").err().expect("unknown kind");

      assert_eq!(error, ResolveError::Unknown(ParseKindError("mail.archive".to_string())));
    }

    #[test]
    fn it_reports_a_known_kind_with_no_handler_distinctly() {
      let registry = Registry::new();

      let error = registry.resolve("mail.send").err().expect("no handler");

      assert_eq!(error, ResolveError::Unregistered(OutboxKind::MailSend));
    }

    #[test]
    fn it_folds_another_registrys_handlers_in_via_extend() {
      let registry = Registry::new()
        .with(Box::new(StubHandler::new(OutboxKind::MailSend, StubCalls::default())))
        .extend(Registry::new().with(Box::new(StubHandler::new(
          OutboxKind::CalendarRespond,
          StubCalls::default(),
        ))));

      assert!(registry.handler(OutboxKind::MailSend).is_some());
      assert!(registry.handler(OutboxKind::CalendarRespond).is_some());
    }

    #[tokio::test]
    async fn it_replaces_an_earlier_handler_for_the_same_kind() {
      let first = StubCalls::default();
      let second = StubCalls::default();
      let registry = Registry::new()
        .with(Box::new(StubHandler::new(OutboxKind::MailSend, first.clone())))
        .with(Box::new(StubHandler::new(OutboxKind::MailSend, second.clone())));

      let db = store::open_test().await.unwrap();
      registry
        .handler(OutboxKind::MailSend)
        .unwrap()
        .apply(&db, "{}")
        .await
        .unwrap();

      assert_eq!(first.applies(), 0);
      assert_eq!(second.applies(), 1);
    }
  }

  mod dispatch {
    use pretty_assertions::assert_eq;

    use super::*;

    fn registry(kind: OutboxKind, calls: StubCalls) -> Registry {
      Registry::new().with(Box::new(StubHandler::new(kind, calls)))
    }

    #[tokio::test]
    async fn it_drives_the_optimistic_apply_with_the_payload() {
      let calls = StubCalls::default();
      let registry = registry(OutboxKind::MailSetRead, calls.clone());
      let db = store::open_test().await.unwrap();

      registry
        .handler(OutboxKind::MailSetRead)
        .unwrap()
        .apply(&db, "{\"mail_id\":1}")
        .await
        .unwrap();

      assert_eq!(calls.applies(), 1);
      assert_eq!(calls.payloads(), ["{\"mail_id\":1}"]);
    }

    #[tokio::test]
    async fn it_drives_a_successful_esi_execute() {
      let calls = StubCalls::default();
      let registry = registry(OutboxKind::MailSend, calls.clone());
      let db = store::open_test().await.unwrap();
      let http = crate::clients::http::Client::builder(crate::clients::http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http, "http://localhost");
      let grant = Grant::new_test("tok", 42);

      registry
        .handler(OutboxKind::MailSend)
        .unwrap()
        .execute(&esi, &grant, "{}")
        .await
        .unwrap();

      assert_eq!(calls.executes(), 1);
    }

    #[tokio::test]
    async fn it_surfaces_a_primed_execute_error_for_the_drainer_to_classify() {
      let calls = StubCalls::default();
      let handler =
        StubHandler::new(OutboxKind::MailSend, calls.clone()).failing_execute(|| clients::Error::RateLimit {
          retry_after_secs: 30,
        });
      let registry = Registry::new().with(Box::new(handler));
      let db = store::open_test().await.unwrap();
      let http = crate::clients::http::Client::builder(crate::clients::http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http, "http://localhost");
      let grant = Grant::new_test("tok", 42);

      let error = registry
        .handler(OutboxKind::MailSend)
        .unwrap()
        .execute(&esi, &grant, "{}")
        .await
        .expect_err("primed to fail");

      assert!(matches!(
        error,
        clients::Error::RateLimit {
          retry_after_secs: 30
        }
      ));
    }

    #[tokio::test]
    async fn it_drives_compensation_on_permanent_failure() {
      let calls = StubCalls::default();
      let registry = registry(OutboxKind::MailDelete, calls.clone());
      let db = store::open_test().await.unwrap();

      registry
        .handler(OutboxKind::MailDelete)
        .unwrap()
        .compensate(&db, "{\"mail_id\":7}")
        .await
        .unwrap();

      assert_eq!(calls.compensates(), 1);
      assert_eq!(calls.payloads(), ["{\"mail_id\":7}"]);
    }
  }
}
