use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use async_graphql_axum::GraphQL;
use axum::Router;
use axum::http::Method;
use axum::routing::get;
use deadpool_postgres::Runtime;
use dotenvy::dotenv;
use futures_util::StreamExt;
use memory_map_backend::{Config, Mutation, Query, SchemaData, graphiql, migrations};
use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;
use minio::s3::response::BucketExistsResponse;
use minio::s3::types::{S3Api, ToStream};
use minio::s3::{Client, ClientBuilder};
use std::ops::DerefMut;
use tokio::net::TcpListener;
use tokio_postgres::NoTls;

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

	let minio_client = ClientBuilder::new(base_url.clone())
		.provider(Some(Box::new(static_provider)))
		.build()
		.unwrap();

	let bucket = "memory-map";

	let mut resp =
		minio_client.list_objects(bucket).recursive(true).include_versions(true).to_stream().await;

	while let Some(result) = resp.next().await {
		match result {
			Ok(resp) => {
				for item in resp.contents {
					println!("list_entry: {:?}", item);
					println!("get_object: {:?}", minio_client.get_object(bucket, &item.name));
					println!(
						"url: {:?}",
						minio_client
							.get_presigned_object_url(bucket, &item.name, Method::GET)
							.send()
							.await
							.unwrap()
							.url
					);
				}
			}
			Err(e) => println!("Error: {:?}", e),
		}
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
