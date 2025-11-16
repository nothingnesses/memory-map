use async_graphql::{EmptySubscription, Schema};
use async_graphql_axum::GraphQL;
use axum::Router;
use axum::routing::get;
use backend::graphql::SchemaData;
use backend::graphql::queries::mutation::Mutation;
use backend::graphql::queries::query::Query;
use backend::{Config, graphiql, migrations};
use deadpool_postgres::Runtime;
use dotenvy::dotenv;
use minio::s3::ClientBuilder;
use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;
use std::ops::DerefMut;
use tokio::net::TcpListener;
use tokio_postgres::NoTls;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
	// Initialise logging
	tracing_subscriber::fmt::init();

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

	let bucket_name = "memory-map";

	// Set up GraphQL
	let schema = Schema::build(Query, Mutation, EmptySubscription)
		.data(SchemaData { bucket_name: bucket_name.to_string(), pool, minio_client })
		.finish();

	let app = Router::new()
		.route("/", get(graphiql).post_service(GraphQL::new(schema)))
		.layer(CorsLayer::permissive());

	println!("GraphiQL IDE: http://localhost:8000");

	axum::serve(TcpListener::bind("127.0.0.1:8000").await.unwrap(), app).await.unwrap();
}
