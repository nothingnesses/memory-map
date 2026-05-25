use {
	anyhow::Context,
	async_graphql::{
		EmptySubscription,
		Schema,
	},
	async_graphql_axum::{
		GraphQLRequest,
		GraphQLResponse,
	},
	axum::{
		Router,
		body::{
			Body,
			to_bytes,
		},
		extract::{
			DefaultBodyLimit,
			Extension,
			State,
		},
		http::{
			HeaderMap,
			HeaderValue,
			Method,
			Request,
			StatusCode,
			header,
			request::Parts,
		},
		middleware::{
			self,
			Next,
		},
		response::IntoResponse,
		routing::{
			get,
			post,
		},
	},
	axum_extra::extract::cookie::{
		Cookie,
		Key,
		PrivateCookieJar,
	},
	backend::{
		AppState,
		Config,
		SharedState,
		UserId,
		constants::{
			BODY_MAX_SIZE_LIMIT_BYTES,
			CACHE_MAX_CAPACITY,
			CACHE_TTL_SECONDS,
			GRAPHQL_BODY_LIMIT_BYTES,
		},
		controllers::api::locations::post as post_locations,
		db::queries::SELECT_USER_EXISTS_QUERY,
		graphiql,
		graphql::queries::{
			mutation::Mutation,
			query::Query,
		},
		migrations,
	},
	casbin::{
		CoreApi,
		Enforcer,
	},
	deadpool::managed::Object,
	deadpool_postgres::{
		Manager,
		Runtime,
	},
	dotenvy::dotenv,
	minio::s3::{
		ClientBuilder,
		creds::StaticProvider,
		http::BaseUrl,
	},
	moka::future::Cache,
	std::{
		collections::hash_map::DefaultHasher,
		hash::{
			Hash,
			Hasher,
		},
		ops::DerefMut,
		sync::{
			Arc,
			Mutex,
			atomic::{
				AtomicU64,
				Ordering,
			},
		},
		time::{
			Duration,
			SystemTime,
			UNIX_EPOCH,
		},
	},
	tokio::{
		net::TcpListener,
		sync::RwLock,
	},
	tokio_postgres::NoTls,
	tower_http::cors::{
		AllowOrigin,
		CorsLayer,
	},
	tracing_subscriber::EnvFilter,
};

async fn caching_middleware(
	State(state): State<AppState<Manager, Object<Manager>>>,
	_headers: HeaderMap,
	request: Request<Body>,
	next: Next,
) -> axum::response::Result<impl IntoResponse> {
	// 1. Read body
	let (parts, body) = request.into_parts();
	// Limit body size to avoid DoS.
	let bytes = match to_bytes(body, GRAPHQL_BODY_LIMIT_BYTES).await {
		Ok(b) => b,
		Err(_) => return Ok(StatusCode::PAYLOAD_TOO_LARGE.into_response()),
	};

	// Return early if it's a mutation.
	// Mutations change state and should not be cached. Caching them would prevent
	// the server from executing the mutation on subsequent requests.
	if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) &&
		let Some(query) = json.get("query").and_then(|q| q.as_str()) &&
		query.trim().to_lowercase().starts_with("mutation")
	{
		let req = Request::from_parts(parts, Body::from(bytes));
		return Ok(next.run(req).await.into_response());
	}

	// 2. Hash body
	let mut hasher = DefaultHasher::new();
	bytes.hash(&mut hasher);
	if let Some(auth_header) = parts.headers.get("Authorization") {
		auth_header.hash(&mut hasher);
	}
	if let Some(cookie_header) = parts.headers.get(header::COOKIE) {
		cookie_header.hash(&mut hasher);
	}
	let hash = hasher.finish();

	// 3. Check cache
	if let Some(cached_response) = state.inner.response_cache.get(&hash).await {
		tracing::info!("Cache hit for hash: {}", hash);
		let mut response = axum::response::Response::new(Body::from(cached_response));
		response.headers_mut().insert("Content-Type", HeaderValue::from_static("application/json"));

		let last_modified = state.inner.last_modified.load(Ordering::Relaxed);
		let etag = format!("\"{last_modified}\"");
		response.headers_mut().insert(
			"ETag",
			HeaderValue::from_str(&etag).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
		);

		return Ok(response);
	}

	// 4. Process request
	let req = Request::from_parts(parts, Body::from(bytes));
	let response = next.run(req).await;

	// 5. Cache response
	let (parts, body) = response.into_parts();
	let bytes = match to_bytes(body, BODY_MAX_SIZE_LIMIT_BYTES).await {
		Ok(b) => b,
		Err(_) => return Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
	};

	state.inner.response_cache.insert(hash, bytes.clone()).await;

	let mut response = axum::response::Response::from_parts(parts, Body::from(bytes));

	let last_modified = state.inner.last_modified.load(Ordering::Relaxed);
	let etag = format!("\"{last_modified}\"");
	response.headers_mut().insert(
		"ETag",
		HeaderValue::from_str(&etag).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
	);

	Ok(response)
}

