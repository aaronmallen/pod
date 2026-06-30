use sqlx::FromRow;

/// The industry rigs fitted to a facility: up to three rig `type_id`s installed in `facility_id`'s
/// slots, with `None` for an empty slot.
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct FacilityIntel {
  pub facility_id: i64,
  pub rig_1_type_id: Option<i64>,
  pub rig_2_type_id: Option<i64>,
  pub rig_3_type_id: Option<i64>,
}
