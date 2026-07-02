use sqlx::FromRow;

/// Up to three rig `type_id`s installed in `facility_id`'s slots, `None` for an empty slot.
///
/// `name`/`solar_system_id`/`type_id` snapshot the facility's display identity onto the row so intel
/// still renders once the facility is no longer accessible; all three are `None` together when the
/// facility could not be identified.
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct FacilityIntel {
  pub facility_id: i64,
  pub name: Option<String>,
  pub rig_1_type_id: Option<i64>,
  pub rig_2_type_id: Option<i64>,
  pub rig_3_type_id: Option<i64>,
  pub solar_system_id: Option<i64>,
  pub type_id: Option<i64>,
}
