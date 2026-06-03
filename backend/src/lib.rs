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
			HeaderMap,
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
			atomic::{
				AtomicU64,
				AtomicUsize,
				Ordering,
			},
		},
	},
	tokio::sync::RwLock,
};

pub mod app;
pub mod constants;
pub mod db;
pub mod email;
pub mod email_worker;
pub mod errors;
pub mod graphql;
pub mod object_lifecycle;
pub mod outbox;
pub mod storage;
pub mod worker;

use {
	email_worker::EmailOutboxConfig,
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
	#[serde(default)]
	pub cookie_secure: Option<bool>,
}

impl AuthConfig {
	const MIN_COOKIE_SECRET_BYTES: usize = 64;

	pub fn validate(&self) -> anyhow::Result<()> {
		if self.cookie_secret.len() < Self::MIN_COOKIE_SECRET_BYTES {
			anyhow::bail!(
				"auth.cookie_secret must be at least {} bytes",
				Self::MIN_COOKIE_SECRET_BYTES
			);
		}
		Ok(())
	}
}

impl fmt::Debug for AuthConfig {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("AuthConfig")
			.field("cookie_secret", &"<redacted>")
			.field("enable_registration", &self.enable_registration)
			.field("cookie_secure", &self.cookie_secure)
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
	#[serde(default)]
	pub object_lifecycle: ObjectLifecycleConfig,
	#[serde(default)]
	pub email_outbox: EmailOutboxConfig,
}

impl Config {
	/// Whether auth cookies should carry the `Secure` attribute.
	///
	/// Explicit config wins; otherwise derive from the frontend URL so login and
	/// logout agree on the cookie shape. Without that the browser may refuse the
	/// logout overwrite.
	pub fn cookie_secure(&self) -> bool {
		self.auth.cookie_secure.unwrap_or_else(|| self.frontend.url.starts_with("https"))
	}

	/// Loads, deserializes, and validates the configuration.
	///
	/// An optional TOML file (selected by the `MEMORY_MAP_CONFIG` env var) is read
	/// first, then the environment is layered on top so env always wins. When
	/// `MEMORY_MAP_CONFIG` is unset, no file source is added and loading is
	/// pure-environment, identical to the previous env-only behaviour. Env vars
	/// share the `MEMORY_MAP__` prefix with `__` as both the prefix and the
	/// nested-path separator, so `MEMORY_MAP__STORAGE__ENDPOINT_URL` maps to
	/// `config.storage.endpoint_url`.
	pub fn load() -> Result<Self, errors::AppError> {
		let mut builder = config::Config::builder();
		// The file is opt-in and, when requested, required: a missing or unreadable
		// path is a loud startup error rather than a silent fall-through to defaults.
		if let Ok(path) = std::env::var("MEMORY_MAP_CONFIG") {
			builder = builder
				.add_source(config::File::new(&path, config::FileFormat::Toml).required(true));
		}
		let raw = builder
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
		self.auth.validate()?;
		self.storage.validate()?;
		self.object_lifecycle.validate()?;
		self.email_outbox.validate()?;
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
			.field("email_outbox", &self.email_outbox)
			.finish()
	}
}

refinery::embed_migrations!("migrations");

#[derive(Clone, Debug)]
pub struct CallerIdentity {
	pub user_id: i64,
	pub casbin_user: CasbinUser,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GraphqlResponseCacheKey([u8; 32]);

impl GraphqlResponseCacheKey {
	pub fn new(bytes: [u8; 32]) -> Self {
		Self(bytes)
	}
}

/// A previously-computed GraphQL query response cached for replay.
///
/// The cache stores only successful query responses. Mutations never read from
/// it, and successful writes invalidate it centrally.
#[derive(Clone, Debug)]
pub struct CachedGraphqlResponse {
	pub status: StatusCode,
	pub headers: HeaderMap,
	pub bytes: Bytes,
}

impl CachedGraphqlResponse {
	/// Byte cost of an entry, used by the response cache weigher.
	pub fn weight(&self) -> u32 {
		let header_bytes = self
			.headers
			.iter()
			.map(|(name, value)| name.as_str().len().saturating_add(value.len()))
			.sum::<usize>();
		u32::try_from(self.bytes.len().saturating_add(header_bytes)).unwrap_or(u32::MAX)
	}
}

/// Per-request mutation cache effect accounting.
///
/// Mutation resolvers that do not change query-visible state may mark their root
/// field as non-invalidating. The handler suppresses global invalidation only
/// when every root mutation field in the operation was marked this way.
#[derive(Debug, Default)]
pub struct GraphqlMutationCacheEffect {
	non_invalidating_field_count: AtomicUsize,
}

impl GraphqlMutationCacheEffect {
	pub fn mark_non_invalidating_field(&self) {
		self.non_invalidating_field_count.fetch_add(1, Ordering::AcqRel);
	}