async fn graphql_handler(
	State(state): State<AppState<Manager, Object<Manager>>>,
	Extension(schema): Extension<Schema<Query, Mutation, EmptySubscription>>,
	jar: PrivateCookieJar,
	req: GraphQLRequest,
) -> (PrivateCookieJar, GraphQLResponse) {
	let mut req = req.into_inner();

	if let Some(cookie) = jar.get("auth_token") &&
		let Ok(user_id) = cookie.value().parse::<i64>()
	{
		// Verify user exists in database
		let user_exists = if let Ok(client) = state.inner.pool.get().await {
			match client.prepare_cached(SELECT_USER_EXISTS_QUERY).await {
				Ok(stmt) => matches!(client.query_opt(&stmt, &[&user_id]).await, Ok(Some(_))),
				Err(_) => false,
			}
		} else {
			false
		};

		if user_exists {
			req = req.data(UserId(user_id));
		}
	}

	let cookies = Arc::new(Mutex::new(Vec::<Cookie<'static>>::new()));
	req = req.data(cookies.clone());

	let response = schema.execute(req).await;

	let mut jar = jar;
	if let Ok(cookies) = cookies.lock() {
		for cookie in cookies.iter() {
			jar = jar.add(cookie.clone());
		}
	}

	(jar, response.into())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// Initialise logging
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	// Read and parse dotenv config
	dotenv().ok();
	let cfg = Config::from_env().context("Failed to load configuration from environment")?;
	let frontend_url = cfg.frontend_url.clone();
	let cors_allowed_origins = cfg.cors_allowed_origins.clone();

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

	// Initialise minio client
	let base_url = cfg.minio_url.parse::<BaseUrl>().context("Failed to parse MinIO URL")?;
	tracing::info!("Trying to connect to MinIO at: `{:?}`", base_url);

	let static_provider = StaticProvider::new(&cfg.minio_access_key, &cfg.minio_secret_key, None);

	let minio_client = ClientBuilder::new(base_url)
		.provider(Some(Box::new(static_provider)))
		.build()
		.context("Failed to build MinIO client")?;

	let bucket_name = cfg.s3_bucket_name.clone();

	let last_modified = AtomicU64::new(
		SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.context("System time is before UNIX EPOCH")?
			.as_millis() as u64,
	);

	let response_cache = Cache::builder()
		.max_capacity(CACHE_MAX_CAPACITY)
		.time_to_live(Duration::from_secs(CACHE_TTL_SECONDS))
		.build();

	// Initialise Casbin Enforcer
	let enforcer = Enforcer::new("authz_model.conf", "authz_policy.csv").await?;
	let enforcer = Arc::new(RwLock::new(enforcer));

	// Set up GraphQL
	let key = Key::from(cfg.cookie_secret.as_bytes());
	let shared_state = Arc::new(SharedState {
		pool,
		minio_client,
		bucket_name,
		last_modified,
		response_cache,
		key: key.clone(),
		config: cfg.clone(),
		enforcer,
	});

	let app_state = AppState {
		inner: shared_state.clone(),
	};

	let schema = Schema::build(Query, Mutation, EmptySubscription)
		.data(shared_state.clone())
		.data(key.clone())
		.finish();

	let cors = CorsLayer::new()
		.allow_origin(AllowOrigin::predicate(
			move |origin: &HeaderValue, _request_parts: &Parts| {
				let origin_bytes = origin.as_bytes();
				origin_bytes == frontend_url.as_bytes() ||
					origin_bytes == cors_allowed_origins.as_bytes()
			},
		))
		.allow_methods([Method::GET, Method::POST])
		.allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
		.allow_credentials(true);

	let app = Router::new()
		.route("/", get(graphiql))
		.route(
			"/",
			post(graphql_handler)
				.with_state(app_state.clone())
				.layer(middleware::from_fn_with_state(app_state.clone(), caching_middleware)),
		)
		.route(
			"/api/locations/",
			post(post_locations)
				.route_layer(DefaultBodyLimit::max(BODY_MAX_SIZE_LIMIT_BYTES))
				.with_state(app_state.clone()),
		)
		.layer(Extension(schema))
		.layer(Extension(key))
		.route_layer(cors);

	let bind_addr = format!("{}:{}", cfg.server_host, cfg.server_port);
	println!("GraphiQL IDE: http://{bind_addr}");

	axum::serve(TcpListener::bind(bind_addr).await.context("Failed to bind to address")?, app)
		.await
		.context("Failed to start server")?;

	Ok(())
}
