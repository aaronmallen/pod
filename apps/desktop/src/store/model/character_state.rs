use sqlx::FromRow;

// Public store API exercised by unit tests; not yet wired into a production call site.
#[expect(dead_code)]
#[derive(Clone, Debug, FromRow)]
pub struct CharacterState {
  pub character_id: i64,
  pub online: Option<bool>,
  pub ship_item_id: Option<i64>,
  pub ship_name: Option<String>,
  pub ship_type_id: Option<i64>,
  pub solar_system_id: Option<i64>,
  pub station_id: Option<i64>,
  pub structure_id: Option<i64>,
  pub synced_at: Option<i64>,
  pub total_sp: Option<i64>,
  pub wallet_balance: Option<f64>,
}