	pub fn non_invalidating_field_count(&self) -> usize {
		self.non_invalidating_field_count.load(Ordering::Acquire)
	}
}

pub struct SharedState<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
	pub storage: StorageClient,
	pub graphql_response_cache_epoch: AtomicU64,
	pub graphql_response_cache: Cache<GraphqlResponseCacheKey, CachedGraphqlResponse>,
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
	pub fn graphql_response_cache_epoch(&self) -> u64 {
		self.graphql_response_cache_epoch.load(Ordering::Acquire)
	}

	pub fn invalidate_graphql_response_cache(&self) {
		self.graphql_response_cache_epoch.fetch_add(1, Ordering::AcqRel);
		self.graphql_response_cache.invalidate_all();
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
			.field("enforcer", &"Enforcer")
			.finish()
	}
}

pub struct ContextWrapper<'a> {
	state: &'a Arc<SharedState<Manager, deadpool_postgres::Client>>,
	caller_identity: Option<&'a CallerIdentity>,
}

impl<'a> ContextWrapper<'a> {
	pub fn new(ctx: &'a Context<'a>) -> Result<Self, GraphQLError> {
		let state = ctx
			.data::<std::sync::Arc<SharedState<Manager, deadpool_postgres::Client>>>()
			.map_err(|e| {
				errors::AppError::Internal(anyhow::anyhow!(
					"Shared state not found in context: {}",
					e.message
				))
			})
			.map_err(errors::AppError::graphql)?;
		Ok(Self {
			state,
			caller_identity: ctx.data_opt::<CallerIdentity>(),
		})
	}

