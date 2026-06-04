use {
	crate::Config,
	anyhow::Context,
	aws_credential_types::Credentials,
	aws_sdk_s3::{
		Client,
		config::{
			BehaviorVersion,
			Region,
		},
		error::{
			ProvideErrorMetadata,
			SdkError,
		},
		operation::{
			abort_multipart_upload::AbortMultipartUploadError,
			complete_multipart_upload::CompleteMultipartUploadError,
			create_bucket::CreateBucketError,
			head_bucket::HeadBucketError,
			head_object::HeadObjectError,
		},
		presigning::PresigningConfig,
		primitives::ByteStream,
		types::{
			CompletedMultipartUpload,
			CompletedPart,
			CorsConfiguration,
			CorsRule,
			Delete,
			ObjectIdentifier,
		},
	},
	serde::Deserialize,
	std::{
		fmt,
		time::Duration,
	},
	tokio::time::{
		Instant,
		sleep,
	},
};

#[derive(Clone, Deserialize)]
pub struct StorageConfig {
	pub endpoint_url: String,
	#[serde(default)]
	pub public_endpoint_url: Option<String>,
	pub access_key: String,
	pub secret_key: String,
	pub bucket_name: String,
	#[serde(default = "StorageConfig::default_region")]
	pub region: String,
	#[serde(default = "StorageConfig::default_force_path_style")]
	pub force_path_style: bool,
	#[serde(default = "StorageConfig::default_presigned_url_ttl_seconds")]
	pub presigned_url_ttl_seconds: u64,
}

impl fmt::Debug for StorageConfig {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("StorageConfig")
			.field("endpoint_url", &self.endpoint_url)
			.field("public_endpoint_url", &self.public_endpoint_url)
			.field("access_key", &"<redacted>")
			.field("secret_key", &"<redacted>")
			.field("bucket_name", &self.bucket_name)
			.field("region", &self.region)
			.field("force_path_style", &self.force_path_style)
			.field("presigned_url_ttl_seconds", &self.presigned_url_ttl_seconds)
			.finish()
	}
}

impl StorageConfig {
	pub const MAX_PRESIGNED_URL_TTL_SECONDS: u64 = 604_800;

	pub fn default_region() -> String {
		"us-east-1".to_string()
	}

	pub const fn default_force_path_style() -> bool {
		true
	}

	pub const fn default_presigned_url_ttl_seconds() -> u64 {
		604_800
	}

	/// Loads just the storage section from the environment for binaries that don't
	/// need the rest of the config (e.g. the storage bootstrap binary). Uses the same
	/// `MEMORY_MAP__STORAGE__*` keys as the main config.
	pub fn from_env() -> anyhow::Result<Self> {
		let raw = config::Config::builder()
			.add_source(
				config::Environment::with_prefix("MEMORY_MAP__STORAGE")
					.prefix_separator("__")
					.separator("__"),
			)
			.build()
			.context("Failed to read storage config from environment")?;
		let config: StorageConfig = raw
			.try_deserialize()
			.context("Failed to deserialize storage config from environment")?;
		config.validate()?;
		Ok(config)
	}

