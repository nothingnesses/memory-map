use {
	anyhow::Context,
	backend::{
		Config,
		app::{
			build_app,
			build_shared_state,
		},
		email_worker::EmailWorker,
		migrations,
		object_lifecycle::ObjectLifecycleWorker,
		storage::StorageClient,
		worker,
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
	let cfg = Config::load().context("Failed to load configuration")?;

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

	let _object_lifecycle_worker = worker::spawn(ObjectLifecycleWorker::new(
		pool.clone(),
		storage.clone(),
		cfg.object_lifecycle.clone(),
	));
	let _email_worker = worker::spawn(EmailWorker::new(pool.clone(), cfg.clone()));

	// Initialise Casbin Enforcer
	let enforcer = Enforcer::new("authz_model.conf", "authz_policy.csv").await?;
	let enforcer = Arc::new(RwLock::new(enforcer));

	let bind_addr = format!("{}:{}", cfg.server.host, cfg.server.port);
	let shared_state = build_shared_state(cfg, pool, storage, enforcer);
	let app = build_app(shared_state);

	println!("GraphiQL IDE: http://{bind_addr}");

	axum::serve(TcpListener::bind(bind_addr).await.context("Failed to bind to address")?, app)
		.await
		.context("Failed to start server")?;

	Ok(())
}
