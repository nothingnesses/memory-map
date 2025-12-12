use async_graphql::http::GraphiQLSource;
use axum::response::{self, IntoResponse};
pub mod controllers;
pub mod graphql;
use minio::s3;

pub const ONE_GB: usize = 1_073_741_824;

#[derive(Debug, serde::Deserialize)]
pub struct Config {
	pub pg: deadpool_postgres::Config,
}

impl Config {
	pub fn from_env() -> Result<Self, config::ConfigError> {
		config::Config::builder()
			.add_source(config::Environment::default().separator("__"))
			.build()?
			.try_deserialize()
	}
}

refinery::embed_migrations!("migrations");

pub struct AxumState {
	pub minio_client: s3::Client,
	pub bucket_name: String,
}

pub async fn graphiql() -> impl IntoResponse {
	response::Html(GraphiQLSource::build().endpoint("/").finish())
}

pub fn parse_latitude(latitude: f64) -> Result<f64, Box<dyn std::error::Error>> {
	if latitude >= -90.0 && latitude <= 90.0 {
		return Ok(latitude);
	}
	return Err(format!("{latitude} is not a valid latitude value.").into());
}

pub fn parse_longitude(longitude: f64) -> Result<f64, Box<dyn std::error::Error>> {
	if longitude >= -180.0 && longitude <= 180.0 {
		return Ok(longitude);
	}
	return Err(format!("{longitude} is not a valid longitude value.").into());
}
