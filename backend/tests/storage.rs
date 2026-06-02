mod common;

use {
	aws_sdk_s3::primitives::ByteStream,
	backend::storage::{
		StorageClient,
		StorageConfig,
	},
	common::{
		endpoint_is_reachable,
		skip_or_fail,
		unique_suffix,
	},
};

#[tokio::test]
#[ignore = "requires an S3-compatible storage service"]
async fn storage_roundtrip_against_configured_service() -> anyhow::Result<()> {
	let config = match StorageConfig::from_env() {
		Ok(config) => config,
		Err(error) =>
			return skip_or_fail(
				"storage integration test",
				format!("storage config is unavailable: {error:#}"),
				(),
			),
	};
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

fn unique_prefix() -> anyhow::Result<String> {
	Ok(format!("storage-regression/{}", unique_suffix()?))
}
