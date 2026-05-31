use {
	crate::{
		errors::AppError,
		parse_latitude,
		parse_longitude,
	},
	async_graphql::{
		InputObject,
		SimpleObject,
	},
	serde::Serialize,
	tokio_postgres::Row,
};

#[derive(SimpleObject, InputObject, Clone, Debug, Serialize)]
#[graphql(concrete(name = "Location", input_name = "LocationInput", params()))]
pub struct Location {
	pub latitude: f64,
	pub longitude: f64,
}

impl Location {
	pub fn validated(self) -> Result<Self, AppError> {
		Ok(Self {
			latitude: parse_latitude(self.latitude)?,
			longitude: parse_longitude(self.longitude)?,
		})
	}

	pub fn geometry(&self) -> Result<String, AppError> {
		let latitude = parse_latitude(self.latitude)?;
		let longitude = parse_longitude(self.longitude)?;
		Ok(format!("SRID=4326;POINT({longitude} {latitude})"))
	}
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

#[cfg(test)]
mod tests {
	use {
		super::Location,
		crate::errors::AppError,
	};

	#[test]
	fn validated_accepts_boundary_coordinates() {
		let location = Location {
			latitude: -90.0,
			longitude: 180.0,
		}
		.validated()
		.expect("valid coordinates should be accepted");

		assert_eq!(location.latitude, -90.0);
		assert_eq!(location.longitude, 180.0);
	}

	#[test]
	fn validated_rejects_out_of_range_coordinates() {
		let location = Location {
			latitude: 90.1,
			longitude: 0.0,
		};

		assert!(matches!(location.validated(), Err(AppError::Validation(_))));
	}

	#[test]
	fn geometry_validates_before_formatting() {
		let location = Location {
			latitude: 0.0,
			longitude: 180.1,
		};

		assert!(matches!(location.geometry(), Err(AppError::Validation(_))));
	}

	#[test]
	fn geometry_formats_valid_coordinates() {
		let location = Location {
			latitude: 12.5,
			longitude: -45.25,
		};

		assert_eq!(location.geometry().expect("valid geometry"), "SRID=4326;POINT(-45.25 12.5)");
	}
}
