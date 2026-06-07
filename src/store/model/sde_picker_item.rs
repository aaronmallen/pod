#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickerItem {
  pub group_name: String,
  pub id: i64,
  pub mastery_cert_ids: Vec<Vec<i64>>,
  pub name: String,
  pub skill_requirements: Vec<(String, u8)>,
}
