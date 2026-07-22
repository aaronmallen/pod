use sqlx::FromRow;

#[derive(Clone, Debug, PartialEq)]
pub struct CardRow {
  pub alliance_name: Option<String>,
  pub alliance_ticker: Option<String>,
  pub ceo_name: Option<String>,
  pub corporation_id: i64,
  pub hq_name: Option<String>,
  pub member_count: i64,
  pub name: String,
  pub tags: Vec<CardTag>,
  pub tax_rate: f64,
  pub ticker: String,
}

#[derive(FromRow)]
pub struct CardRowSql {
  pub alliance_name: Option<String>,
  pub alliance_ticker: Option<String>,
  pub ceo_name: Option<String>,
  pub corporation_id: i64,
  pub hq_name: Option<String>,
  pub member_count: i64,
  pub name: String,
  pub tax_rate: f64,
  pub ticker: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CardTag {
  pub color_hex: Option<String>,
  pub id: i64,
  pub name: String,
}

#[derive(FromRow)]
pub struct TagRowSql {
  pub color: Option<String>,
  pub corporation_id: i64,
  pub id: i64,
  pub name: String,
}
