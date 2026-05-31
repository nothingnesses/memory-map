use {
	anyhow::Context,
	backend::storage::{
		StorageClient,
		StorageConfig,
	},
	std::time::Duration,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = StorageConfig::from_env().context("Failed to read storage bootstrap config")?;
	ensure_rustfs_health(&config.endpoint_url)
		.await
		.context("Failed to verify RustFS health endpoint")?;
	let storage =
		StorageClient::from_storage_config(&config).context("Failed to build storage client")?;
	storage.ensure_bucket_exists().await.context("Failed to ensure S3 bucket exists")?;
	Ok(())
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
	if !(body.contains("\"service\"") && body.contains("\"rustfs-endpoint\"")) {
		anyhow::bail!("RustFS health endpoint did not identify a RustFS service");
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
	use super::rustfs_health_url;

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
}
