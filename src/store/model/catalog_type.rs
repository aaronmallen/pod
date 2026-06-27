use sqlx::FromRow;

/// An `item_types` row joined to group and category names in one scan; LEFT JOIN ensures types with no group
/// still appear with blank group/category strings rather than being silently excluded.
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct CatalogType {
  pub category_name: String,
  pub group_name: String,
  pub id: i64,
  pub name: String,
  pub packaged_volume: Option<f64>,
  pub volume: Option<f64>,
}
