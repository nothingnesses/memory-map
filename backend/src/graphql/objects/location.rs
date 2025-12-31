use async_graphql::{InputObject, SimpleObject};
use serde::Serialize;
use tokio_postgres::Row;

use crate::{parse_latitude, parse_longitude};

#[derive(SimpleObject, InputObject, Clone, Debug, Serialize)]
#[graphql(concrete(name = "Location", input_name = "LocationInput", params()))]
pub struct Location {
	pub latitude: f64,
	pub longitude: f64,
}

impl TryFrom<Row> for Location {
	type Error = Box<dyn std::error::Error>;

	fn try_from(value: Row) -> Result<Self, Self::Error> {
		Ok(Location {
			latitude: parse_latitude(value.try_get("latitude")?)?,
			longitude: parse_longitude(value.try_get("longitude")?)?,
		})
	}
}
