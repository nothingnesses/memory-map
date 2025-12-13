use async_graphql::http::GraphiQLSource;
use axum::response::{self, IntoResponse};
use std::fmt;
pub mod controllers;
pub mod graphql;
use async_graphql::{Context, Error as GraphQLError};
use deadpool::managed::{Manager as ManagedManager, Object, Pool};
use deadpool_postgres::Manager;
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

pub struct SharedState<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
	pub minio_client: s3::Client,
	pub bucket_name: String,
}

impl<M: ManagedManager, W: From<Object<M>>> fmt::Debug for SharedState<M, W> {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("SharedState")
			.field("pool", &"Pool")
			.field("minio_client", &self.minio_client)
			.field("bucket_name", &self.bucket_name)
			.finish()
	}
}

pub struct ContextWrapper<'a>(&'a Context<'a>);

impl<'a> ContextWrapper<'a> {
	pub async fn get_db_client(&self) -> Result<Object<Manager>, GraphQLError> {
		let pool: &Pool<Manager> =
			&self.0.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()?.pool;
		Ok(pool.get().await?)
	}

	pub fn get_minio_client(&self) -> Result<&s3::Client, GraphQLError> {
		Ok(&self
			.0
			.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()?
			.minio_client)
	}

	pub fn get_bucket_name(&self) -> Result<&str, GraphQLError> {
		Ok(&self
			.0
			.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()?
			.bucket_name
			.as_str())
	}
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
