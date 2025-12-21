use async_graphql::{EmptySubscription, Schema};
use async_graphql_axum::GraphQL;
use axum::{
	Router,
	body::Body,
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
use std::{
	ops::DerefMut,
	sync::{
		Arc,
		atomic::{AtomicU64, Ordering},
	},
	time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tokio_postgres::NoTls;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

async fn etag_middleware(
	State(state): State<Arc<SharedState<Manager, Object<Manager>>>>,
	headers: HeaderMap,
	request: Request<Body>,
	next: Next,
) -> impl IntoResponse {
	let last_modified = state.last_modified.load(Ordering::Relaxed);
	let etag = format!("\"{}\"", last_modified);

	if let Some(if_none_match) = headers.get("if-none-match") {
		if if_none_match.to_str().unwrap_or("") == etag {
			return StatusCode::NOT_MODIFIED.into_response();
		}
	}

	let mut response = next.run(request).await;
	response.headers_mut().insert("ETag", HeaderValue::from_str(&etag).unwrap());
	response.headers_mut().insert("Cache-Control", HeaderValue::from_static("no-cache"));

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

	let shared_state = Arc::new(SharedState { pool, minio_client, bucket_name, last_modified });

	// Set up GraphQL
	let schema =
		Schema::build(Query, Mutation, EmptySubscription).data(shared_state.clone()).finish();

	let permissive_cors = CorsLayer::permissive();

	let app = Router::new()
		.route(
			"/",
			get(graphiql)
				.post_service(GraphQL::new(schema))
				.layer(middleware::from_fn_with_state(shared_state.clone(), etag_middleware)),
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
