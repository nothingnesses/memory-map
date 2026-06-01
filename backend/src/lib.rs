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
	serde::Deserialize,
	storage::{
		StorageClient,
		StorageConfig,
	},
};

#[derive(Clone, Debug, Deserialize)]
pub struct ServerConfig {
	pub host: String,
	pub port: u16,
}

#[derive(Clone, Deserialize)]
pub struct SmtpConfig {
	pub host: String,
	pub user: String,
	pub pass: String,
	pub from: String,
}

impl fmt::Debug for SmtpConfig {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("SmtpConfig")
			.field("host", &self.host)
			.field("user", &self.user)
			.field("pass", &"<redacted>")
			.field("from", &self.from)
			.finish()
	}
}

#[derive(Clone, Deserialize)]
pub struct AuthConfig {
	pub cookie_secret: String,
	pub enable_registration: bool,
}

impl fmt::Debug for AuthConfig {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("AuthConfig")
			.field("cookie_secret", &"<redacted>")
			.field("enable_registration", &self.enable_registration)
			.finish()
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct FrontendConfig {
	pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CorsConfig {
	pub allowed_origins: String,
}

#[derive(Clone, Deserialize)]
pub struct Config {
	pub pg: deadpool_postgres::Config,
	pub server: ServerConfig,
	pub smtp: SmtpConfig,
	pub auth: AuthConfig,
	pub frontend: FrontendConfig,
	pub cors: CorsConfig,
	pub storage: StorageConfig,
	pub object_lifecycle: ObjectLifecycleConfig,
}

impl Config {
	/// Whether auth cookies should carry the `Secure` attribute.
	///
	/// Derived from the frontend URL so login and logout agree on the cookie shape;
	/// without that the browser may refuse the logout overwrite.
	pub fn cookie_secure(&self) -> bool {
		self.frontend.url.starts_with("https")
	}

	/// Loads, deserializes, and validates the configuration from environment variables.
	///
	/// All env vars share the `MEMORY_MAP__` prefix with `__` as both the prefix and
	/// the nested-path separator, so `MEMORY_MAP__STORAGE__ENDPOINT_URL` maps to
	/// `config.storage.endpoint_url`. One source of truth, one deserialization path,
	/// one validation pass.
	pub fn from_env() -> Result<Self, errors::AppError> {
		let raw = config::Config::builder()
			.add_source(
				config::Environment::with_prefix("MEMORY_MAP")
					.prefix_separator("__")
					.separator("__"),
			)
			.build()
			.map_err(errors::AppError::from)?;
		let config: Config = raw.try_deserialize().map_err(errors::AppError::from)?;
		config.validated().map_err(errors::AppError::from)
	}

	/// Runs sub-config validation. Call after deserialization or struct construction.
	pub fn validated(self) -> anyhow::Result<Self> {
		self.storage.validate()?;
		self.object_lifecycle.validate()?;
		Ok(self)
	}
}

impl fmt::Debug for Config {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("Config")
			.field("pg", &"<redacted>")
			.field("server", &self.server)
			.field("smtp", &self.smtp)
			.field("auth", &self.auth)
			.field("frontend", &self.frontend)
			.field("cors", &self.cors)
			.field("storage", &self.storage)
			.field("object_lifecycle", &self.object_lifecycle)
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
	/// Bumps the cache-busting timestamp and invalidates the response cache.
	///
	/// Returns Err if the system clock is somehow before UNIX_EPOCH; callers
	/// propagate via `?` so the failure is observable rather than silently
	/// producing a zero timestamp that would break ETag semantics.
	pub fn update_last_modified(&self) -> Result<(), errors::AppError> {
		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map_err(|e| anyhow::anyhow!("System time is before UNIX EPOCH: {e}"))?;
		self.last_modified.store(now.as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
		self.response_cache.invalidate_all();
		Ok(())
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

	/// Resolves the caller's `UserId`, loads their `User`, looks up the enforcer,
	/// and returns the `CasbinUser` if `(caller, target, action)` is allowed.
	///
	/// Replaces ~15 lines of repeated state/enforcer/user/casbin boilerplate
	/// in every authorized mutation and query resolver.
	pub async fn require_permission(
		&self,
		action: &str,
		target: CasbinObject,
	) -> Result<CasbinUser, GraphQLError> {
		use {
			casbin::CoreApi,
			errors::AppError,
			graphql::objects::user::User,
		};

		let user_id =
			self.0.data_opt::<UserId>().ok_or_else(|| AppError::Unauthorized.extend_graphql())?.0;
		let state = self
			.0
			.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Shared state not found in context"))?;
		let enforcer = state.enforcer.read().await;
		let user = User::by_id(self.0, user_id)
			.await?
			.ok_or_else(|| AppError::NotFound("User not found".to_string()).extend_graphql())?;
		let casbin_user = CasbinUser {
			id: user_id,
			role: user.role.to_string(),
		};
		if !enforcer.enforce((casbin_user.clone(), target, action)).map_err(AppError::from)? {
			return Err(AppError::Forbidden.extend_graphql());
		}
		Ok(casbin_user)
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
			AuthConfig,
			Config,
			CorsConfig,
			FrontendConfig,
			ServerConfig,
			SmtpConfig,
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
			server: ServerConfig {
				host: "127.0.0.1".to_string(),
				port: 8000,
			},
			smtp: SmtpConfig {
				host: "smtp.example.test".to_string(),
				user: "debug-smtp-user".to_string(),
				pass: "debug-smtp-pass-secret".to_string(),
				from: "noreply@example.test".to_string(),
			},
			auth: AuthConfig {
				cookie_secret: "debug-cookie-secret".to_string(),
				enable_registration: true,
			},
			frontend: FrontendConfig {
				url: "https://memory-map.example.test".to_string(),
			},
			cors: CorsConfig {
				allowed_origins: "https://memory-map.example.test".to_string(),
			},
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
