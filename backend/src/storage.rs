use {
	crate::{
		Config,
		constants::ERR_UPLOAD_STORAGE,
	},
	anyhow::Context,
	aws_credential_types::Credentials,
	aws_sdk_s3::{
		Client,
		config::{
			BehaviorVersion,
			Region,
		},
		error::SdkError,
		operation::{
			create_bucket::CreateBucketError,
			head_bucket::HeadBucketError,
		},
		presigning::PresigningConfig,
		primitives::ByteStream,
		types::{
			Delete,
			ObjectIdentifier,
		},
	},
	std::{
		env,
		fmt,
		time::Duration,
	},
};

#[derive(Clone, Debug)]
pub struct StorageConfig {
	pub endpoint_url: String,
	pub access_key: String,
	pub secret_key: String,
	pub bucket_name: String,
	pub region: String,
	pub force_path_style: bool,
	pub presigned_url_ttl_seconds: u64,
}

impl StorageConfig {
	pub const MAX_PRESIGNED_URL_TTL_SECONDS: u64 = 604_800;

	pub fn from_env() -> anyhow::Result<Self> {
		let config = Self {
			endpoint_url: required_env("S3_ENDPOINT_URL")?,
			access_key: required_env("S3_ACCESS_KEY")?,
			secret_key: required_env("S3_SECRET_KEY")?,
			bucket_name: required_env("S3_BUCKET_NAME")?,
			region: env_or_default("S3_REGION", "us-east-1"),
			force_path_style: parse_bool_env("S3_FORCE_PATH_STYLE", true)?,
			presigned_url_ttl_seconds: env_or_default("S3_PRESIGNED_URL_TTL_SECONDS", "604800")
				.parse()
				.context("S3_PRESIGNED_URL_TTL_SECONDS must be an unsigned integer")?,
		};
		config.validate()?;
		Ok(config)
	}

	pub fn validate(&self) -> anyhow::Result<()> {
		if !(1 ..= Self::MAX_PRESIGNED_URL_TTL_SECONDS).contains(&self.presigned_url_ttl_seconds) {
			anyhow::bail!(
				"s3_presigned_url_ttl_seconds must be between 1 and {}",
				Self::MAX_PRESIGNED_URL_TTL_SECONDS
			);
		}
		Ok(())
	}
}

impl From<&Config> for StorageConfig {
	fn from(config: &Config) -> Self {
		Self {
			endpoint_url: config.s3_endpoint_url.clone(),
			access_key: config.s3_access_key.clone(),
			secret_key: config.s3_secret_key.clone(),
			bucket_name: config.s3_bucket_name.clone(),
			region: config.s3_region.clone(),
			force_path_style: config.s3_force_path_style,
			presigned_url_ttl_seconds: config.s3_presigned_url_ttl_seconds,
		}
	}
}

#[derive(Clone)]
pub struct StorageClient {
	client: Client,
	bucket_name: String,
	presigning_config: PresigningConfig,
}

impl StorageClient {
	pub fn from_config(config: &Config) -> anyhow::Result<Self> {
		Self::from_storage_config(&StorageConfig::from(config))
	}

	pub fn from_storage_config(config: &StorageConfig) -> anyhow::Result<Self> {
		config.validate()?;
		let credentials = Credentials::new(
			config.access_key.clone(),
			config.secret_key.clone(),
			None,
			None,
			"memory-map",
		);
		let sdk_config = aws_sdk_s3::Config::builder()
			.behavior_version(BehaviorVersion::latest())
			.region(Region::new(config.region.clone()))
			.credentials_provider(credentials)
			.endpoint_url(config.endpoint_url.clone())
			.force_path_style(config.force_path_style)
			.build();
		let presigning_config =
			PresigningConfig::expires_in(Duration::from_secs(config.presigned_url_ttl_seconds))
				.context("Failed to configure S3 presigned URL expiry")?;

		Ok(Self {
			client: Client::from_conf(sdk_config),
			bucket_name: config.bucket_name.clone(),
			presigning_config,
		})
	}

	pub async fn upload_object(
		&self,
		object_name: &str,
		bytes: impl Into<ByteStream>,
		content_type: impl Into<String>,
	) -> anyhow::Result<()> {
		self.client
			.put_object()
			.bucket(&self.bucket_name)
			.key(object_name)
			.body(bytes.into())
			.content_type(content_type.into())
			.send()
			.await
			.context(ERR_UPLOAD_STORAGE)?;
		Ok(())
	}

	pub async fn object_content_type(
		&self,
		object_name: &str,
	) -> anyhow::Result<String> {
		let output = self
			.client
			.head_object()
			.bucket(&self.bucket_name)
			.key(object_name)
			.send()
			.await
			.context("Failed to read S3 object metadata")?;
		output
			.content_type()
			.map(str::to_string)
			.context("S3 object response did not include Content-Type")
	}

	pub async fn presigned_get_url(
		&self,
		object_name: &str,
	) -> anyhow::Result<String> {
		let request = self
			.client
			.get_object()
			.bucket(&self.bucket_name)
			.key(object_name)
			.presigned(self.presigning_config.clone())
			.await
			.context("Failed to generate S3 presigned GET URL")?;
		Ok(request.uri().to_string())
	}

	pub async fn delete_objects(
		&self,
		object_names: &[String],
	) -> anyhow::Result<()> {
		if object_names.is_empty() {
			return Ok(());
		}

		let mut objects = Vec::with_capacity(object_names.len());
		for object_name in object_names {
			objects.push(
				ObjectIdentifier::builder()
					.key(object_name)
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

	pub async fn ensure_bucket_ready(&self) -> anyhow::Result<()> {
		self.client.list_buckets().send().await.context("Failed to list S3 buckets")?;

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

		self.client
			.head_bucket()
			.bucket(&self.bucket_name)
			.send()
			.await
			.context("Failed to verify S3 bucket after readiness check")?;
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

fn create_bucket_error_means_existing_bucket(error: &SdkError<CreateBucketError>) -> bool {
	error.as_service_error().is_some_and(|error| {
		error.is_bucket_already_exists() || error.is_bucket_already_owned_by_you()
	})
}

fn required_env(name: &str) -> anyhow::Result<String> {
	env::var(name).with_context(|| format!("{name} must be set"))
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
