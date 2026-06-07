use sqlx::FromRow;

#[derive(Clone, Debug, Eq, FromRow, PartialEq)]
pub struct SourceTypeFilter {
  pub category: String,
  pub source_type_id: i64,
  pub source_type_name: String,
}
