use async_graphql::{EmptySubscription, Schema};
use async_graphql_axum::GraphQL;
use axum::{
	Router,
	body::{Body, to_bytes},
	extract::{DefaultBodyLimit, State},
	http::{HeaderMap, HeaderValue, Request, StatusCode},
	middleware::{self, Next},
	response::IntoResponse,
	routing::{delete, get, post},
};
use backend::{
	BODY_MAX_SIZE_LIMIT_BYTES, Config, SharedState,
	controllers::api::{
		locations::post as post_locations,
		s3_objects::{delete as delete_s3_object, delete_many as delete_s3_objects},
	},
	graphiql,
	graphql::queries::{mutation::Mutation, query::Query},
	migrations,
};
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
		Arc,
		atomic::{AtomicU64, Ordering},
	},
	time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tokio_postgres::NoTls;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

async fn caching_middleware(
	State(state): State<Arc<SharedState<Manager, Object<Manager>>>>,
	_headers: HeaderMap,
	request: Request<Body>,
	next: Next,
) -> impl IntoResponse {
	// 1. Read body
	let (parts, body) = request.into_parts();
	// Limit body size to avoid DoS. Using a reasonable limit for GraphQL queries (e.g. 1MB).
	let bytes = match to_bytes(body, 1024 * 1024).await {
		Ok(b) => b,
		Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
	};

	// Return early if it's a mutation.
	// Mutations change state and should not be cached. Caching them would prevent
	// the server from executing the mutation on subsequent requests.
	if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
		if let Some(query) = json.get("query").and_then(|q| q.as_str()) {
			if query.trim().to_lowercase().starts_with("mutation") {
				let req = Request::from_parts(parts, Body::from(bytes));
				return next.run(req).await.into_response();
			}
		}
	}

	// 2. Hash body
	let mut hasher = DefaultHasher::new();
	bytes.hash(&mut hasher);
	if let Some(auth_header) = parts.headers.get("Authorization") {
		auth_header.hash(&mut hasher);
	}
	let hash = hasher.finish();

	// 3. Check cache
	if let Some(cached_response) = state.response_cache.get(&hash).await {
		tracing::info!("Cache hit for hash: {}", hash);
		let mut response = axum::response::Response::new(Body::from(cached_response));
		response.headers_mut().insert("Content-Type", HeaderValue::from_static("application/json"));

		let last_modified = state.last_modified.load(Ordering::Relaxed);
		let etag = format!("\"{}\"", last_modified);
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

	state.response_cache.insert(hash, bytes.clone()).await;

	let mut response = axum::response::Response::from_parts(parts, Body::from(bytes));

	let last_modified = state.last_modified.load(Ordering::Relaxed);
	let etag = format!("\"{}\"", last_modified);
	response.headers_mut().insert("ETag", HeaderValue::from_str(&etag).unwrap());

	response
}

#[tokio::main]
async fn main() {
	// Initialise logging
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	// Read and parse dotenv config
	dotenv().ok();
	let cfg = Config::from_env().unwrap();

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

	let response_cache =
		Cache::builder().max_capacity(10000).time_to_live(Duration::from_secs(60)).build();

	let shared_state =
		Arc::new(SharedState { pool, minio_client, bucket_name, last_modified, response_cache });

	// Set up GraphQL
	let schema =
		Schema::build(Query, Mutation, EmptySubscription).data(shared_state.clone()).finish();

	let permissive_cors = CorsLayer::permissive();

	let app = Router::new()
		.route(
			"/",
			get(graphiql)
				.post_service(GraphQL::new(schema))
				.layer(middleware::from_fn_with_state(shared_state.clone(), caching_middleware)),
		)
		.route(
			"/api/locations/",
			post(post_locations)
				.route_layer(DefaultBodyLimit::max(BODY_MAX_SIZE_LIMIT_BYTES))
				.with_state(shared_state.clone()),
		)
		.route("/api/s3-objects/{id}", delete(delete_s3_object).with_state(shared_state.clone()))
		.route("/api/delete-s3-objects/", post(delete_s3_objects).with_state(shared_state.clone()))
		.route_layer(permissive_cors);

	println!("GraphiQL IDE: http://localhost:8000");

	axum::serve(TcpListener::bind("127.0.0.1:8000").await.unwrap(), app).await.unwrap();
}
