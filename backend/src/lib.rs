use async_graphql::http::GraphiQLSource;
use async_graphql::{Context, Error as GraphQLError};
use axum::{
	body::Bytes,
	extract::FromRef,
	response::{self, IntoResponse},
};
use axum_extra::extract::cookie::Key;
use deadpool::managed::{Manager as ManagedManager, Object, Pool};
use deadpool_postgres::Manager;
use minio::s3;
use moka::future::Cache;
use std::{
	fmt,
	sync::{
		Arc,
		atomic::AtomicU64,
	},
	time::{SystemTime, UNIX_EPOCH},
};

pub mod controllers;
pub mod graphql;

#[derive(Debug, serde::Deserialize)]
pub struct Config {
	pub pg: deadpool_postgres::Config,
	pub enable_registration: bool,
	pub smtp_host: String,
	pub smtp_user: String,
	pub smtp_pass: String,
	pub smtp_from: String,
	pub cookie_secret: String,
}

impl Config {
	pub fn from_env() -> Result<Self, config::ConfigError> {
		config::Config::builder()
			.add_source(config::Environment::default().separator("__"))
			.build()?
			.try_deserialize()
	}
}

// 1GB.
pub const BODY_MAX_SIZE_LIMIT_BYTES: usize = 1_073_741_824;

refinery::embed_migrations!("migrations");

pub struct UserId(pub i64);

pub struct SharedState<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
	pub minio_client: s3::Client,
	pub bucket_name: String,
	pub last_modified: AtomicU64,
	pub response_cache: Cache<u64, Bytes>,
	pub key: Key,
}

impl<M: ManagedManager, W: From<Object<M>>> FromRef<SharedState<M, W>> for Key {
	fn from_ref(state: &SharedState<M, W>) -> Self {
		state.key.clone()
	}
}

impl<M: ManagedManager, W: From<Object<M>>> SharedState<M, W> {
	pub fn update_last_modified(&self) {
		let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
		self.last_modified.store(now, std::sync::atomic::Ordering::Relaxed);
		self.response_cache.invalidate_all();
	}
}

pub struct AppState<M: ManagedManager, W: From<Object<M>>> {
	pub inner: Arc<SharedState<M, W>>,
}

impl<M: ManagedManager, W: From<Object<M>>> Clone for AppState<M, W> {
	fn clone(&self) -> Self {
		Self { inner: self.inner.clone() }
	}
}

impl<M: ManagedManager, W: From<Object<M>>> FromRef<AppState<M, W>> for Key {
	fn from_ref(state: &AppState<M, W>) -> Self {
		state.inner.key.clone()
	}
}

impl<M: ManagedManager, W: From<Object<M>>> FromRef<AppState<M, W>> for Arc<SharedState<M, W>> {
	fn from_ref(state: &AppState<M, W>) -> Self {
		state.inner.clone()
	}
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
			.field("last_modified", &self.last_modified)
			.field("response_cache", &"Cache<u64, Bytes>")
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
