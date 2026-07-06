use getset::Getters;
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub blocked: Option<String>,
  #[getset(get = "pub")]
  pub build: Option<String>,
  #[getset(get = "pub")]
  pub combat: Option<String>,
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get = "pub")]
  pub date: String,
  #[getset(get = "pub")]
  pub goal: Option<String>,
  #[getset(get = "pub")]
  pub marked_complete: bool,
  #[getset(get = "pub")]
  pub narrative: Option<String>,
  #[getset(get = "pub")]
  pub next: Option<String>,
  #[getset(get = "pub")]
  pub remember: Option<String>,
  #[getset(get = "pub")]
  pub research: Option<String>,
  #[getset(get = "pub")]
  pub skill: Option<String>,
  #[getset(get = "pub")]
  pub updated_at: String,
}
