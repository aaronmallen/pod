use std::{
  fmt::{self, Display, Formatter},
  str::FromStr,
};

use getset::{CopyGetters, Getters};
use sqlx::{
  Decode, Encode, FromRow, Sqlite, Type,
  encode::IsNull,
  error::BoxDynError,
  sqlite::{SqliteArgumentsBuffer, SqliteTypeInfo, SqliteValueRef},
};

use crate::clients::esi::models::character::CharacterInfo;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gender {
  Female,
  Male,
}

impl Gender {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Female => "female",
      Self::Male => "male",
    }
  }
}

impl Display for Gender {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

impl FromStr for Gender {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "female" => Ok(Self::Female),
      "male" => Ok(Self::Male),
      other => Err(format!("unknown gender: {other}")),
    }
  }
}

impl Type<Sqlite> for Gender {
  fn type_info() -> SqliteTypeInfo {
    <String as Type<Sqlite>>::type_info()
  }
}

impl<'q> Encode<'q, Sqlite> for Gender {
  fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
    <String as Encode<Sqlite>>::encode(self.as_str().to_string(), args)
  }
}

impl<'r> Decode<'r, Sqlite> for Gender {
  fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
    let s = <&str as Decode<Sqlite>>::decode(value)?;
    Self::from_str(s).map_err(|e| e.into())
  }
}

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  alliance_id: Option<i64>,
  #[getset(get = "pub")]
  birthday: String,
  #[getset(get_copy = "pub")]
  bloodline_id: i64,
  #[getset(get_copy = "pub")]
  corporation_id: i64,
  #[getset(get = "pub")]
  description: Option<String>,
  #[getset(get_copy = "pub")]
  faction_id: Option<i64>,
  #[getset(get = "pub")]
  gender: Gender,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  name: String,
  #[getset(get_copy = "pub")]
  race_id: i64,
  #[getset(get_copy = "pub")]
  security_status: Option<f64>,
  #[getset(get = "pub")]
  title: Option<String>,
}

impl Model {
  pub fn new(
    id: i64,
    bloodline_id: i64,
    corporation_id: i64,
    race_id: i64,
    birthday: impl Into<String>,
    gender: Gender,
    name: impl Into<String>,
  ) -> Self {
    Self {
      alliance_id: None,
      birthday: birthday.into(),
      bloodline_id,
      corporation_id,
      description: None,
      faction_id: None,
      gender,
      id,
      name: name.into(),
      race_id,
      security_status: None,
      title: None,
    }
  }

  pub fn set_alliance_id(&mut self, id: i64) {
    self.alliance_id = Some(id);
  }

  pub fn set_description(&mut self, description: impl Into<String>) {
    self.description = Some(description.into());
  }

  pub fn set_faction_id(&mut self, id: i64) {
    self.faction_id = Some(id);
  }

  pub fn set_security_status(&mut self, status: f64) {
    self.security_status = Some(status);
  }

  pub fn set_title(&mut self, title: impl Into<String>) {
    self.title = Some(title.into());
  }
}

impl From<(i64, CharacterInfo)> for Model {
  fn from((id, info): (i64, CharacterInfo)) -> Self {
    let gender = Gender::from_str(&info.gender).unwrap_or(Gender::Male);
    let mut model = Self::new(
      id,
      i64::from(info.bloodline_id),
      info.corporation_id,
      i64::from(info.race_id),
      info.birthday,
      gender,
      info.name,
    );

    if let Some(alliance_id) = info.alliance_id {
      model.set_alliance_id(alliance_id);
    }
    if let Some(description) = info.description {
      model.set_description(description);
    }
    if let Some(faction_id) = info.faction_id {
      model.set_faction_id(faction_id);
    }
    if let Some(security_status) = info.security_status {
      model.set_security_status(security_status);
    }
    if let Some(title) = info.title {
      model.set_title(title);
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

    fn make_info() -> CharacterInfo {
      CharacterInfo {
        alliance_id: None,
        birthday: "2015-03-24T11:37:00Z".to_owned(),
        bloodline_id: 3,
        corporation_id: 109_299_958,
        description: None,
        faction_id: None,
        gender: "female".to_owned(),
        name: "Test Pilot".to_owned(),
        race_id: 2,
        security_status: None,
        title: None,
      }
    }

    #[test]
    fn it_applies_the_path_id_and_parses_gender() {
      let model = Model::from((42, make_info()));

      assert_eq!(model.id(), 42);
      assert_eq!(model.gender(), &Gender::Female);
      assert_eq!(model.bloodline_id(), 3);
      assert_eq!(model.race_id(), 2);
    }

    #[test]
    fn it_defaults_unknown_gender_to_male() {
      let mut info = make_info();
      info.gender = "nonbinary".to_owned();

      let model = Model::from((42, info));

      assert_eq!(model.gender(), &Gender::Male);
    }

    #[test]
    fn it_maps_optional_fields_when_present() {
      let mut info = make_info();
      info.alliance_id = Some(99_000_001);
      info.faction_id = Some(500_001);
      info.security_status = Some(4.5);
      info.title = Some("CEO".to_owned());

      let model = Model::from((42, info));

      assert_eq!(model.alliance_id(), Some(99_000_001));
      assert_eq!(model.faction_id(), Some(500_001));
      assert_eq!(model.security_status(), Some(4.5));
      assert_eq!(model.title().as_deref(), Some("CEO"));
    }
  }
}
