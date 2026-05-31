use {
	async_graphql::{
		Context,
		Error as GraphQLError,
		http::GraphiQLSource,
	},
	axum::{
		body::Bytes,
		extract::FromRef,
		http::{
			HeaderValue,
			StatusCode,
		},
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

pub mod app;
pub mod constants;
pub mod controllers;
pub mod db;
pub mod email;
pub mod errors;
pub mod graphql;
pub mod object_lifecycle;
pub mod storage;

use {
	object_lifecycle::ObjectLifecycleConfig,
	storage::{
		StorageClient,
		StorageConfig,
	},
};

#[derive(serde::Deserialize, Clone)]
struct AppConfig {
	pub pg: deadpool_postgres::Config,
	pub enable_registration: bool,
	pub smtp_host: String,
	pub smtp_user: String,
	pub smtp_pass: String,
	pub smtp_from: String,
	pub cookie_secret: String,
	pub frontend_url: String,
	pub server_host: String,
	pub server_port: u16,
	pub cors_allowed_origins: String,
}

#[derive(Clone)]
pub struct Config {
	pub pg: deadpool_postgres::Config,
	pub enable_registration: bool,
	pub smtp_host: String,
	pub smtp_user: String,
	pub smtp_pass: String,
	pub smtp_from: String,
	pub cookie_secret: String,
	pub frontend_url: String,
	pub storage: StorageConfig,
	pub object_lifecycle: ObjectLifecycleConfig,
	pub server_host: String,
	pub server_port: u16,
	pub cors_allowed_origins: String,
}

impl Config {
	/// Whether auth cookies should carry the `Secure` attribute.
	///
	/// Derived from `frontend_url` so login and logout agree on the cookie shape;
	/// without that the browser may refuse the logout overwrite.
	pub fn cookie_secure(&self) -> bool {
		self.frontend_url.starts_with("https")
	}

	pub fn from_env() -> Result<Self, errors::AppError> {
		let cfg = config::Config::builder()
			.add_source(config::Environment::default().separator("__"))
			.build()
			.map_err(errors::AppError::from)?;
		let config: AppConfig = cfg.try_deserialize().map_err(errors::AppError::from)?;
		let storage = StorageConfig::from_env().map_err(errors::AppError::from)?;
		let object_lifecycle = ObjectLifecycleConfig::from_env().map_err(errors::AppError::from)?;
		Ok(Self {
			pg: config.pg,
			enable_registration: config.enable_registration,
			smtp_host: config.smtp_host,
			smtp_user: config.smtp_user,
			smtp_pass: config.smtp_pass,
			smtp_from: config.smtp_from,
			cookie_secret: config.cookie_secret,
			frontend_url: config.frontend_url,
			storage,
			object_lifecycle,
			server_host: config.server_host,
			server_port: config.server_port,
			cors_allowed_origins: config.cors_allowed_origins,
		})
	}
}

impl fmt::Debug for Config {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("Config")
			.field("pg", &"<redacted>")
			.field("enable_registration", &self.enable_registration)
			.field("smtp_host", &self.smtp_host)
			.field("smtp_user", &self.smtp_user)
			.field("smtp_pass", &"<redacted>")
			.field("smtp_from", &self.smtp_from)
			.field("cookie_secret", &"<redacted>")
			.field("frontend_url", &self.frontend_url)
			.field("storage", &self.storage)
			.field("object_lifecycle", &self.object_lifecycle)
			.field("server_host", &self.server_host)
			.field("server_port", &self.server_port)
			.field("cors_allowed_origins", &self.cors_allowed_origins)
			.finish()
	}
}

refinery::embed_migrations!("migrations");

pub struct UserId(pub i64);

/// A previously-computed GraphQL response cached for replay.
///
/// Stores the original status and `Content-Type` so the cache hit path can
/// rebuild the response exactly, instead of forcing every cached response
/// to a hard-coded 200 OK.
#[derive(Clone, Debug)]
pub struct CachedResponse {
	pub status: StatusCode,
	pub content_type: Option<HeaderValue>,
	pub bytes: Bytes,
}

impl CachedResponse {
	/// Byte cost of an entry, used by the response cache weigher.
	pub fn weight(&self) -> u32 {
		let content_type = self.content_type.as_ref().map(|v| v.len()).unwrap_or(0);
		// `status` and the struct overhead are constant; weigh by what scales with payload.
		u32::try_from(self.bytes.len().saturating_add(content_type)).unwrap_or(u32::MAX)
	}
}

pub struct SharedState<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
	pub storage: StorageClient,
	pub last_modified: AtomicU64,
	pub response_cache: Cache<u64, CachedResponse>,
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
			.field("storage", &self.storage)
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

	pub fn get_storage_client(&self) -> Result<&StorageClient, GraphQLError> {
		Ok(&self
			.0
			.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()?
			.storage)
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
	use {
		super::{
			Config,
			errors::AppError,
			object_lifecycle::ObjectLifecycleConfig,
			parse_latitude,
			parse_longitude,
			storage::StorageConfig,
		},
		deadpool_postgres::Config as PostgresConfig,
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

	#[test]
	fn app_config_debug_redacts_secrets() {
		let config = Config {
			pg: PostgresConfig::new(),
			enable_registration: true,
			smtp_host: "smtp.example.test".to_string(),
			smtp_user: "debug-smtp-user".to_string(),
			smtp_pass: "debug-smtp-pass-secret".to_string(),
			smtp_from: "noreply@example.test".to_string(),
			cookie_secret: "debug-cookie-secret".to_string(),
			frontend_url: "https://memory-map.example.test".to_string(),
			storage: StorageConfig {
				endpoint_url: "https://s3.example.test".to_string(),
				access_key: "debug-storage-access-secret".to_string(),
				secret_key: "debug-storage-secret-secret".to_string(),
				bucket_name: "memory-map".to_string(),
				region: "us-east-1".to_string(),
				force_path_style: false,
				presigned_url_ttl_seconds: 60,
			},
			object_lifecycle: ObjectLifecycleConfig::default(),
			server_host: "127.0.0.1".to_string(),
			server_port: 8000,
			cors_allowed_origins: "https://memory-map.example.test".to_string(),
		};

		let debug = format!("{config:?}");

		assert!(debug.contains("Config"));
		assert!(debug.contains("smtp.example.test"));
		assert!(debug.contains("https://s3.example.test"));
		assert!(debug.contains("<redacted>"));
		assert!(!debug.contains("debug-smtp-pass-secret"));
		assert!(!debug.contains("debug-cookie-secret"));
		assert!(!debug.contains("debug-storage-access-secret"));
		assert!(!debug.contains("debug-storage-secret-secret"));
	}
}
