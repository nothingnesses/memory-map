#![allow(dead_code)]

use {
	std::{
		env,
		sync::atomic::{
			AtomicU64,
			Ordering,
		},
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

const REQUIRE_SERVICE_ENV: &str = "BACKEND_TEST_REQUIRE_SERVICE";

pub(crate) fn env_or_default(
	name: &str,
	default: &str,
) -> String {
	env::var(name).unwrap_or_else(|_| default.to_string())
}

pub(crate) fn parse_bool_env(
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

pub(crate) async fn tcp_endpoint_is_reachable(
	host: &str,
	port: u16,
) -> anyhow::Result<bool> {
	Ok(matches!(timeout(Duration::from_secs(2), TcpStream::connect((host, port))).await, Ok(Ok(_))))
}

pub(crate) async fn endpoint_is_reachable(endpoint_url: &str) -> anyhow::Result<bool> {
	let url = reqwest::Url::parse(endpoint_url)?;
	let host =
		url.host_str().ok_or_else(|| anyhow::anyhow!("S3 endpoint URL is missing a host"))?;
	let port = url
		.port_or_known_default()
		.ok_or_else(|| anyhow::anyhow!("S3 endpoint URL is missing a port"))?;
	tcp_endpoint_is_reachable(host, port).await
}

pub(crate) fn skip_or_fail<T>(
	test_name: &str,
	message: String,
	skipped: T,
) -> anyhow::Result<T> {
	if backend_test_service_required() {
		anyhow::bail!("{message}");
	}

	eprintln!("skipping {test_name}: {message}");
	Ok(skipped)
}

pub(crate) fn backend_test_service_required() -> bool {
	env::var(REQUIRE_SERVICE_ENV)
		.map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
		.unwrap_or(false)
}

pub(crate) fn unique_suffix() -> anyhow::Result<String> {
	// Tests currently run serially when they require services, but the counter
	// keeps suffixes robust if a runner accidentally enables parallelism.
	static COUNTER: AtomicU64 = AtomicU64::new(0);
	let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
	let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
	Ok(format!("{}-{counter}", now.as_nanos()))
}
