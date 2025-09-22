use deadpool_postgres::Runtime;
use dotenvy::dotenv;
use std::ops::DerefMut;
use tokio_postgres::NoTls;use minio::s3::Client;
use minio::s3::types::S3Api;
use minio::s3::response::BucketExistsResponse;

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

#[tokio::main]
async fn main() {
	dotenv().ok();
	let cfg = Config::from_env().unwrap();
	let pool = cfg.pg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
	let mut conn = pool.get().await.unwrap();
	let client = conn.deref_mut().deref_mut();
	migrations::runner().run_async(client).await.unwrap();
}
