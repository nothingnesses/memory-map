use async_graphql::{EmptySubscription, Schema};
use async_graphql_axum::GraphQL;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use backend::controllers::api::locations::post as post_locations;
use backend::graphql::queries::mutation::Mutation;
use backend::graphql::queries::query::Query;
use backend::{Config, ONE_GB, SharedState, graphiql, migrations};
use deadpool_postgres::Runtime;
use dotenvy::dotenv;
use minio::s3::ClientBuilder;
use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;
use std::ops::DerefMut;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_postgres::NoTls;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

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

	let shared_state = Arc::new(SharedState { pool, minio_client, bucket_name });

	// Set up GraphQL
	let schema =
		Schema::build(Query, Mutation, EmptySubscription).data(shared_state.clone()).finish();

	let permissive_cors = CorsLayer::permissive();

	let app = Router::new()
		.route("/", get(graphiql).post_service(GraphQL::new(schema)))
		.route(
			"/api/locations/",
			post(post_locations)
				.route_layer(DefaultBodyLimit::max(ONE_GB))
				.with_state(shared_state.clone()),
		)
		.route_layer(permissive_cors);

	println!("GraphiQL IDE: http://localhost:8000");

	axum::serve(TcpListener::bind("127.0.0.1:8000").await.unwrap(), app).await.unwrap();
}
