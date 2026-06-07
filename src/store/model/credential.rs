use getset::{CopyGetters, Getters};
use sqlx::{
  Decode, Encode, FromRow, Sqlite, Type,
  encode::IsNull,
  error::BoxDynError,
  sqlite::{SqliteArgumentsBuffer, SqliteTypeInfo, SqliteValueRef},
};

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  access_token: String,
  #[getset(get_copy = "pub")]
  authorized_by: Option<i64>,
  #[getset(get_copy = "pub")]
  created_at: i64,
  #[getset(get_copy = "pub")]
  expires_at: i64,
  #[getset(get_copy = "pub")]
  owner_id: i64,
  #[getset(get_copy = "pub")]
  owner_type: OwnerType,
  #[getset(get = "pub")]
  refresh_token: String,
  #[getset(get = "pub")]
  scopes: Option<String>,
  #[getset(get_copy = "pub")]
  updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerType {
  Character,
  Corporation,
}

impl<'r> Decode<'r, Sqlite> for OwnerType {
  fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
    let s = <String as Decode<'r, Sqlite>>::decode(value)?;
    match s.as_str() {
      "character" => Ok(Self::Character),
      "corporation" => Ok(Self::Corporation),
      other => Err(format!("unknown owner_type: {other}").into()),
    }
  }
}

impl Encode<'_, Sqlite> for OwnerType {
  fn encode_by_ref(&self, buf: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
    let s = match self {
      Self::Character => "character",
      Self::Corporation => "corporation",
    };
    <String as Encode<'_, Sqlite>>::encode(s.to_string(), buf)
  }
}

impl Type<Sqlite> for OwnerType {
  fn type_info() -> SqliteTypeInfo {
    <str as Type<Sqlite>>::type_info()
  }
}
