// Force recompile
use {
	async_graphql::{
		Context,
		Error as GraphQLError,
		http::GraphiQLSource,
	},
	axum::{
		body::Bytes,
		extract::FromRef,
		response::{
			self,
			IntoResponse,
		},
	},
	axum_extra::extract::cookie::Key,
	casbin::Enforcer,
	deadpool::managed::{
		Manager as ManagedManager,
		Object,
		Pool,
	},
	deadpool_postgres::Manager,
	minio::s3,
	moka::future::Cache,
	std::{
		fmt,
		sync::{
			Arc,
			atomic::AtomicU64,
		},
		time::{
			SystemTime,
			UNIX_EPOCH,
		},
	},
	tokio::sync::RwLock,
};

pub mod constants;
pub mod controllers;
pub mod db;
pub mod email;
pub mod errors;
pub mod graphql;

#[derive(Debug, serde::Deserialize, Clone)]
pub struct Config {
	pub pg: deadpool_postgres::Config,
	pub enable_registration: bool,
	pub smtp_host: String,
	pub smtp_user: String,
	pub smtp_pass: String,
	pub smtp_from: String,
	pub cookie_secret: String,
	pub frontend_url: String,
	pub s3_endpoint_url: String,
	pub s3_access_key: String,
	pub s3_secret_key: String,
	pub s3_bucket_name: String,
	pub s3_region: String,
	pub s3_force_path_style: bool,
	pub server_host: String,
	pub server_port: u16,
	pub cors_allowed_origins: String,
}

impl Config {
	pub fn from_env() -> Result<Self, errors::AppError> {
		let cfg = config::Config::builder()
			.set_default("s3_region", "us-east-1")
			.map_err(errors::AppError::from)?
			.set_default("s3_force_path_style", true)
			.map_err(errors::AppError::from)?
			.add_source(config::Environment::default().separator("__"))
			.build()
			.map_err(errors::AppError::from)?;
		cfg.try_deserialize().map_err(errors::AppError::from)
	}
}

refinery::embed_migrations!("migrations");

pub struct UserId(pub i64);

pub struct SharedState<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
	pub s3_client: s3::Client,
	pub bucket_name: String,
	pub last_modified: AtomicU64,
	pub response_cache: Cache<u64, Bytes>,
	pub key: Key,
	pub config: Config,
	pub enforcer: Arc<RwLock<Enforcer>>,
}

impl<M: ManagedManager, W: From<Object<M>>> FromRef<SharedState<M, W>> for Key {
	fn from_ref(state: &SharedState<M, W>) -> Self {
		state.key.clone()
	}
}

impl<M: ManagedManager, W: From<Object<M>>> SharedState<M, W> {
	pub fn update_last_modified(&self) {
		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map(|d| d.as_millis() as u64)
			.unwrap_or_else(|e| {
				tracing::error!("System time is before UNIX EPOCH: {}", e);
				0
			});
		self.last_modified.store(now, std::sync::atomic::Ordering::Relaxed);
		self.response_cache.invalidate_all();
	}
}

pub struct AppState<M: ManagedManager, W: From<Object<M>>> {
	pub inner: Arc<SharedState<M, W>>,
}

impl<M: ManagedManager, W: From<Object<M>>> Clone for AppState<M, W> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
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
			.field("s3_client", &self.s3_client)
			.field("bucket_name", &self.bucket_name)
			.field("last_modified", &self.last_modified)
			.field("response_cache", &"Cache<u64, Bytes>")
			.field("enforcer", &"Enforcer")
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

	pub fn get_s3_client(&self) -> Result<&s3::Client, GraphQLError> {
		Ok(&self
			.0
			.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()?
			.s3_client)
	}

	pub fn get_bucket_name(&self) -> Result<&str, GraphQLError> {
		Ok(self
			.0
			.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()?
			.bucket_name
			.as_str())
	}
}

pub async fn graphiql() -> impl IntoResponse {
	response::Html(GraphiQLSource::build().endpoint("/").finish())
}

pub fn parse_latitude(latitude: f64) -> Result<f64, errors::AppError> {
	if (-90.0 ..= 90.0).contains(&latitude) {
		return Ok(latitude);
	}
	Err(errors::AppError::Validation(format!("{latitude} is not a valid latitude value.")))
}

pub fn parse_longitude(longitude: f64) -> Result<f64, errors::AppError> {
	if (-180.0 ..= 180.0).contains(&longitude) {
		return Ok(longitude);
	}
	Err(errors::AppError::Validation(format!("{longitude} is not a valid longitude value.")))
}

#[derive(Clone, serde::Serialize, Hash, Eq, PartialEq)]
pub struct CasbinUser {
	pub id: i64,
	pub role: String,
}

#[derive(Clone, serde::Serialize, Hash, Eq, PartialEq)]
pub struct CasbinObject {
	pub user_id: i64,
}

#[cfg(test)]
mod tests {
	use super::{
		errors::AppError,
		parse_latitude,
		parse_longitude,
	};

	#[test]
	fn parse_latitude_accepts_boundary_values() {
		assert!(parse_latitude(-90.0).is_ok());
		assert!(parse_latitude(0.0).is_ok());
		assert!(parse_latitude(90.0).is_ok());
	}

	#[test]
	fn parse_latitude_rejects_out_of_range_values() {
		assert!(matches!(parse_latitude(-90.1), Err(AppError::Validation(_))));
		assert!(matches!(parse_latitude(90.1), Err(AppError::Validation(_))));
	}

	#[test]
	fn parse_longitude_accepts_boundary_values() {
		assert!(parse_longitude(-180.0).is_ok());
		assert!(parse_longitude(0.0).is_ok());
		assert!(parse_longitude(180.0).is_ok());
	}

	#[test]
	fn parse_longitude_rejects_out_of_range_values() {
		assert!(matches!(parse_longitude(-180.1), Err(AppError::Validation(_))));
		assert!(matches!(parse_longitude(180.1), Err(AppError::Validation(_))));
	}
}
