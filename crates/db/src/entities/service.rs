//! Database entity types for station services.

use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

/// A list of service name strings stored as a JSON column.
#[derive(Clone, Debug, Default, Deserialize, FromJsonQueryResult, PartialEq, Serialize)]
pub struct List(pub Vec<String>);
