use {
	anyhow::Context,
	backend::{
		Config,
		app::{
			build_app,
			build_shared_state,
		},
		migrations,
		object_lifecycle::ObjectLifecycleService,
		storage::StorageClient,
	},
	casbin::{
		CoreApi,
		Enforcer,
	},
	deadpool_postgres::Runtime,
	dotenvy::dotenv,
	std::{
		ops::DerefMut,
		sync::Arc,
	},
	tokio::{
		net::TcpListener,
		sync::RwLock,
	},
	tokio_postgres::NoTls,
	tracing_subscriber::EnvFilter,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// Initialise logging
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	// Read and parse dotenv config
	dotenv().ok();
	let cfg = Config::from_env().context("Failed to load configuration from environment")?;

	// Connect to DB
	let pool = cfg
		.pg
		.create_pool(Some(Runtime::Tokio1), NoTls)
		.context("Failed to create database pool")?;

	{
		let mut postgresql_connection =
			pool.get().await.context("Failed to get database connection from pool")?;
		let postgresql_client = postgresql_connection.deref_mut().deref_mut();

		// Run DB migrations
		migrations::runner()
			.run_async(postgresql_client)
			.await
			.context("Failed to run database migrations")?;
	}

	// Initialise S3 storage client
	tracing::info!(
		"S3 endpoint configured: {} (region: {}, force path-style: {})",
		cfg.storage.endpoint_url,
		cfg.storage.region,
		cfg.storage.force_path_style
	);
	let storage = StorageClient::from_config(&cfg).context("Failed to build S3 storage client")?;
	storage.verify_bucket_ready().await.context("Failed to verify S3 bucket readiness")?;

	if let Ok(mut cleanup_connection) = pool.get().await {
		let mut object_lifecycle = ObjectLifecycleService::new(&mut cleanup_connection, &storage);
		if let Err(error) = object_lifecycle.drain_storage_deletions().await {
			tracing::warn!(
				error = ?error,
				"Failed to drain pending object storage deletions during startup"
			);
		}
	}

	// Initialise Casbin Enforcer
	let enforcer = Enforcer::new("authz_model.conf", "authz_policy.csv").await?;
	let enforcer = Arc::new(RwLock::new(enforcer));

	let bind_addr = format!("{}:{}", cfg.server_host, cfg.server_port);
	let shared_state = build_shared_state(cfg, pool, storage, enforcer)?;
	let app = build_app(shared_state);

	println!("GraphiQL IDE: http://{bind_addr}");

	axum::serve(TcpListener::bind(bind_addr).await.context("Failed to bind to address")?, app)
		.await
		.context("Failed to start server")?;

	Ok(())
}
