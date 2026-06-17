use sqlx::FromRow;

#[derive(Clone, Debug, PartialEq)]
pub struct CardRow {
  pub character_id: i64,
  pub corp_ticker: Option<String>,
  pub corporation_id: i64,
  pub docked: Option<bool>,
  pub location: Option<String>,
  pub name: String,
  pub position: Option<i64>,
  pub squad_accent_hex: Option<String>,
  pub tags: Vec<CardTag>,
  pub total_sp: Option<i64>,
  pub training: Option<CardTraining>,
  pub wallet_balance: Option<f64>,
}

#[derive(FromRow)]
pub struct CardRowSql {
  pub character_id: i64,
  pub corp_ticker: Option<String>,
  pub corporation_id: i64,
  pub docked: Option<bool>,
  pub location: Option<String>,
  pub name: String,
  pub position: Option<i64>,
  pub squad_accent_hex: Option<String>,
  pub total_sp: Option<i64>,
  pub training_finish_date: Option<String>,
  pub training_finished_level: Option<i64>,
  pub training_skill_id: Option<i64>,
  pub training_skill_name: Option<String>,
  pub training_start_date: Option<String>,
  pub wallet_balance: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CardTag {
  pub color_hex: Option<String>,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CardTraining {
  pub finish_date: Option<String>,
  pub finished_level: i64,
  pub skill_id: i64,
  pub skill_name: Option<String>,
  pub start_date: Option<String>,
}

#[derive(FromRow)]
pub struct TagRowSql {
  pub character_id: i64,
  pub color: Option<String>,
  pub id: i64,
  pub name: String,
}
