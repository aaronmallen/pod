use sqlx::FromRow;

#[derive(Clone, Debug, FromRow)]
pub struct CharacterWalletPeriodSummary {
  pub character_id: i64,
  pub income: f64,
  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[cfg_attr(not(test), expect(dead_code))]
  pub net: f64,
  // Public store API exercised by unit tests; not yet wired into a production call site.
  #[cfg_attr(not(test), expect(dead_code))]
  pub period: String,
  pub spend: f64,
}
