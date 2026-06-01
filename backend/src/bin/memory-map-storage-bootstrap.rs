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
	std::{
		collections::BTreeSet,
		time::Duration,
	},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config =
		StorageBootstrapConfig::from_env().context("Failed to read storage bootstrap config")?;
	ensure_rustfs_health(&config.storage.endpoint_url)
		.await
		.context("Failed to verify RustFS health endpoint")?;
	let storage = StorageClient::from_storage_config(&config.storage)
		.context("Failed to build storage client")?;
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

#[derive(serde::Deserialize)]
struct RustFsHealth {
	service: String,
}

async fn ensure_rustfs_health(endpoint_url: &str) -> anyhow::Result<()> {
	let health_url = rustfs_health_url(endpoint_url)?;
	let client = reqwest::Client::builder()
		.timeout(Duration::from_secs(5))
		.build()
		.context("Failed to build RustFS health HTTP client")?;
	let response = client
		.get(health_url.clone())
		.send()
		.await
		.with_context(|| format!("Failed to request RustFS health endpoint: {health_url}"))?;
	let status = response.status();
	if !status.is_success() {
		anyhow::bail!("RustFS health endpoint returned HTTP {status}");
	}
	let body = response.text().await.context("Failed to read RustFS health response")?;
	let health: RustFsHealth = serde_json::from_str(&body)
		.context("Failed to parse RustFS health endpoint response as JSON")?;
	if !health.service.to_ascii_lowercase().contains("rustfs") {
		anyhow::bail!(
			"RustFS health endpoint did not identify a RustFS service (got service={:?})",
			health.service
		);
	}
	Ok(())
}

fn rustfs_health_url(endpoint_url: &str) -> anyhow::Result<reqwest::Url> {
	let mut url = reqwest::Url::parse(endpoint_url).context("S3 endpoint URL must be valid")?;
	let base_path = url.path().trim_matches('/');
	let health_path = if base_path.is_empty() {
		"/health/ready".to_string()
	} else {
		format!("/{base_path}/health/ready")
	};
	url.set_path(&health_path);
	url.set_query(None);
	url.set_fragment(None);
	if url.host_str().is_none() {
		anyhow::bail!("S3 endpoint URL is missing a host");
	}
	Ok(url)
}

#[cfg(test)]
mod tests {
	use {
		super::{
			StorageBootstrapConfig,
			rustfs_health_url,
		},
		backend::{
			CorsConfig,
			FrontendConfig,
			storage::StorageConfig,
		},
	};

	#[test]
	fn rustfs_health_url_uses_root_health_path() -> anyhow::Result<()> {
		let url = rustfs_health_url("http://127.0.0.1:9000/")?;

		assert_eq!(url.as_str(), "http://127.0.0.1:9000/health/ready");
		Ok(())
	}

	#[test]
	fn rustfs_health_url_preserves_endpoint_base_path() -> anyhow::Result<()> {
		let url = rustfs_health_url("http://127.0.0.1:9000/s3/")?;

		assert_eq!(url.as_str(), "http://127.0.0.1:9000/s3/health/ready");
		Ok(())
	}

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