	pub fn shared_state(&self) -> &'a Arc<SharedState<Manager, deadpool_postgres::Client>> {
		self.state
	}

	pub async fn db_client(&self) -> Result<Object<Manager>, GraphQLError> {
		self.state.pool.get().await.map_err(errors::AppError::graphql)
	}

	pub fn storage_client(&self) -> &StorageClient {
		&self.state.storage
	}

	pub fn caller_identity(&self) -> Result<&CallerIdentity, GraphQLError> {
		self.caller_identity.ok_or_else(|| errors::AppError::Unauthorized.extend_graphql())
	}

	pub fn caller_identity_opt(&self) -> Option<&CallerIdentity> {
		self.caller_identity
	}

	pub fn user_id(&self) -> Result<i64, GraphQLError> {
		Ok(self.caller_identity()?.user_id)
	}

	pub fn user_id_opt(&self) -> Option<i64> {
		self.caller_identity.map(|identity| identity.user_id)
	}

	pub async fn has_permission(
		&self,
		action: &str,
		target: CasbinObject,
	) -> Result<bool, GraphQLError> {
		use {
			casbin::CoreApi,
			errors::AppError,
		};

		let caller_identity = self.caller_identity()?;
		let enforcer = self.state.enforcer.read().await;
		enforcer
			.enforce((caller_identity.casbin_user.clone(), target, action))
			.map_err(AppError::graphql)
	}

	pub async fn require_permission(
		&self,
		action: &str,
		target: CasbinObject,
	) -> Result<(), GraphQLError> {
		self.require_permission_on_each(action, [target]).await
	}

	pub async fn require_permission_on_each<I>(
		&self,
		action: &str,
		targets: I,
	) -> Result<(), GraphQLError>
	where
		I: IntoIterator<Item = CasbinObject>, {
		use {
			casbin::CoreApi,
			errors::AppError,
		};

		let caller_identity = self.caller_identity()?;
		let enforcer = self.state.enforcer.read().await;
		for target in targets {
			if !enforcer
				.enforce((caller_identity.casbin_user.clone(), target, action))
				.map_err(AppError::graphql)?
			{
				return Err(AppError::Forbidden.extend_graphql());
			}
		}
		Ok(())
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

#[derive(Clone, Debug, serde::Serialize, Hash, Eq, PartialEq)]
pub struct CasbinUser {
	pub id: i64,
	pub role: String,
}

#[derive(Clone, Debug, serde::Serialize, Hash, Eq, PartialEq)]
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
			email_worker::EmailOutboxConfig,
			errors::AppError,
			object_lifecycle::ObjectLifecycleConfig,
			parse_latitude,
			parse_longitude,
			storage::StorageConfig,
		},
		deadpool_postgres::Config as PostgresConfig,
	};

	#[test]
	fn config_example_toml_deserializes_into_config() {
		// Guards against drift between config.example.toml and the Config structs:
		// a renamed or removed field there fails to deserialize here. Only the
		// structural mapping is checked; validation is not run because the example
		// intentionally leaves secrets blank.
		let parsed = config::Config::builder()
			.add_source(config::File::from_str(
				include_str!("../../config.example.toml"),
				config::FileFormat::Toml,
			))
			.build()
			.and_then(|raw| raw.try_deserialize::<Config>());
		assert!(parsed.is_ok(), "config.example.toml should map onto Config: {parsed:?}");
	}

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
				cookie_secure: None,
			},
			frontend: FrontendConfig {
				url: "https://memory-map.example.test".to_string(),
			},
			cors: CorsConfig {
				allowed_origins: "https://memory-map.example.test".to_string(),
			},
			storage: StorageConfig {
				endpoint_url: "https://s3.example.test".to_string(),
				public_endpoint_url: Some("https://public-s3.example.test".to_string()),
				access_key: "debug-storage-access-secret".to_string(),
				secret_key: "debug-storage-secret-secret".to_string(),
				bucket_name: "memory-map".to_string(),
				region: "us-east-1".to_string(),
				force_path_style: false,
				presigned_url_ttl_seconds: 60,
			},
			object_lifecycle: ObjectLifecycleConfig::default(),
			email_outbox: EmailOutboxConfig::default(),
		};

		let debug = format!("{config:?}");

		assert!(debug.contains("Config"));
		assert!(debug.contains("smtp.example.test"));
		assert!(debug.contains("https://s3.example.test"));
		assert!(debug.contains("https://public-s3.example.test"));
		assert!(debug.contains("<redacted>"));
		assert!(!debug.contains("debug-smtp-pass-secret"));
		assert!(!debug.contains("debug-cookie-secret"));
		assert!(!debug.contains("debug-storage-access-secret"));
		assert!(!debug.contains("debug-storage-secret-secret"));
	}

	#[test]
	fn auth_config_validate_accepts_cookie_secret_boundary_length() {
		let config = AuthConfig {
			cookie_secret: "a".repeat(64),
			enable_registration: true,
			cookie_secure: None,
		};

		assert!(config.validate().is_ok());
	}

	#[test]
	fn auth_config_validate_rejects_short_cookie_secret() {
		let config = AuthConfig {
			cookie_secret: "a".repeat(63),
			enable_registration: true,
			cookie_secure: None,
		};

		let error = config.validate().err();
		assert!(
			error.as_ref().is_some_and(|error| error.to_string().contains("at least 64 bytes"))
		);
	}

	#[test]
	fn config_deserialization_defaults_missing_object_lifecycle_section() -> anyhow::Result<()> {
		let config: Config = serde_json::from_value(serde_json::json!({
			"pg": {},
			"server": {
				"host": "127.0.0.1",
				"port": 8000
			},
			"smtp": {
				"host": "smtp.example.test",
				"user": "smtp-user",
				"pass": "smtp-pass",
				"from": "noreply@example.test"
			},
			"auth": {
				"cookie_secret": "cookie-secret",
				"enable_registration": true
			},
			"frontend": {
				"url": "http://127.0.0.1:3000"
			},
			"cors": {
				"allowed_origins": "http://127.0.0.1:3000"
			},
			"storage": {
				"endpoint_url": "http://127.0.0.1:9000/",
				"access_key": "storage-access",
				"secret_key": "storage-secret",
				"bucket_name": "memory-map",
				"region": "us-east-1",
				"force_path_style": true,
				"presigned_url_ttl_seconds": 60
			}
		}))?;

		let default = ObjectLifecycleConfig::default();
		assert_eq!(
			config.object_lifecycle.pending_upload_timeout_seconds,
			default.pending_upload_timeout_seconds
		);
		assert_eq!(
			config.object_lifecycle.storage_deletion_max_attempts,
			default.storage_deletion_max_attempts
		);
		assert_eq!(
			config.email_outbox.worker_interval_seconds,
			EmailOutboxConfig::default().worker_interval_seconds
		);
		Ok(())
	}

	#[test]
	fn config_cookie_secure_defaults_from_frontend_url() {
		let mut config = test_config_with_frontend_url("https://memory-map.example.test");
		assert!(config.cookie_secure());

		config.frontend.url = "http://memory-map.example.test".to_string();
		assert!(!config.cookie_secure());
	}

	#[test]
	fn config_cookie_secure_uses_explicit_auth_override() {
		let mut config = test_config_with_frontend_url("http://memory-map.example.test");
		config.auth.cookie_secure = Some(true);
		assert!(config.cookie_secure());

		config.frontend.url = "https://memory-map.example.test".to_string();
		config.auth.cookie_secure = Some(false);
		assert!(!config.cookie_secure());
	}

	fn test_config_with_frontend_url(frontend_url: &str) -> Config {
		Config {
			pg: PostgresConfig::new(),
			server: ServerConfig {
				host: "127.0.0.1".to_string(),
				port: 8000,
			},
			smtp: SmtpConfig {
				host: "smtp.example.test".to_string(),
				user: "smtp-user".to_string(),
				pass: "smtp-pass".to_string(),
				from: "noreply@example.test".to_string(),
			},
			auth: AuthConfig {
				cookie_secret: "a".repeat(64),
				enable_registration: true,
				cookie_secure: None,
			},
			frontend: FrontendConfig {
				url: frontend_url.to_string(),
			},
			cors: CorsConfig {
				allowed_origins: frontend_url.to_string(),
			},
			storage: StorageConfig {
				endpoint_url: "http://127.0.0.1:9000/".to_string(),
				public_endpoint_url: None,
				access_key: "storage-access".to_string(),
				secret_key: "storage-secret".to_string(),
				bucket_name: "memory-map".to_string(),
				region: "us-east-1".to_string(),
				force_path_style: true,
				presigned_url_ttl_seconds: 60,
			},
			object_lifecycle: ObjectLifecycleConfig::default(),
			email_outbox: EmailOutboxConfig::default(),
		}
	}
}
