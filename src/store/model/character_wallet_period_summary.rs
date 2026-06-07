use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, FromRow)]
pub struct CharacterWalletPeriodSummary {
  pub character_id: i64,
  pub income: f64,
  pub net: f64,
  pub period: String,
  pub spend: f64,
}
