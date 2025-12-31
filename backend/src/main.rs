use async_graphql::{EmptySubscription, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
	Router,
	body::{Body, to_bytes},
	extract::{DefaultBodyLimit, Extension, State},
	http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header, request::Parts},
	middleware::{self, Next},
	response::IntoResponse,
	routing::{delete, get, post},
};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar};
use backend::{
	AppState, BODY_MAX_SIZE_LIMIT_BYTES, Config, SharedState, UserId,
	controllers::api::{
		locations::post as post_locations,
	},
	graphiql,
	graphql::queries::{mutation::Mutation, query::Query},
	migrations,
};
use casbin::{CoreApi, Enforcer};
use deadpool::managed::Object;
use deadpool_postgres::{Manager, Runtime};
use dotenvy::dotenv;
use minio::s3::{ClientBuilder, creds::StaticProvider, http::BaseUrl};
use moka::future::Cache;
use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	ops::DerefMut,
	sync::{
		Arc, Mutex,
		atomic::{AtomicU64, Ordering},
	},
	time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, sync::RwLock};
use tokio_postgres::NoTls;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::EnvFilter;

// Amount of bytes to cache.
const CACHE_MAX_CAPACITY: u64 = 10_000;
// Cache time-to-live duration in seconds. Currently 10 minutes.
const CACHE_TTL_SECONDS: u64 = 600;
// Max body size for GraphQL queries (1MB).
const GRAPHQL_BODY_LIMIT_BYTES: usize = 1024 * 1024;

async fn caching_middleware(
	State(state): State<AppState<Manager, Object<Manager>>>,
	_headers: HeaderMap,
	request: Request<Body>,
	next: Next,
) -> impl IntoResponse {
	// 1. Read body
	let (parts, body) = request.into_parts();
	// Limit body size to avoid DoS.
	let bytes = match to_bytes(body, GRAPHQL_BODY_LIMIT_BYTES).await {
		Ok(b) => b,
		Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
	};

	// Return early if it's a mutation.
	// Mutations change state and should not be cached. Caching them would prevent
	// the server from executing the mutation on subsequent requests.
	if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes)
		&& let Some(query) = json.get("query").and_then(|q| q.as_str())
		&& query.trim().to_lowercase().starts_with("mutation")
	{
		let req = Request::from_parts(parts, Body::from(bytes));
		return next.run(req).await.into_response();
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
		response.headers_mut().insert("ETag", HeaderValue::from_str(&etag).unwrap());

		return response;
	}

	// 4. Process request
	let req = Request::from_parts(parts, Body::from(bytes));
	let response = next.run(req).await;

	// 5. Cache response
	let (parts, body) = response.into_parts();
	let bytes = match to_bytes(body, BODY_MAX_SIZE_LIMIT_BYTES).await {
		Ok(b) => b,
		Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
	};

	state.inner.response_cache.insert(hash, bytes.clone()).await;

	let mut response = axum::response::Response::from_parts(parts, Body::from(bytes));

	let last_modified = state.inner.last_modified.load(Ordering::Relaxed);
	let etag = format!("\"{last_modified}\"");
	response.headers_mut().insert("ETag", HeaderValue::from_str(&etag).unwrap());

	response
}

async fn graphql_handler(
	State(state): State<AppState<Manager, Object<Manager>>>,
	Extension(schema): Extension<Schema<Query, Mutation, EmptySubscription>>,
	jar: PrivateCookieJar,
	req: GraphQLRequest,
) -> (PrivateCookieJar, GraphQLResponse) {
	let mut req = req.into_inner();

	if let Some(cookie) = jar.get("auth_token")
		&& let Ok(user_id) = cookie.value().parse::<i64>()
	{
		// Verify user exists in database
		let user_exists = if let Ok(client) = state.inner.pool.get().await {
			match client.prepare_cached("SELECT 1 FROM users WHERE id = $1").await {
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
async fn main() {
	// Initialise logging
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	// Read and parse dotenv config
	dotenv().ok();
	let cfg = Config::from_env().unwrap();
	let frontend_url = cfg.frontend_url.clone();

	// Connect to DB
	let pool = cfg.pg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();

	{
		let mut postgresql_connection = pool.get().await.unwrap();
		let postgresql_client = postgresql_connection.deref_mut().deref_mut();

		// Run DB migrations
		migrations::runner().run_async(postgresql_client).await.unwrap();
	}

	// Initialise minio client
	let base_url = "http://localhost:9000/".parse::<BaseUrl>().unwrap();
	tracing::info!("Trying to connect to MinIO at: `{:?}`", base_url);

	let static_provider = StaticProvider::new("minioadmin", "minioadmin", None);

	let minio_client =
		ClientBuilder::new(base_url).provider(Some(Box::new(static_provider))).build().unwrap();

	let bucket_name = "memory-map".to_string();

	let last_modified =
		AtomicU64::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64);

	let response_cache = Cache::builder()
		.max_capacity(CACHE_MAX_CAPACITY)
		.time_to_live(Duration::from_secs(CACHE_TTL_SECONDS))
		.build();

	// Initialise Casbin Enforcer
	let enforcer = Enforcer::new("authz_model.conf", "authz_policy.csv").await.unwrap();
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
		config: cfg,
		enforcer,
	});

	let app_state = AppState { inner: shared_state.clone() };

	let schema = Schema::build(Query, Mutation, EmptySubscription)
		.data(shared_state.clone())
		.data(key.clone())
		.finish();

	let cors = CorsLayer::new()
		.allow_origin(AllowOrigin::predicate(
			move |origin: &HeaderValue, _request_parts: &Parts| {
				let origin_bytes = origin.as_bytes();
				origin_bytes == frontend_url.as_bytes() || origin_bytes == b"http://127.0.0.1:3000"
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

	println!("GraphiQL IDE: http://localhost:8000");

	axum::serve(TcpListener::bind("127.0.0.1:8000").await.unwrap(), app).await.unwrap();
}
