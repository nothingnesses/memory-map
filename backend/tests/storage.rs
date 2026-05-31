use {
	aws_sdk_s3::primitives::ByteStream,
	backend::storage::{
		StorageClient,
		StorageConfig,
	},
	std::{
		env,
		time::{
			Duration,
			SystemTime,
			UNIX_EPOCH,
		},
	},
	tokio::{
		net::TcpStream,
		time::timeout,
	},
};

#[tokio::test]
#[ignore = "requires an S3-compatible storage service"]
async fn storage_roundtrip_against_configured_service() -> anyhow::Result<()> {
	let config = storage_config_from_env()?;
	if !endpoint_is_reachable(&config.endpoint_url).await? {
		return skip_or_fail(format!("storage endpoint is not reachable: {}", config.endpoint_url));
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
		endpoint_url: env_or_default("S3_ENDPOINT_URL", "http://127.0.0.1:9000/"),
		access_key: env_or_default("S3_ACCESS_KEY", "memorymapdev"),
		secret_key: env_or_default("S3_SECRET_KEY", "memorymapdevsecret"),
		bucket_name: env_or_default("S3_BUCKET_NAME", "memory-map"),
		region: env_or_default("S3_REGION", "us-east-1"),
		force_path_style: parse_bool_env("S3_FORCE_PATH_STYLE", true)?,
		presigned_url_ttl_seconds: env_or_default("S3_PRESIGNED_URL_TTL_SECONDS", "604800")
			.parse()?,
	};
	config.validate()?;
	Ok(config)
}

async fn endpoint_is_reachable(endpoint_url: &str) -> anyhow::Result<bool> {
	let url = reqwest::Url::parse(endpoint_url)?;
	let host =
		url.host_str().ok_or_else(|| anyhow::anyhow!("S3 endpoint URL is missing a host"))?;
	let port = url
		.port_or_known_default()
		.ok_or_else(|| anyhow::anyhow!("S3 endpoint URL is missing a port"))?;
	Ok(matches!(timeout(Duration::from_secs(2), TcpStream::connect((host, port))).await, Ok(Ok(_))))
}

fn skip_or_fail(message: String) -> anyhow::Result<()> {
	if storage_service_required() {
		anyhow::bail!("{message}");
	}

	eprintln!("skipping storage integration test: {message}");
	Ok(())
}

fn storage_service_required() -> bool {
	env::var("STORAGE_TEST_REQUIRE_SERVICE")
		.map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
		.unwrap_or(false)
}

fn unique_prefix() -> anyhow::Result<String> {
	let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
	Ok(format!("storage-regression/{}", now.as_nanos()))
}

fn env_or_default(
	name: &str,
	default: &str,
) -> String {
	env::var(name).unwrap_or_else(|_| default.to_string())
}

fn parse_bool_env(
	name: &str,
	default: bool,
) -> anyhow::Result<bool> {
	let value = env_or_default(name, if default { "true" } else { "false" });
	match value.to_ascii_lowercase().as_str() {
		"1" | "true" | "yes" | "on" => Ok(true),
		"0" | "false" | "no" | "off" => Ok(false),
		_ => anyhow::bail!("{name} must be a boolean"),
	}
}
