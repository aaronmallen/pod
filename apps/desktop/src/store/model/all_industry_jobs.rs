use crate::store::model::{CharacterIndustryJob, CorporationIndustryJob};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AllIndustryJobs {
  pub character_jobs: Vec<CharacterIndustryJob>,
  pub corporation_jobs: Vec<CorporationIndustryJob>,
}
