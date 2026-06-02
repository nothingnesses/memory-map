use {
	anyhow::Context,
	backend::{
		CorsConfig,
		FrontendConfig,
		storage::{
			StorageClient,
			StorageConfig,
		},
	},
	serde::Deserialize,
	std::collections::BTreeSet,
};

const STORAGE_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config =
		StorageBootstrapConfig::from_env().context("Failed to read storage bootstrap config")?;
	let storage = StorageClient::from_storage_config(&config.storage)
		.context("Failed to build storage client")?;
	storage
		.wait_until_ready(STORAGE_READY_TIMEOUT)
		.await
		.context("Failed to wait for S3-compatible storage readiness")?;
	storage.ensure_bucket_exists().await.context("Failed to ensure S3 bucket exists")?;
	let cors_allowed_origins = config.upload_cors_allowed_origins();
	if !cors_allowed_origins.is_empty() {
		storage
			.configure_upload_cors(&cors_allowed_origins)
			.await
			.context("Failed to configure bucket CORS for browser uploads")?;
	}
	Ok(())
}

#[derive(Deserialize)]
struct StorageBootstrapConfig {
	storage: StorageConfig,
	#[serde(default)]
	frontend: Option<FrontendConfig>,
	#[serde(default)]
	cors: Option<CorsConfig>,
}

impl StorageBootstrapConfig {
	fn from_env() -> anyhow::Result<Self> {
		let raw = config::Config::builder()
			.add_source(
				config::Environment::with_prefix("MEMORY_MAP")
					.prefix_separator("__")
					.separator("__"),
			)
			.build()
			.context("Failed to read config from environment")?;
		let config: StorageBootstrapConfig = raw
			.try_deserialize()
			.context("Failed to deserialize storage bootstrap config from environment")?;
		config.storage.validate()?;
		Ok(config)
	}

	fn upload_cors_allowed_origins(&self) -> Vec<String> {
		let mut origins = BTreeSet::new();
		if let Some(frontend) = &self.frontend {
			insert_origin(&mut origins, &frontend.url);
		}
		if let Some(cors) = &self.cors {
			for origin in cors.allowed_origins.split(',') {
				insert_origin(&mut origins, origin);
			}
		}
		origins.into_iter().collect()
	}
}

fn insert_origin(
	origins: &mut BTreeSet<String>,
	origin: &str,
) {
	let origin = origin.trim();
	if !origin.is_empty() {
		origins.insert(origin.to_string());
	}
}

#[cfg(test)]
mod tests {
	use {
		super::StorageBootstrapConfig,
		backend::{
			CorsConfig,
			FrontendConfig,
			storage::StorageConfig,
		},
	};

	#[test]
	fn upload_cors_allowed_origins_combines_frontend_and_cors_config() {
		let config = StorageBootstrapConfig {
			storage: StorageConfig {
				endpoint_url: "http://127.0.0.1:9000".to_string(),
				public_endpoint_url: None,
				access_key: "access".to_string(),
				secret_key: "secret".to_string(),
				bucket_name: "memory-map".to_string(),
				region: "us-east-1".to_string(),
				force_path_style: true,
				presigned_url_ttl_seconds: 60,
			},
			frontend: Some(FrontendConfig {
				url: "http://127.0.0.1:3000".to_string(),
			}),
			cors: Some(CorsConfig {
				allowed_origins: "http://127.0.0.1:3000, http://localhost:3000".to_string(),
			}),
		};

		assert_eq!(
			config.upload_cors_allowed_origins(),
			vec!["http://127.0.0.1:3000".to_string(), "http://localhost:3000".to_string(),]
		);
	}
}