	pub fn validate(&self) -> anyhow::Result<()> {
		if let Some(public_endpoint_url) = &self.public_endpoint_url &&
			public_endpoint_url.trim().is_empty()
		{
			anyhow::bail!("storage.public_endpoint_url must not be empty when configured");
		}
		if !(1 ..= Self::MAX_PRESIGNED_URL_TTL_SECONDS).contains(&self.presigned_url_ttl_seconds) {
			anyhow::bail!(
				"s3_presigned_url_ttl_seconds must be between 1 and {}",
				Self::MAX_PRESIGNED_URL_TTL_SECONDS
			);
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresignedHeader {
	pub name: String,
	pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresignedUploadPart {
	pub url: String,
	pub method: String,
	pub headers: Vec<PresignedHeader>,
	pub expected_content_length: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedUploadPart {
	pub part_number: i32,
	pub e_tag: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredObjectMetadata {
	pub content_length: i64,
	pub content_type: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultipartUploadCompleteOutcome {
	Completed,
	UploadNotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultipartUploadAbortOutcome {
	Aborted,
	UploadNotFound,
}

#[derive(Clone)]
pub struct StorageClient {
	client: Client,
	presigning_client: Client,
	bucket_name: String,
	presigning_config: PresigningConfig,
}

impl StorageClient {
	/// S3 caps multi-object delete requests at 1000 keys per call. This is an internal
	/// concern of the storage layer; higher levels pass arbitrary key counts and the
	/// chunking happens inside `delete_objects`.
	const MAX_DELETE_OBJECTS_PER_REQUEST: usize = 1000;

	pub fn from_config(config: &Config) -> anyhow::Result<Self> {
		Self::from_storage_config(&config.storage)
	}

	pub fn from_storage_config(config: &StorageConfig) -> anyhow::Result<Self> {
		config.validate()?;
		let client_config = s3_client_config(config, &config.endpoint_url);
		let presigning_endpoint_url =
			config.public_endpoint_url.as_deref().unwrap_or(&config.endpoint_url);
		let presigning_client_config = s3_client_config(config, presigning_endpoint_url);
		let presigning_config =
			PresigningConfig::expires_in(Duration::from_secs(config.presigned_url_ttl_seconds))
				.context("Failed to configure S3 presigned URL expiry")?;

		Ok(Self {
			client: Client::from_conf(client_config),
			presigning_client: Client::from_conf(presigning_client_config),
			bucket_name: config.bucket_name.clone(),
			presigning_config,
		})
	}

	/// Uploads an object directly. Now exercised only by the integration tests
	/// (the production upload path is presigned multipart, driven by the client);
	/// kept as a public storage primitive and test seam.
	pub async fn upload_object(
		&self,
		storage_key: &str,
		bytes: impl Into<ByteStream>,
		content_type: impl Into<String>,
	) -> anyhow::Result<()> {
		self.client
			.put_object()
			.bucket(&self.bucket_name)
			.key(storage_key)
			.body(bytes.into())
			.content_type(content_type.into())
			.send()
			.await
			.context("Failed to upload object to S3 storage")?;
		Ok(())
	}

	/// Reads a stored object's content type. Now exercised only by the integration
	/// tests (production reads metadata via [`Self::head_object`]); kept as a public
	/// storage primitive and test seam.
	pub async fn object_content_type(
		&self,
		storage_key: &str,
	) -> anyhow::Result<String> {
		Ok(self.head_object(storage_key).await?.content_type)
	}

	/// Presigns a GET for `storage_key`. When `content_disposition` is set it is
	/// signed into the URL as `response-content-disposition`, so the storage
	/// response carries that header (used to force `attachment` for script-capable
	/// types like SVG; see the object resolver).
	pub async fn presigned_get_url(
		&self,
		storage_key: &str,
		content_disposition: Option<&str>,
	) -> anyhow::Result<String> {
		let request = self
			.presigning_client
			.get_object()
			.bucket(&self.bucket_name)
			.key(storage_key)
			.set_response_content_disposition(content_disposition.map(str::to_string))
			.presigned(self.presigning_config.clone())
			.await
			.context("Failed to generate S3 presigned GET URL")?;
		Ok(request.uri().to_string())
	}

	pub async fn create_multipart_upload(
		&self,
		storage_key: &str,
		content_type: &str,
	) -> anyhow::Result<String> {
		let output = self
			.client
			.create_multipart_upload()
			.bucket(&self.bucket_name)
			.key(storage_key)
			.content_type(content_type)
			.send()
			.await
			.context("Failed to create S3 multipart upload")?;
		output
			.upload_id()
			.map(str::to_string)
			.context("S3 multipart upload response did not include an upload id")
	}

	pub async fn presigned_upload_part_url(
		&self,
		storage_key: &str,
		upload_id: &str,
		part_number: i32,
		expected_content_length: i64,
	) -> anyhow::Result<PresignedUploadPart> {
		let request = self
			.presigning_client
			.upload_part()
			.bucket(&self.bucket_name)
			.key(storage_key)
			.upload_id(upload_id)
			.part_number(part_number)
			.content_length(expected_content_length)
			.presigned(self.presigning_config.clone())
			.await
			.context("Failed to generate S3 presigned upload-part URL")?;
		let headers = request
			.headers()
			.filter(|(name, _value)| browser_can_set_header(name))
			.map(|(name, value)| PresignedHeader {
				name: name.to_string(),
				value: value.to_string(),
			})
			.collect();

		Ok(PresignedUploadPart {
			url: request.uri().to_string(),
			method: request.method().to_string(),
			headers,
			expected_content_length,
		})
	}

	pub async fn complete_multipart_upload(
		&self,
		storage_key: &str,
		upload_id: &str,
		completed_parts: &[CompletedUploadPart],
	) -> anyhow::Result<MultipartUploadCompleteOutcome> {
		let completed_parts = completed_parts
			.iter()
			.map(|part| {
				CompletedPart::builder()
					.part_number(part.part_number)
					.e_tag(part.e_tag.clone())
					.build()
			})
			.collect::<Vec<_>>();
		let upload = CompletedMultipartUpload::builder().set_parts(Some(completed_parts)).build();

		match self
			.client
			.complete_multipart_upload()
			.bucket(&self.bucket_name)
			.key(storage_key)
			.upload_id(upload_id)
			.multipart_upload(upload)
			.send()
			.await
		{
			Ok(_) => Ok(MultipartUploadCompleteOutcome::Completed),
			Err(error) if complete_multipart_upload_error_is_no_such_upload(&error) =>
				Ok(MultipartUploadCompleteOutcome::UploadNotFound),
			Err(error) => Err(error).context("Failed to complete S3 multipart upload"),
		}
	}

	pub async fn abort_multipart_upload(
		&self,
		storage_key: &str,
		upload_id: &str,
	) -> anyhow::Result<MultipartUploadAbortOutcome> {
		match self
			.client
			.abort_multipart_upload()
			.bucket(&self.bucket_name)
			.key(storage_key)
			.upload_id(upload_id)
			.send()
			.await
		{
			Ok(_) => Ok(MultipartUploadAbortOutcome::Aborted),
			Err(error) if abort_multipart_upload_error_is_no_such_upload(&error) =>
				Ok(MultipartUploadAbortOutcome::UploadNotFound),
			Err(error) => Err(error).context("Failed to abort S3 multipart upload"),
		}
	}

	pub async fn head_object(
		&self,
		storage_key: &str,
	) -> anyhow::Result<StoredObjectMetadata> {
		self.head_object_opt(storage_key)
			.await?
			.with_context(|| format!("S3 object not found: {storage_key}"))
	}

	pub async fn head_object_opt(
		&self,
		storage_key: &str,
	) -> anyhow::Result<Option<StoredObjectMetadata>> {
		let output =
			match self.client.head_object().bucket(&self.bucket_name).key(storage_key).send().await
			{
				Ok(output) => output,
				Err(error) if head_object_error_is_not_found(&error) => return Ok(None),
				Err(error) => return Err(error).context("Failed to read S3 object metadata"),
			};
		let content_length =
			output.content_length().context("S3 object response did not include Content-Length")?;
		let content_type = output
			.content_type()
			.map(str::to_string)
			.context("S3 object response did not include Content-Type")?;

		Ok(Some(StoredObjectMetadata {
			content_length,
			content_type,
		}))
	}

	pub async fn delete_objects(
		&self,
		storage_keys: &[String],
	) -> anyhow::Result<()> {
		for storage_keys in storage_key_delete_batches(storage_keys) {
			self.delete_object_batch(storage_keys).await?;
		}
		Ok(())
	}

	async fn delete_object_batch(
		&self,
		storage_keys: &[String],
	) -> anyhow::Result<()> {
		let mut objects = Vec::with_capacity(storage_keys.len());
		for storage_key in storage_keys {
			objects.push(
				ObjectIdentifier::builder()
					.key(storage_key)
					.build()
					.context("Failed to build S3 delete object identifier")?,
			);
		}
		let delete = Delete::builder()
			.set_objects(Some(objects))
			.build()
			.context("Failed to build S3 delete request")?;

		let output = self
			.client
			.delete_objects()
			.bucket(&self.bucket_name)
			.delete(delete)
			.send()
			.await
			.context("Failed to delete objects from S3 storage")?;
		let errors = output.errors();
		if !errors.is_empty() {
			let details = errors
				.iter()
				.take(3)
				.map(|error| {
					let message =
						error.message().map(|message| format!(" ({message})")).unwrap_or_default();
					format!(
						"{}: {}{}",
						error.key().unwrap_or("<unknown key>"),
						error.code().unwrap_or("unknown error"),
						message
					)
				})
				.collect::<Vec<_>>()
				.join("; ");
			anyhow::bail!("S3 reported {} failed object delete(s): {}", errors.len(), details);
		}
		Ok(())
	}

	pub async fn verify_bucket_ready(&self) -> anyhow::Result<()> {
		self.client
			.head_bucket()
			.bucket(&self.bucket_name)
			.send()
			.await
			.context("Failed to verify S3 bucket readiness")?;
		Ok(())
	}

	pub async fn wait_until_ready(
		&self,
		timeout: Duration,
	) -> anyhow::Result<()> {
		let started_at = Instant::now();
		let mut retry_delay = Duration::from_millis(250);

		let error = loop {
			match self.head_bucket().await {
				Ok(_) => return Ok(()),
				Err(error) => {
					let elapsed = started_at.elapsed();
					if elapsed >= timeout {
						break error;
					}

					let remaining = timeout.saturating_sub(elapsed);
					sleep(retry_delay.min(remaining)).await;
					retry_delay = retry_delay.saturating_mul(2).min(Duration::from_secs(5));
				}
			}
		};

		Err(error).with_context(|| format!("S3 storage did not become ready within {timeout:?}"))
	}

	pub async fn ensure_bucket_exists(&self) -> anyhow::Result<()> {
		if self.head_bucket().await? {
			return Ok(());
		}

		match self.client.create_bucket().bucket(&self.bucket_name).send().await {
			Ok(_) => {}
			Err(error) if create_bucket_error_means_existing_bucket(&error) => {}
			Err(error) => {
				return Err(error).context("Failed to create S3 bucket");
			}
		}

		self.verify_bucket_ready().await.context("Failed to verify S3 bucket after creation")?;
		Ok(())
	}

	pub async fn configure_upload_cors(
		&self,
		allowed_origins: &[String],
	) -> anyhow::Result<()> {
		if allowed_origins.is_empty() {
			anyhow::bail!("At least one CORS allowed origin is required");
		}

		let upload_rule = CorsRule::builder()
			.set_allowed_origins(Some(allowed_origins.to_vec()))
			.set_allowed_methods(Some(vec!["PUT".to_string()]))
			.set_allowed_headers(Some(vec!["*".to_string()]))
			.set_expose_headers(Some(vec!["ETag".to_string()]))
			.max_age_seconds(3000)
			.build()
			.context("Failed to build S3 upload CORS rule")?;
		let cors = CorsConfiguration::builder()
			.cors_rules(upload_rule)
			.build()
			.context("Failed to build S3 CORS configuration")?;

		self.client
			.put_bucket_cors()
			.bucket(&self.bucket_name)
			.cors_configuration(cors)
			.send()
			.await
			.context("Failed to configure S3 bucket CORS")?;
		Ok(())
	}

	async fn head_bucket(&self) -> anyhow::Result<bool> {
		match self.client.head_bucket().bucket(&self.bucket_name).send().await {
			Ok(_) => Ok(true),
			Err(error) if head_bucket_error_is_not_found(&error) => Ok(false),
			Err(error) => Err(error).context("Failed to check S3 bucket readiness"),
		}
	}
}

impl fmt::Debug for StorageClient {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("StorageClient")
			.field("bucket_name", &self.bucket_name)
			.field("presigning_config", &self.presigning_config)
			.finish_non_exhaustive()
	}
}

fn head_bucket_error_is_not_found(error: &SdkError<HeadBucketError>) -> bool {
	error.as_service_error().is_some_and(HeadBucketError::is_not_found)
}

fn head_object_error_is_not_found(error: &SdkError<HeadObjectError>) -> bool {
	error.as_service_error().is_some_and(HeadObjectError::is_not_found)
}

fn abort_multipart_upload_error_is_no_such_upload(
	error: &SdkError<AbortMultipartUploadError>
) -> bool {
	error
		.as_service_error()
		.is_some_and(|error| error.is_no_such_upload() || error.code() == Some("NoSuchUpload"))
}

fn complete_multipart_upload_error_is_no_such_upload(
	error: &SdkError<CompleteMultipartUploadError>
) -> bool {
	error.as_service_error().is_some_and(|error| error.code() == Some("NoSuchUpload"))
}

fn create_bucket_error_means_existing_bucket(error: &SdkError<CreateBucketError>) -> bool {
	error.as_service_error().is_some_and(|error| {
		error.is_bucket_already_exists() || error.is_bucket_already_owned_by_you()
	})
}

fn s3_client_config(
	config: &StorageConfig,
	endpoint_url: &str,
) -> aws_sdk_s3::Config {
	let credentials = Credentials::new(
		config.access_key.clone(),
		config.secret_key.clone(),
		None,
		None,
		"memory-map",
	);
	aws_sdk_s3::Config::builder()
		.behavior_version(BehaviorVersion::latest())
		.region(Region::new(config.region.clone()))
		.credentials_provider(credentials)
		.endpoint_url(endpoint_url)
		.force_path_style(config.force_path_style)
		.build()
}

fn browser_can_set_header(name: &str) -> bool {
	let name = name.to_ascii_lowercase();
	if name.starts_with("proxy-") || name.starts_with("sec-") {
		return false;
	}

	!matches!(
		name.as_str(),
		"accept-charset" |
			"accept-encoding" |
			"access-control-request-headers" |
			"access-control-request-method" |
			"connection" |
			"content-length" |
			"cookie" | "date" |
			"dnt" | "expect" |
			"host" | "keep-alive" |
			"origin" | "permissions-policy" |
			"referer" | "te" |
			"trailer" | "transfer-encoding" |
			"upgrade" | "user-agent" |
			"via"
	)
}

fn storage_key_delete_batches(storage_keys: &[String]) -> impl Iterator<Item = &[String]> {
	storage_keys.chunks(StorageClient::MAX_DELETE_OBJECTS_PER_REQUEST)
}

#[cfg(test)]
mod tests {
	use super::{
		StorageClient,
		StorageConfig,
		browser_can_set_header,
		storage_key_delete_batches,
	};

	fn storage_config_with_ttl(presigned_url_ttl_seconds: u64) -> StorageConfig {
		StorageConfig {
			endpoint_url: "http://127.0.0.1:9000/".to_string(),
			public_endpoint_url: None,
			access_key: "memorymapdev".to_string(),
			secret_key: "memorymapdevsecret".to_string(),
			bucket_name: "memory-map".to_string(),
			region: "us-east-1".to_string(),
			force_path_style: true,
			presigned_url_ttl_seconds,
		}
	}

	#[test]
	fn storage_config_accepts_presigned_url_ttl_boundaries() {
		assert!(storage_config_with_ttl(1).validate().is_ok());
		assert!(
			storage_config_with_ttl(StorageConfig::MAX_PRESIGNED_URL_TTL_SECONDS)
				.validate()
				.is_ok()
		);
	}

	#[test]
	fn storage_config_rejects_presigned_url_ttl_outside_allowed_range() {
		assert!(storage_config_with_ttl(0).validate().is_err());
		assert!(
			storage_config_with_ttl(StorageConfig::MAX_PRESIGNED_URL_TTL_SECONDS + 1)
				.validate()
				.is_err()
		);
	}

	#[test]
	fn storage_config_debug_redacts_credentials() {
		let mut config = storage_config_with_ttl(60);
		config.public_endpoint_url = Some("https://public-s3.example.test".to_string());
		config.access_key = "debug-access-key-secret".to_string();
		config.secret_key = "debug-secret-key-secret".to_string();

		let debug = format!("{config:?}");

		assert!(debug.contains("StorageConfig"));
		assert!(debug.contains("http://127.0.0.1:9000/"));
		assert!(debug.contains("https://public-s3.example.test"));
		assert!(debug.contains("<redacted>"));
		assert!(!debug.contains("debug-access-key-secret"));
		assert!(!debug.contains("debug-secret-key-secret"));
	}

	#[test]
	fn storage_config_rejects_empty_public_endpoint_url() {
		let mut config = storage_config_with_ttl(60);
		config.public_endpoint_url = Some(" ".to_string());

		let error = config.validate().err();
		assert!(
			error.as_ref().is_some_and(|error| error.to_string().contains("public_endpoint_url"))
		);
	}

	#[test]
	fn presigned_upload_headers_skip_browser_forbidden_headers() {
		assert!(browser_can_set_header("x-amz-checksum-crc32"));
		assert!(!browser_can_set_header("host"));
		assert!(!browser_can_set_header("content-length"));
		assert!(!browser_can_set_header("proxy-authorization"));
		assert!(!browser_can_set_header("sec-fetch-mode"));
	}

	#[test]
	fn delete_object_batches_respect_s3_multi_delete_limit() {
		let storage_keys = (0 .. StorageClient::MAX_DELETE_OBJECTS_PER_REQUEST + 1)
			.map(|index| format!("object-{index}"))
			.collect::<Vec<_>>();
		let batch_lengths =
			storage_key_delete_batches(&storage_keys).map(<[String]>::len).collect::<Vec<_>>();

		assert_eq!(batch_lengths, vec![StorageClient::MAX_DELETE_OBJECTS_PER_REQUEST, 1]);
	}
}
