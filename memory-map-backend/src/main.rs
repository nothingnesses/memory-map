use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use axum::response::{self, IntoResponse};
use axum::Router;
use deadpool::managed::{Manager, Object, Pool};
use deadpool_postgres::Runtime;
use dotenvy::dotenv;
use memory_map_backend::{Query, SchemaData};
use minio::s3::Client;
use minio::s3::response::BucketExistsResponse;
use minio::s3::types::S3Api;
use std::ops::DerefMut;
use tokio_postgres::NoTls;

#[derive(Debug, serde::Deserialize)]
struct Config {
	pg: deadpool_postgres::Config,
}

impl Config {
	pub fn from_env() -> Result<Self, config::ConfigError> {
		config::Config::builder()
			.add_source(config::Environment::default().separator("__"))
			.build()?
			.try_deserialize()
	}
}

refinery::embed_migrations!("migrations");

async fn graphiql() -> impl IntoResponse {
	response::Html(GraphiQLSource::build().endpoint("/").finish())
}

#[tokio::main]
async fn main() {
	// Read and parse dotenv config
	dotenv().ok();
	let cfg = Config::from_env().unwrap();

	// Connect to DB
	let pool = cfg.pg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();

	{
		let mut conn = pool.get().await.unwrap();
		let client = conn.deref_mut().deref_mut();

		// Run DB migrations
		migrations::runner().run_async(client).await.unwrap();
	}

	// Set up GraphQL
	let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
		.data(SchemaData {
			pool: pool
		})
		.finish();

	let result = schema.execute("{ locations { id latitude longitude } }").await;

	println!("{}", serde_json::to_string(&result).unwrap());
}
