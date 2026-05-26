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
		fmt,
		time::Duration,
	},
};

#[derive(Clone)]
pub struct StorageClient {
	client: Client,
	bucket_name: String,
	presigning_config: PresigningConfig,
}

impl StorageClient {
	pub fn from_config(config: &Config) -> anyhow::Result<Self> {
		let credentials = Credentials::new(
			config.s3_access_key.clone(),
			config.s3_secret_key.clone(),
			None,
			None,
			"memory-map",
		);
		let sdk_config = aws_sdk_s3::Config::builder()
			.behavior_version(BehaviorVersion::latest())
			.region(Region::new(config.s3_region.clone()))
			.credentials_provider(credentials)
			.endpoint_url(config.s3_endpoint_url.clone())
			.force_path_style(config.s3_force_path_style)
			.build();
		let presigning_config =
			PresigningConfig::expires_in(Duration::from_secs(config.s3_presigned_url_ttl_seconds))
				.context("Failed to configure S3 presigned URL expiry")?;

		Ok(Self {
			client: Client::from_conf(sdk_config),
			bucket_name: config.s3_bucket_name.clone(),
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
