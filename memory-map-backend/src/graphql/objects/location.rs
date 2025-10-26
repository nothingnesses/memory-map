use async_graphql::{InputObject, SimpleObject};
use tokio_postgres::{Error as TPError, Row};

#[derive(SimpleObject, InputObject, Clone)]
#[graphql(concrete(name = "Location", input_name = "LocationInput", params()))]
pub struct Location {
	pub latitude: f64,
	pub longitude: f64,
}

impl TryFrom<Row> for Location {
	type Error = TPError;

	fn try_from(value: Row) -> Result<Self, Self::Error> {
		Ok(Location {
			latitude: value.try_get("latitude")?,
			longitude: value.try_get("longitude")?,
		})
	}
}
