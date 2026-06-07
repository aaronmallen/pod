use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::corporation::CorporationInfo;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  alliance_id: Option<i64>,
  #[getset(get_copy = "pub")]
  ceo_id: Option<i64>,
  #[getset(get = "pub")]
  creation_date: Option<String>,
  #[getset(get_copy = "pub")]
  creator_id: Option<i64>,
  #[getset(get = "pub")]
  description: Option<String>,
  #[getset(get_copy = "pub")]
  faction_id: Option<i64>,
  #[getset(get_copy = "pub")]
  home_station_id: Option<i64>,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get_copy = "pub")]
  member_count: Option<i32>,
  #[getset(get = "pub")]
  name: String,
  #[getset(get_copy = "pub")]
  shares: Option<i64>,
  #[getset(get_copy = "pub")]
  tax_rate: Option<f64>,
  #[getset(get = "pub")]
  ticker: String,
  #[getset(get = "pub")]
  url: Option<String>,
  #[getset(get_copy = "pub")]
  war_eligible: Option<bool>,
}

impl Model {
  pub fn new(id: i64, name: impl Into<String>, ticker: impl Into<String>) -> Self {
    Self {
      alliance_id: None,
      ceo_id: None,
      creation_date: None,
      creator_id: None,
      description: None,
      faction_id: None,
      home_station_id: None,
      id,
      member_count: None,
      name: name.into(),
      shares: None,
      tax_rate: None,
      ticker: ticker.into(),
      url: None,
      war_eligible: None,
    }
  }

  pub fn set_alliance_id(&mut self, id: i64) {
    self.alliance_id = Some(id);
  }

  pub fn set_ceo_id(&mut self, id: i64) {
    self.ceo_id = Some(id);
  }

  pub fn set_creation_date(&mut self, date: impl Into<String>) {
    self.creation_date = Some(date.into());
  }

  pub fn set_creator_id(&mut self, id: i64) {
    self.creator_id = Some(id);
  }

  pub fn set_description(&mut self, description: impl Into<String>) {
    self.description = Some(description.into());
  }

  pub fn set_faction_id(&mut self, id: i64) {
    self.faction_id = Some(id);
  }

  pub fn set_home_station_id(&mut self, id: i64) {
    self.home_station_id = Some(id);
  }

  pub fn set_member_count(&mut self, count: i32) {
    self.member_count = Some(count);
  }

  pub fn set_shares(&mut self, shares: i64) {
    self.shares = Some(shares);
  }

  pub fn set_tax_rate(&mut self, rate: f64) {
    self.tax_rate = Some(rate);
  }

  pub fn set_url(&mut self, url: impl Into<String>) {
    self.url = Some(url.into());
  }

  pub fn set_war_eligible(&mut self, eligible: bool) {
    self.war_eligible = Some(eligible);
  }
}

impl From<(i64, CorporationInfo)> for Model {
  fn from((id, info): (i64, CorporationInfo)) -> Self {
    let mut model = Self::new(id, info.name, info.ticker);

    model.set_ceo_id(info.ceo_id);
    model.set_creator_id(info.creator_id);
    model.set_member_count(info.member_count);
    model.set_tax_rate(info.tax_rate);

    if let Some(alliance_id) = info.alliance_id {
      model.set_alliance_id(alliance_id);
    }
    if let Some(date_founded) = info.date_founded {
      model.set_creation_date(date_founded);
    }
    if let Some(description) = info.description {
      model.set_description(description);
    }
    if let Some(faction_id) = info.faction_id {
      model.set_faction_id(faction_id);
    }
    if let Some(home_station_id) = info.home_station_id {
      model.set_home_station_id(home_station_id);
    }
    if let Some(shares) = info.shares {
      model.set_shares(shares);
    }
    if let Some(url) = info.url {
      model.set_url(url);
    }
    if let Some(war_eligible) = info.war_eligible {
      model.set_war_eligible(war_eligible);
    }

    model
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_info() -> CorporationInfo {
      CorporationInfo {
        alliance_id: None,
        ceo_id: 180_548_812,
        creator_id: 180_548_813,
        date_founded: None,
        description: None,
        faction_id: None,
        home_station_id: None,
        member_count: 42,
        name: "Test Corp".to_owned(),
        shares: None,
        tax_rate: 0.1,
        ticker: "TEST".to_owned(),
        url: None,
        war_eligible: None,
      }
    }

    #[test]
    fn it_applies_the_path_id_and_required_fields() {
      let model = Model::from((2000, make_info()));

      assert_eq!(model.id(), 2000);
      assert_eq!(model.ceo_id(), Some(180_548_812));
      assert_eq!(model.creator_id(), Some(180_548_813));
      assert_eq!(model.member_count(), Some(42));
      assert_eq!(model.tax_rate(), Some(0.1));
    }

    #[test]
    fn it_maps_optional_fields_when_present() {
      let mut info = make_info();
      info.alliance_id = Some(99_000_001);
      info.date_founded = Some("2005-06-23T00:00:00Z".to_owned());
      info.war_eligible = Some(true);

      let model = Model::from((2000, info));

      assert_eq!(model.alliance_id(), Some(99_000_001));
      assert_eq!(model.creation_date().as_deref(), Some("2005-06-23T00:00:00Z"));
      assert_eq!(model.war_eligible(), Some(true));
    }
  }
}
