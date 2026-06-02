mod common;

use {
	aws_sdk_s3::primitives::ByteStream,
	backend::storage::{
		StorageClient,
		StorageConfig,
	},
	common::{
		endpoint_is_reachable,
		env_or_default,
		parse_bool_env,
		skip_or_fail,
		unique_suffix,
	},
};

#[tokio::test]
#[ignore = "requires an S3-compatible storage service"]
async fn storage_roundtrip_against_configured_service() -> anyhow::Result<()> {
	let config = storage_config_from_env()?;
	if !endpoint_is_reachable(&config.endpoint_url).await? {
		return skip_or_fail(
			"storage integration test",
			format!("storage endpoint is not reachable: {}", config.endpoint_url),
			(),
		);
	}

	let storage = StorageClient::from_storage_config(&config)?;
	storage.ensure_bucket_exists().await?;

	let prefix = unique_prefix()?;
	let first_object = format!("{prefix}/first.txt");
	let second_object = format!("{prefix}/second.bin");
	let first_body = b"memory-map storage regression\n";
	let second_body = b"delete me too\n";

	storage.upload_object(&first_object, ByteStream::from_static(first_body), "text/plain").await?;
	storage
		.upload_object(
			&second_object,
			ByteStream::from_static(second_body),
			"application/octet-stream",
		)
		.await?;

	assert_eq!(storage.object_content_type(&first_object).await?, "text/plain");

	let url = storage.presigned_get_url(&first_object).await?;
	let body = reqwest::get(url).await?.error_for_status()?.bytes().await?;
	assert_eq!(body.as_ref(), first_body);

	storage.delete_objects(&[first_object.clone(), second_object.clone()]).await?;
	assert!(storage.object_content_type(&first_object).await.is_err());
	assert!(storage.object_content_type(&second_object).await.is_err());

	Ok(())
}

fn storage_config_from_env() -> anyhow::Result<StorageConfig> {
	let config = StorageConfig {
		endpoint_url: env_or_default("MEMORY_MAP__STORAGE__ENDPOINT_URL", "http://127.0.0.1:9000/"),
		public_endpoint_url: std::env::var("MEMORY_MAP__STORAGE__PUBLIC_ENDPOINT_URL").ok(),
		access_key: env_or_default("MEMORY_MAP__STORAGE__ACCESS_KEY", "memorymapdev"),
		secret_key: env_or_default("MEMORY_MAP__STORAGE__SECRET_KEY", "memorymapdevsecret"),
		bucket_name: env_or_default("MEMORY_MAP__STORAGE__BUCKET_NAME", "memory-map"),
		region: env_or_default("MEMORY_MAP__STORAGE__REGION", "us-east-1"),
		force_path_style: parse_bool_env("MEMORY_MAP__STORAGE__FORCE_PATH_STYLE", true)?,
		presigned_url_ttl_seconds: env_or_default(
			"MEMORY_MAP__STORAGE__PRESIGNED_URL_TTL_SECONDS",
			"604800",
		)
		.parse()?,
	};
	config.validate()?;
	Ok(config)
}

fn unique_prefix() -> anyhow::Result<String> {
	Ok(format!("storage-regression/{}", unique_suffix()?))
}
