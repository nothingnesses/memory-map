use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use async_graphql_axum::GraphQL;
use axum::Router;
use axum::routing::get;
use deadpool_postgres::Runtime;
use dotenvy::dotenv;
use memory_map_backend::{graphiql, migrations, Config, Mutation, Query, SchemaData};
use minio::s3::Client;
use minio::s3::response::BucketExistsResponse;
use minio::s3::types::S3Api;
use std::ops::DerefMut;
use tokio::net::TcpListener;
use tokio_postgres::NoTls;

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
	let schema =
		Schema::build(Query, Mutation, EmptySubscription).data(SchemaData { pool }).finish();

	// let result = schema.execute("{ locations { id latitude longitude } }").await;

	// println!("{}", serde_json::to_string(&result).unwrap());

	let app = Router::new().route("/", get(graphiql).post_service(GraphQL::new(schema)));

	println!("GraphiQL IDE: http://localhost:8000");

	axum::serve(TcpListener::bind("127.0.0.1:8000").await.unwrap(), app).await.unwrap();
}
