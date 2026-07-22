use sqlx::FromRow;

#[derive(Clone, Debug, FromRow)]
pub struct CharacterFinancials {
  pub asset_value: Option<f64>,
  pub character_id: i64,
  pub escrow: Option<f64>,
  pub liquid: Option<f64>,
  pub net_worth: Option<f64>,
}
